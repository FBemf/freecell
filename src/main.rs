use std::env;
use std::fs;
use std::io::Write;
use std::path::PathBuf;
use std::thread::sleep;
use std::time::{Duration, Instant};

use anyhow::{anyhow, Context, Result};
use clipboard::{ClipboardContext, ClipboardProvider};
use rand::prelude::*;
use sdl2::event::Event;
use sdl2::keyboard::Keycode;
use sdl2::mouse::MouseState;
use sdl2::pixels::Color;
use sdl2::rect::Rect;
use sdl2::render::Canvas;
use sdl2::video::Window;
use sdl2::EventPump;
use sdl2_unifont::renderer::SurfaceRenderer;
use structopt::StructOpt;

mod display;
mod engine;

use display::*;
use engine::*;

/// Play FreeCell
///
/// Sort all the cards into the top right by suit in ascending order in order to win.
/// On the main board, you can stack cards on top of each other alternating colours.
/// You have four free cells in the top left, each of which can hold any single card.
///
/// Undo your previous move with `U` or `Backspace`.
/// Redo an undone move with `R` or `Enter`.
///
/// Press `S` to save your game.
/// Press `C` to copy the game's seed to your clipboard.
/// By loading from a seed, you can replay the same exact deal.
#[derive(StructOpt)]
#[structopt(name = "freecell", about = "FreeCell solitaire game")]
struct Opt {
    /// Seed to randomly generate game from
    #[structopt(short, long)]
    seed: Option<u64>,
    /// Save file to load
    #[structopt(short, long)]
    load: Option<PathBuf>,
    /// Output nothing to stdout or stderr
    #[structopt(short, long)]
    quiet: bool,
}

struct State {
    opt: Opt,
    game: Game,
    view: GameView,
    undo_stack: GameUndoStack,
    display_settings: DisplaySettings,
    clipboard: Option<ClipboardContext>,
    canvas: Canvas<Window>,
    status_text: Option<(Instant, String)>,
    seed: u64,
    last_auto_moved: Instant,
}

fn main() -> Result<()> {
    let cli_options = Opt::from_args();

    let sdl_context = sdl2::init().unwrap();
    let video_subsystem = sdl_context.video().unwrap();
    let window = video_subsystem
        .window("FreeCell", 700, 800)
        .position_centered()
        .build()
        .context("building window")?;
    let canvas = window.into_canvas().build().context("building canvas")?;
    let mut state = initialize_state(cli_options, canvas)?;

    let mut event_pump = sdl_context
        .event_pump()
        .map_err(|s| anyhow!("getting event pump: {}", s))?;

    'running: loop {
        for event in event_pump.poll_iter() {
            if handle_event(event, &mut state)? {
                break 'running;
            }
        }

        draw_canvas(&mut state, &event_pump)?;
        update(&mut state);

        sleep(Duration::new(0, 1_000_000_000u32 / 60));
    }

    Ok(())
}

fn initialize_state(opt: Opt, mut canvas: Canvas<Window>) -> Result<State> {
    let clipboard: Option<ClipboardContext> = if let Ok(c) = ClipboardProvider::new() {
        Some(c)
    } else {
        None
    };

    let display_settings =
        DisplaySettings::new(canvas.viewport().width(), canvas.viewport().height());

    let (seed, game, undo_stack) = if let Some(path) = &opt.load {
        if !opt.quiet {
            if let Some(_) = opt.seed {
                eprintln!("Ignoring seed in favour of loading from file");
            }
            eprintln!("Loading from {:?}", path);
        }
        load(path)?
    } else {
        let seed = if let Some(s) = opt.seed {
            s
        } else {
            rand::thread_rng().gen()
        };
        if !opt.quiet {
            eprintln!("Seed is {}", seed);
        }
        (seed, Game::new_game(seed), GameUndoStack::new())
    };

    let view = game.view();

    canvas.set_draw_color(display_settings.background);
    canvas.clear();
    canvas.present();

    let last_auto_moved = Instant::now();
    let status_text: Option<(Instant, String)> = None;

    Ok(State {
        opt,
        canvas,
        clipboard,
        display_settings,
        game,
        seed,
        status_text,
        undo_stack,
        view,
        last_auto_moved,
    })
}

fn draw_canvas(state: &mut State, event_pump: &EventPump) -> Result<()> {
    state.canvas.clear();
    let frame = sdl2::surface::Surface::new(
        state.canvas.viewport().width(),
        state.canvas.viewport().height(),
        sdl2::pixels::PixelFormatEnum::RGBA8888,
    )
    .unwrap();
    let mut frame = frame
        .into_canvas()
        .map_err(|s| anyhow!("getting event pump: {}", s))?;

    let corner_text = if let Some((instant, text)) = state.status_text.clone() {
        if instant.elapsed() > Duration::from_secs(state.display_settings.text_display_secs) {
            state.status_text = None;
        }
        text
    } else {
        format!("seed: {}", state.seed)
    };
    let renderer = SurfaceRenderer::new(state.display_settings.ui_text, Color::RGBA(0, 0, 0, 0));
    let text_rect = Rect::new(0, 0, 200, 50);
    renderer
        .draw(&corner_text)
        .map_err(|e| anyhow!("drawing text: {}", e))?
        .blit(None, frame.surface_mut(), text_rect)
        .map_err(|e| anyhow!("blit-ing text: {}", e))?;

    draw_game(
        &mut frame,
        &state.view,
        &state.display_settings,
        MouseState::new(&event_pump),
    )?;

    if state.view.is_won() {
        let mut renderer =
            SurfaceRenderer::new(state.display_settings.ui_text, Color::RGBA(0, 0, 0, 0));
        renderer.bold = true;
        renderer.scale = 8;
        draw_text(&mut frame, &state.display_settings, "You Win!", &renderer)?;
    }

    let texture_creator = state.canvas.texture_creator();
    let frame_tex = texture_creator.create_texture_from_surface(frame.surface())?;
    state
        .canvas
        .copy(&frame_tex, None, None)
        .map_err(|s| anyhow!("getting event pump: {}", s))?;
    state.canvas.present();
    Ok(())
}

