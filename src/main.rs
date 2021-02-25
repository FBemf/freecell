use std::convert::TryInto;
use std::env;
use std::path::PathBuf;
use std::thread::sleep;
use std::time::{Duration, Instant};

use anyhow::{anyhow, Context, Result};
use clipboard::{ClipboardContext, ClipboardProvider};
use rand::prelude::*;
use sdl2::event::{Event, WindowEvent};
use sdl2::keyboard::Keycode;
use sdl2::mouse::MouseState;
use sdl2::render::Canvas;
use sdl2::ttf::Sdl2TtfContext;
use sdl2::video::Window;
use sdl2::EventPump;
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
/// Hold `N` to start a new game with a random seed.
/// Press `S` to save your game.
/// Press `C` to copy the game's seed to your clipboard.
/// By loading from a seed, you can replay the same exact deal.
#[derive(Clone, StructOpt)]
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

// FSM regulating the "hold N to restart" state
#[derive(Clone, PartialEq)]
enum NewGameState {
    Starting(Instant), // "starting" means "if N isn't released, the game will restart at <instant>"
    Cooldown, // "cooldown" means "game just restarted, so N is still held, but we're no longer restarting"
    Ready,
}

// holds the current state of the world
struct State<'a, 'b: 'a> {
    opt: Opt,
    game: Game,
    undo_stack: GameUndoStack,
    ui_settings: UISettings<'a, 'b>,
    clipboard: Option<ClipboardContext>,
    canvas: Canvas<Window>,
    // text to display in corner & the time when it'll disappear
    status_text: Option<(Instant, String)>,
    // when you're holding the "new game" button, this is the instant after which it'll restart
    new_game_timer: NewGameState,
    seed: u64,
    // how long until the game can automatically move a card to the foundation
    next_auto_move: Instant,
}

fn main() -> Result<()> {
    let cli_options = Opt::from_args();

    let ttf_context = sdl2::ttf::init()?;
    let sdl_context = sdl2::init().unwrap();
    let video_subsystem = sdl_context.video().unwrap();
    let window = video_subsystem
        .window("FreeCell", 700, 700)
        .position_centered()
        .resizable()
        .build()
        .context("building window")?;
    let canvas = window.into_canvas().build().context("building canvas")?;
    let mut state = initialize_state(cli_options, canvas, &ttf_context)?;

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
        update(&mut state)?;

        sleep(Duration::new(0, 1_000_000_000u32 / 60));
    }

    Ok(())
}

fn initialize_state<'a>(
    opt: Opt,
    canvas: Canvas<Window>,
    ttf_context: &Sdl2TtfContext,
) -> Result<State> {
    let clipboard: Option<ClipboardContext> = if let Ok(c) = ClipboardProvider::new() {
        Some(c)
    } else {
        None
    };

    let ui_settings = UISettings::new(
        canvas.viewport().width(),
        canvas.viewport().height(),
        ttf_context,
    )?;

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

    let next_auto_move = Instant::now() + ui_settings.timings().auto_move_secs;
    let status_text: Option<(Instant, String)> = None;
    let new_game_timer = NewGameState::Ready;

    Ok(State {
        opt,
        canvas,
        clipboard,
        ui_settings,
        game,
        seed,
        status_text,
        new_game_timer,
        undo_stack,
        next_auto_move,
    })
}

