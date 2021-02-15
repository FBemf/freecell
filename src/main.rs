use std::thread::sleep;
use std::time::{Duration, Instant};

use anyhow::{anyhow, Context, Result};
use sdl2::event::Event;
use sdl2::keyboard::Keycode;
use sdl2::mouse::MouseState;
use sdl2::pixels::Color;
use sdl2_unifont::renderer::SurfaceRenderer;

mod cardengine;
mod display;

use cardengine::*;
use display::*;

fn main() -> Result<()> {
    let sdl_context = sdl2::init().unwrap();
    let video_subsystem = sdl_context.video().unwrap();

    let window = video_subsystem
        .window("FreeCell", 700, 700)
        .position_centered()
        .build()
        .context("building window")?;

    let mut canvas = window.into_canvas().build().context("building canvas")?;
    let mut event_pump = sdl_context
        .event_pump()
        .map_err(|s| anyhow!("getting event pump: {}", s))?;

    let display_settings =
        DisplaySettings::new(canvas.viewport().width(), canvas.viewport().height());

    let mut game = Game::new_game(1);
    let mut undo_stack = Vec::new();
    let mut view = game.view();

    canvas.set_draw_color(display_settings.background);
    canvas.clear();
    canvas.present();

    let mut last_auto_moved = Instant::now();

    'running: loop {
        for event in event_pump.poll_iter() {
            match event {
                Event::Quit { .. } => {
                    break 'running;
                }

                Event::MouseButtonDown { x, y, .. } => {
                    if view.floating.is_none() {
                        for card_rect in get_card_rects(&view, &display_settings).iter().rev() {
                            if rect_intersect(x, y, &card_rect.rect) {
                                if let Some(size) = card_rect.stack_size {
                                    match game.pick_up_stack(card_rect.address, size) {
                                        Ok(new_state) => {
                                            undo_stack.push(game);
                                            game = new_state;
                                            view = game.view();
                                        }
                                        Err(MoveError::CannotPickUp { .. }) => {}
                                        Err(_) => unreachable!(),
                                    }
                                } else {
                                    match game.pick_up_card(card_rect.address) {
                                        Ok(new_state) => {
                                            undo_stack.push(game);
                                            game = new_state;
                                            view = game.view();
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
                    if view.floating.is_some() {
                        let mut did_something = false;
                        for (address, rect) in get_placement_zones(&display_settings).iter() {
                            if rect_intersect(x, y, rect) {
                                match game.place(*address) {
                                    Ok(new_state) => {
                                        did_something = true;
                                        game = new_state;
                                        view = game.view();
                                    }
                                    Err(MoveError::CannotPlace { .. }) => {}
                                    Err(_) => unreachable!(),
                                }
                            }
                        }
                        if !did_something {
                            game = undo_stack.pop().unwrap();
                            view = game.view();
                        }
                    }
                }

                Event::KeyDown {
                    keycode: Some(key), ..
                } => match key {
                    Keycode::U => {
                        if let Some(last_state) = undo_stack.pop() {
                            game = last_state;
                            view = game.view();
                        }
                    }
                    _ => {}
                },
                _ => {}
            }
        }

        canvas.clear();
        let frame = sdl2::surface::Surface::new(
            canvas.viewport().width(),
            canvas.viewport().height(),
            sdl2::pixels::PixelFormatEnum::RGBA8888,
        )
        .unwrap();
        let mut frame = frame
            .into_canvas()
            .map_err(|s| anyhow!("getting event pump: {}", s))?;
        draw_game(
            &mut frame,
            &view,
            &display_settings,
            MouseState::new(&event_pump),
        )?;
        let texture_creator = canvas.texture_creator();
        if game.is_won() {
            let mut renderer =
                SurfaceRenderer::new(display_settings.win_text, Color::RGBA(0, 0, 0, 0));
            renderer.bold = true;
            renderer.scale = 8;
            draw_text(&mut frame, &display_settings, "You Win!", &mut renderer)?;
        }
        let frame_tex = texture_creator.create_texture_from_surface(frame.surface())?;
        canvas
            .copy(&frame_tex, None, None)
            .map_err(|s| anyhow!("getting event pump: {}", s))?;
        canvas.present();

        if last_auto_moved.elapsed() >= Duration::from_secs_f64(0.2) {
            if let Some(new_state) = game.auto_move_to_foundations() {
                game = new_state; // notably, not undo-able
                view = game.view();
                last_auto_moved = Instant::now();
            }
        }

        sleep(Duration::new(0, 1_000_000_000u32 / 60));
    }

    Ok(())
}