// returns true if it wants to quit
fn handle_event(event: Event, state: &mut State) -> Result<bool> {
    match event {
        Event::Quit { .. } => {
            return Ok(true);
        }

        Event::MouseButtonDown { x, y, .. } => {
            if state.view.floating.is_none() {
                for card_rect in get_card_rects(&state.view, &state.display_settings)
                    .iter()
                    .rev()
                {
                    if rect_intersect(x, y, &card_rect.rect) {
                        if let Some(size) = card_rect.stack_size {
                            match state.game.pick_up_stack(card_rect.address, size) {
                                Ok(new_state) => {
                                    state.game =
                                        state.undo_stack.update(state.game.clone(), new_state);
                                    state.view = state.game.view();
                                }
                                Err(MoveError::CannotPickUp { .. }) => {}
                                Err(_) => unreachable!(),
                            }
                        } else {
                            match state.game.pick_up_card(card_rect.address) {
                                Ok(new_state) => {
                                    state.game =
                                        state.undo_stack.update(state.game.clone(), new_state);
                                    state.view = state.game.view();
                                }
                                Err(MoveError::CannotPickUp { .. }) => {}
                                Err(_) => unreachable!(),
                            }
                        }
                    }
                }
            }
        }

        Event::MouseButtonUp { x, y, .. } => {
            if state.view.floating.is_some() {
                let mut did_something = false;
                for (address, rect) in get_placement_zones(&state.display_settings).iter() {
                    if rect_intersect(x, y, rect) {
                        match state.game.place(*address) {
                            Ok(new_state) => {
                                did_something = true;
                                state.game = state.undo_stack.update(state.game.clone(), new_state);
                                state.view = state.game.view();
                            }
                            Err(MoveError::CannotPlace { .. }) => {}
                            Err(_) => unreachable!(),
                        }
                    }
                }
                if !did_something {
                    state.game = state.undo_stack.undo(state.game.clone());
                    state.view = state.game.view();
                }
            }
        }

        Event::KeyDown {
            keycode: Some(key), ..
        } => match key {
            Keycode::U | Keycode::Backspace => {
                state.game = state.undo_stack.undo(state.game.clone());
                state.view = state.game.view();
            }
            Keycode::R | Keycode::Return => {
                state.game = state.undo_stack.redo(state.game.clone());
                state.view = state.game.view();
            }
            Keycode::C => {
                if let Some(ctx) = &mut state.clipboard {
                    if let Err(e) = ctx.set_contents(state.seed.to_string()) {
                        state.status_text = Some((Instant::now(), "Error".to_string()));
                        if !state.opt.quiet {
                            eprintln!("Couldn't access clipboard {}", e);
                        }
                    } else {
                        state.status_text = Some((Instant::now(), "Copied!".to_string()));
                    }
                } else {
                    state.status_text = Some((Instant::now(), "Clipboard Error".to_string()));
                    if !state.opt.quiet {
                        eprintln!("Clipboard is unavailable");
                    }
                }
            }
            Keycode::S => match save(state.seed, &state.game, &state.undo_stack) {
                Ok(filename) => {
                    state.status_text = Some((Instant::now(), format!("Saved to {:?}", filename)));
                    if !state.opt.quiet {
                        eprintln!("Saved to {:?}", filename);
                    }
                }
                Err(e) => {
                    state.status_text = Some((Instant::now(), "Save Error".to_string()));
                    if !state.opt.quiet {
                        eprintln!("Error saving: {}", e);
                    }
                }
            },
            _ => {}
        },
        _ => {}
    }
    return Ok(false);
}

fn update(state: &mut State) {
    if state.last_auto_moved.elapsed() >= Duration::from_secs_f64(0.2) {
        if let Some(new_state) = state.game.auto_move_to_foundations() {
            state.game = state.undo_stack.sneak_update(state.game.clone(), new_state);
            state.view = state.game.view();
            state.last_auto_moved = Instant::now();
        }
    }
}

// load game
pub fn load(filename: &PathBuf) -> Result<(u64, Game, GameUndoStack)> {
    let save = fs::read_to_string(filename)?;
    let result: (u64, Game, GameUndoStack) = serde_json::from_str(&save)?;
    Ok(result)
}

// save game
pub fn save(seed: u64, game: &Game, undo: &GameUndoStack) -> Result<PathBuf> {
    let save = serde_json::to_string(&(seed, game, undo))?;
    let dir = env::current_dir()?;
    let name = "freecell_save.".to_string();
    for n in 0.. {
        let mut filename = dir.clone();
        filename.push(name.clone() + &n.to_string());
        if !filename.exists() {
            let mut file = fs::File::create(filename.clone())?;
            file.write_all(save.as_bytes())?;
            return Ok(filename);
        }
    }
    unreachable!();
}