fn draw_canvas(state: &mut State, event_pump: &EventPump) -> Result<()> {
    let frame = sdl2::surface::Surface::new(
        state.canvas.viewport().width(),
        state.canvas.viewport().height(),
        sdl2::pixels::PixelFormatEnum::RGBA8888,
    )
    .unwrap();
    let mut frame = frame
        .into_canvas()
        .map_err(|s| anyhow!("getting event pump: {}", s))?;

    let mouse = MouseState::new(&event_pump);
    draw_game(
        &mut frame,
        &state.game.view(),
        &state.ui_settings,
        (mouse.x(), mouse.y()),
    )?;

    let status_text = if let Some((instant, text)) = state.status_text.clone() {
        if instant < Instant::now() {
            state.status_text = None;
        }
        text
    } else {
        format!("seed: {}", state.seed)
    };

    draw_status_text(&state.ui_settings, &mut frame, &status_text)?;

    if let NewGameState::Starting(restart_time) = state.new_game_timer {
        let now = Instant::now();
        if restart_time > now {
            let time_remaining: f64 = (restart_time - now).as_secs_f64();
            if time_remaining > 0.0 {
                let proportion_elapsed: f64 =
                    (state.ui_settings.timings().new_game_secs.as_secs_f64() - time_remaining)
                        / state.ui_settings.timings().new_game_secs.as_secs_f64();
                draw_reset_text(
                    &state.ui_settings,
                    &mut frame,
                    &format!(
                        "Shuffling{}",
                        ".".repeat((6.0 * proportion_elapsed).floor() as usize)
                    ),
                )?;
            }
        }
    } else if state.game.view().is_won() {
        draw_victory_text(&state.ui_settings, &mut frame, "You Win!")?;
    }

    draw_background(&mut state.canvas, &state.ui_settings);
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
            if !state.game.has_floating() {
                for card_rect in get_card_rects(&state.game.view(), &state.ui_settings)
                    .iter()
                    .rev()
                {
                    if rect_intersect(x, y, &card_rect.rect) {
                        if let Some(size) = card_rect.stack_size {
                            match state.game.pick_up_stack(card_rect.address, size) {
                                Ok(new_state) => {
                                    state.game =
                                        state.undo_stack.update(state.game.clone(), new_state);
                                }
                                Err(MoveError::CannotPickUp { .. }) => {}
                                Err(_) => unreachable!(),
                            }
                        } else {
                            match state.game.pick_up_card(card_rect.address) {
                                Ok(new_state) => {
                                    state.game =
                                        state.undo_stack.update(state.game.clone(), new_state);
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
            if state.game.has_floating() {
                let mut did_something = false;
                for (address, rect) in get_placement_zones(&state.ui_settings).iter() {
                    if rect_intersect(x, y, rect) {
                        match state.game.place(*address) {
                            Ok(new_state) => {
                                did_something = true;
                                state.game = state.undo_stack.update(state.game.clone(), new_state);
                            }
                            Err(MoveError::CannotPlace { .. }) => {}
                            Err(_) => unreachable!(),
                        }
                    }
                }
                if !did_something {
                    state.game = state.undo_stack.undo(state.game.clone());
                }
            }
        }

        Event::KeyDown {
            keycode: Some(key), ..
        } => match key {
            Keycode::Backspace => {
                state.game = state.undo_stack.undo(state.game.clone());
            }
            Keycode::Return => {
                state.game = state.undo_stack.redo(state.game.clone());
            }
            Keycode::C => {
                if let Some(ctx) = &mut state.clipboard {
                    if let Err(e) = ctx.set_contents(state.seed.to_string()) {
                        state.status_text = Some((
                            Instant::now() + state.ui_settings.timings().status_display_secs,
                            "Clipboard Error".to_string(),
                        ));
                        if !state.opt.quiet {
                            eprintln!("Couldn't access clipboard {}", e);
                        }
                    } else {
                        state.status_text = Some((
                            Instant::now() + state.ui_settings.timings().status_display_secs,
                            "Copied!".to_string(),
                        ));
                    }
                } else {
                    state.status_text = Some((
                        Instant::now() + state.ui_settings.timings().status_display_secs,
                        "Clipboard Error".to_string(),
                    ));
                    if !state.opt.quiet {
                        eprintln!("Clipboard is unavailable");
                    }
                }
            }
            Keycode::S => {
                match save(
                    state.seed,
                    &state.game,
                    &state.undo_stack,
                    env::current_dir()?,
                    "freecell_save.",
                ) {
                    Ok(filename) => {
                        state.status_text = Some((
                            Instant::now() + state.ui_settings.timings().status_display_secs,
                            format!("Saved to {:?}", filename),
                        ));
                        if !state.opt.quiet {
                            eprintln!("Saved to {:?}", filename);
                        }
                    }
                    Err(e) => {
                        state.status_text = Some((
                            Instant::now() + state.ui_settings.timings().status_display_secs,
                            "Save Error".to_string(),
                        ));
                        if !state.opt.quiet {
                            eprintln!("Error saving: {}", e);
                        }
                    }
                }
            }
            Keycode::N => {
                if state.new_game_timer == NewGameState::Ready {
                    state.new_game_timer = NewGameState::Starting(
                        Instant::now() + state.ui_settings.timings().new_game_secs,
                    );
                }
            }
            _ => {}
        },

        Event::KeyUp {
            keycode: Some(key), ..
        } => match key {
            Keycode::N => {
                state.new_game_timer = NewGameState::Ready;
            }
            _ => {}
        },

        Event::Window {
            win_event: event, ..
        } => match event {
            WindowEvent::Resized(width, height) => {
                state
                    .ui_settings
                    .update_proportions(width.try_into().unwrap(), height.try_into().unwrap())?;
                state.status_text = Some((
                    Instant::now() + state.ui_settings.timings().window_size_display_secs,
                    format!("window size: ({} x {})", width, height),
                ));
            }
            _ => {}
        },

        _ => {}
    }
    return Ok(false);
}

fn update(state: &mut State) -> Result<()> {
    if state.next_auto_move <= Instant::now() {
        if let Some(new_state) = state.game.auto_move_to_foundations() {
            state.game = state.undo_stack.sneak_update(state.game.clone(), new_state);
            state.next_auto_move = Instant::now() + state.ui_settings.timings().auto_move_secs;
        }
    }
    if let NewGameState::Starting(time) = state.new_game_timer {
        if time <= Instant::now() {
            // restart game with new seed
            let seed: u64 = thread_rng().gen();
            state.seed = seed;
            state.game = Game::new_game(seed);
            state.undo_stack = GameUndoStack::new();
            state.new_game_timer = NewGameState::Cooldown;
            state.status_text = None;
            state.next_auto_move = Instant::now() + state.ui_settings.timings().auto_move_secs;
            if !state.opt.quiet {
                eprintln!("Started new game. Seed is {}", seed);
            }
        }
    }
    Ok(())
}
