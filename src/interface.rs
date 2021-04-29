use anyhow::Result;
use sdl2::event::{Event, WindowEvent};
use sdl2::keyboard::Keycode;
use std::convert::TryInto;
use std::env;

use super::display::*;
use super::logic::*;
use super::*;

// Holds a few state machines and times that regulate the UI
pub struct InterfaceState {
    // text to display in corner & the time when it'll disappear
    pub status_text: Option<(Instant, String)>,
    // when you're holding the "new game" button, this is the instant after which it'll restart
    pub n_key_state: NewGameState,
    // how long until the game can automatically move a card to the foundation
    pub next_auto_move: Instant,
    // is the s key being held
    pub s_key_held: bool,
}

// NewGameState is a ype defining a finite state machine which
// regulates the state of the "hold N to restart" process
#[derive(Clone, PartialEq)]
pub enum NewGameState {
    Starting(Instant), // "starting" means "if N isn't released, the game will restart at <instant>"
    Cooldown, // "cooldown" means "game just restarted, so N is still held, but we're no longer restarting"
    Ready,
}


impl InterfaceState {
    pub fn new(ui_settings: &UiSettings) -> Self {
        // timeout until an automatic move can be performed
        let next_auto_move = Instant::now() + ui_settings.timings().auto_move_secs;
        // status text to draw to screen & time when it will fade
        let status_text: Option<(Instant, String)> = None;
        // status of "hold n to quit the game" system
        let n_key_state = NewGameState::Ready;
        let s_key_held = false;

        InterfaceState {
            next_auto_move,
            status_text,
            n_key_state,
            s_key_held,
        }
    }
}

// pick up cards at (x, y)
fn pick_up_cards(state: &mut State, x: i32, y: i32) {
    // if the player is not holding cards
    if !state.game.has_floating() {
        // find which card (if any) the player is clicking on
        for card_rect in get_card_rects(&state.game.view(), &state.ui_settings)
            .iter()
            .rev()
        {
            if rect_intersect(x, y, &card_rect.rect) {
                if let Some(size) = card_rect.stack_size {
                    // if the card being clicked on is part of a stack
                    // pick up the card and all the cards stacked on top of it
                    match state.game.pick_up_stack(card_rect.address, size) {
                        Ok(new_state) => {
                            state.game = state.undo_stack.update(state.game.clone(), new_state);
                        }
                        Err(MoveError::CannotPickUp { .. }) => {}
                        Err(_) => unreachable!(),
                    }
                } else {
                    // if the card is not in a stack, just pick up the one card
                    match state.game.pick_up_card(card_rect.address) {
                        Ok(new_state) => {
                            state.game = state.undo_stack.update(state.game.clone(), new_state);
                        }
                        Err(MoveError::CannotPickUp { .. }) => {}
                        Err(_) => unreachable!(),
                    }
                }
            }
        }
    }
}

// place cards at (x, y)
fn place_cards(state: &mut State, x: i32, y: i32) {
    // if the player is holding cards
    if state.game.has_floating() {
        let mut did_something = false;
        // find the location in the game layout corresponding to the mouse's location
        for (address, rect) in get_placement_zones(&state.ui_settings).iter() {
            if rect_intersect(x, y, rect) {
                // place the card at that location
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

fn copy_seed(state: &mut State) {
    if let Some(ctx) = &mut state.clipboard {
        if let Err(e) = ctx.set_contents(state.seed.to_string()) {
            state.interface_state.status_text = Some((
                Instant::now() + state.ui_settings.timings().status_display_secs,
                "Clipboard Error".to_string(),
            ));
            if !state.opt.quiet {
                eprintln!("Couldn't access clipboard {}", e);
            }
        } else {
            state.interface_state.status_text = Some((
                Instant::now() + state.ui_settings.timings().status_display_secs,
                "Copied!".to_string(),
            ));
        }
    } else {
        state.interface_state.status_text = Some((
            Instant::now() + state.ui_settings.timings().status_display_secs,
            "Clipboard Error".to_string(),
        ));
        if !state.opt.quiet {
            eprintln!("Clipboard is unavailable");
        }
    }
}

fn save_game(state: &mut State) -> Result<()> {
    match save(
        state.seed,
        &state.game,
        &state.undo_stack,
        env::current_dir()?,
        "freecell_save.",
    ) {
        Ok(filename) => {
            state.interface_state.status_text = Some((
                Instant::now() + state.ui_settings.timings().status_display_secs,
                format!("Saved to {:?}", filename),
            ));
            if !state.opt.quiet {
                eprintln!("Saved to {:?}", filename);
            }
        }
        Err(e) => {
            state.interface_state.status_text = Some((
                Instant::now() + state.ui_settings.timings().status_display_secs,
                "Save Error".to_string(),
            ));
            if !state.opt.quiet {
                eprintln!("Error saving: {}", e);
            }
        }
    };
    Ok(())
}

// handle a user interface event by modifying the game state.
// returns true when it is time to exit the game.
pub fn handle_event(event: Event, state: &mut State) -> Result<bool> {
    match event {
        Event::Quit { .. } => {
            return Ok(true);
        }

        Event::MouseButtonDown { x, y, .. } => {
            pick_up_cards(state, x, y);
        }

        Event::MouseButtonUp { x, y, .. } => {
            place_cards(state, x, y);
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
                copy_seed(state);
            }
            Keycode::S => {
                if !state.interface_state.s_key_held {
                    state.interface_state.s_key_held = true;
                    save_game(state)?;
                }
            }
            Keycode::N => {
                // begin restarting game
                if state.interface_state.n_key_state == NewGameState::Ready {
                    state.interface_state.n_key_state = NewGameState::Starting(
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
                // stop restarting game
                state.interface_state.n_key_state = NewGameState::Ready;
            }
            Keycode::S => {
                state.interface_state.s_key_held = false;
            }
            _ => {}
        },

        Event::Window {
            win_event: WindowEvent::Resized(width, height),
            ..
        } => {
            state
                .ui_settings
                .update_proportions(width.try_into().unwrap(), height.try_into().unwrap())?;
            state.interface_state.status_text = Some((
                Instant::now() + state.ui_settings.timings().window_size_display_secs,
                format!("window size: ({} x {})", width, height),
            ));
        }

        _ => {}
    }
    Ok(false)
}

pub fn draw_canvas(state: &mut State, event_pump: &EventPump) -> Result<()> {
    // build frame & mouse state
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

    // Draw game onto the frame based on mouse state
    draw_game(
        &mut frame,
        &state.game.view(),
        &state.ui_settings,
        (mouse.x(), mouse.y()),
    )?;

    // Read status text and draw it to frame
    let status_text = if let Some((instant, text)) = state.interface_state.status_text.clone() {
        if instant < Instant::now() {
            state.interface_state.status_text = None;
        }
        text
    } else {
        format!("seed: {}", state.seed)
    };
    draw_status_text(&state.ui_settings, &mut frame, &status_text)?;

    // If N is being held down, draw a restart message on the screen.
    // The opacity of that message increases gradually as N is held longer & longer
    if let NewGameState::Starting(restart_time) = state.interface_state.n_key_state {
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
        // otherwise, if the game is won, draw victory text
        draw_victory_text(&state.ui_settings, &mut frame, "You Win!")?;
    }

    // draw background, then draw the scene on top of it
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
