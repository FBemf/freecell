use std::path::PathBuf;
use std::thread::sleep;
use std::time::{Duration, Instant};

use anyhow::{anyhow, Context, Result};
use clipboard::{ClipboardContext, ClipboardProvider};
use rand::prelude::*;
use sdl2::mouse::MouseState;
use sdl2::render::Canvas;
use sdl2::ttf::Sdl2TtfContext;
use sdl2::video::Window;
use sdl2::EventPump;
use structopt::StructOpt;

mod display;
mod gamelogic;
mod interface;

use display::*;
use gamelogic::*;
use interface::*;

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
struct CliOptions {
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

// holds the current state of the game
pub struct GameState<'a, 'b: 'a> {
    opt: CliOptions,
    board: Board,
    undo_stack: BoardUndoStack,
    ui_settings: UiSettings<'a, 'b>,
    clipboard: Option<ClipboardContext>,
    canvas: Canvas<Window>,
    seed: u64,
    interface_state: InterfaceState,
}

fn main() -> Result<()> {
    let cli_options = CliOptions::from_args();

    // Build the window, canvas, and event pump
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
    let mut user_input_events = sdl_context
        .event_pump()
        .map_err(|s| anyhow!("getting event pump: {}", s))?;

    let mut game_state = initialize_state(cli_options, canvas, &ttf_context)?;

    // runs every step
    'main: loop {
        for event in user_input_events.poll_iter() {
            if handle_event(event, &mut game_state)? {
                break 'main;
            }
        }

        draw_canvas(&mut game_state, &user_input_events)?;
        update_game_state(&mut game_state);

        // Wait one sixtieth of a second
        sleep(Duration::new(0, 1_000_000_000u32 / 60));
    }
    Ok(())
}

// Set up the struct which holds the state of the game
fn initialize_state(
    opt: CliOptions,
    canvas: Canvas<Window>,
    ttf_context: &Sdl2TtfContext,
) -> Result<GameState> {
    // Get clipboard controller
    let clipboard: Option<ClipboardContext> = if let Ok(c) = ClipboardProvider::new() {
        Some(c)
    } else {
        None
    };

    // Set up the UI
    let ui_settings = UiSettings::new(
        canvas.viewport().width(),
        canvas.viewport().height(),
        ttf_context,
    )?;

    // Initialize UI state
    let interface_state = InterfaceState::new(&ui_settings);

    // Initialize the game state, either from a random seed or by loading a save file
    let (seed, board, undo_stack) = if let Some(save_file_path) = &opt.load {
        if !opt.quiet {
            if opt.seed.is_some() {
                eprintln!("Ignoring seed in favour of loading from file");
            }
            eprintln!("Loading from {:?}", save_file_path);
        }
        load(save_file_path)?
    } else {
        // random seed
        let seed = if let Some(s) = opt.seed {
            s
        } else {
            rand::thread_rng().gen()
        };
        if !opt.quiet {
            eprintln!("Seed is {}", seed);
        }
        (seed, Board::new_game(seed), BoardUndoStack::new())
    };

    Ok(GameState {
        opt,
        canvas,
        clipboard,
        ui_settings,
        board,
        undo_stack,
        interface_state,
        seed,
    })
}

// updates the state of the game; executed every step
fn update_game_state(state: &mut GameState) {
    // If we're not still on cooldown from the last auto-move
    if state.interface_state.next_auto_move <= Instant::now() {
        // try auto-moving another card to the foundations
        if let Some(new_state) = state.board.auto_move_to_foundations() {
            state.board = state
                .undo_stack
                .sneak_update(state.board.clone(), new_state);
            // reset timeout
            state.interface_state.next_auto_move =
                Instant::now() + state.ui_settings.timings().auto_move_secs;
        }
    }

    // Clear status text if it has expired
    if let Some((instant, _)) = state.interface_state.status_text {
        if instant < Instant::now() {
            state.interface_state.status_text = None;
        }
    }

    // if the player has been holding down "N" long enough, restart the game
    if let NewGameState::Starting(time) = state.interface_state.n_key_state {
        if time <= Instant::now() {
            // restart game with new seed
            let seed: u64 = thread_rng().gen();
            state.seed = seed;
            state.board = Board::new_game(seed);
            state.undo_stack = BoardUndoStack::new();
            state.interface_state.n_key_state = NewGameState::Cooldown;
            state.interface_state.status_text = None;
            state.interface_state.next_auto_move =
                Instant::now() + state.ui_settings.timings().auto_move_secs;
            if !state.opt.quiet {
                eprintln!("Started new game. Seed is {}", seed);
            }
        }
    }
}
