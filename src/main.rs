use std::convert::TryInto;
use std::thread::sleep;
use std::time::{Duration, Instant};

use anyhow::{anyhow, Context, Result};
use sdl2::event::Event;
use sdl2::pixels::Color;
use sdl2::rect::Rect;
use sdl2::render::Canvas;
use sdl2::surface::Surface;
use sdl2_unifont::renderer::SurfaceRenderer;

mod cardengine;

use cardengine::*;

fn main() -> Result<()> {
    let sdl_context = sdl2::init().unwrap();
    let video_subsystem = sdl_context.video().unwrap();

    let window = video_subsystem
        .window("FreeCell", 800, 800)
        .position_centered()
        .build()
        .context("building window")?;

    let mut canvas = window.into_canvas().build().context("building canvas")?;
    let mut event_pump = sdl_context
        .event_pump()
        .map_err(|s| anyhow!("getting event pump: {}", s))?;

    let mut game = Game::new_game(1);
    let mut view: Option<GameView> = None;

    canvas.set_draw_color(Color::RGB(0xf0, 0xf0, 0xf0));
    canvas.clear();
    canvas.present();

    'running: loop {
        for event in event_pump.poll_iter() {
            match event {
                Event::Quit { .. } => {
                    break 'running;
                }
                _ => {}
            }
        }

        if game.auto_move_to_foundations() {
            view = None;
        }
        canvas.clear();
        if view == None {
            view = Some(game.view());
        }
        if let Some(v) = &view {
            let frame_start = Instant::now();
            let frame = sdl2::surface::Surface::new(
                canvas.viewport().width(),
                canvas.viewport().height(),
                sdl2::pixels::PixelFormatEnum::RGBA8888,
            )
            .unwrap();
            let mut frame = frame
                .into_canvas()
                .map_err(|s| anyhow!("getting event pump: {}", s))?;
            draw_game(&mut frame, v)?;
            let texture_creator = canvas.texture_creator();
            let frame_tex = texture_creator.create_texture_from_surface(frame.surface())?;
            canvas
                .copy(&frame_tex, None, None)
                .map_err(|s| anyhow!("getting event pump: {}", s))?;
            canvas.present();
        } else {
            unreachable!();
        }
        sleep(Duration::new(0, 1_000_000_000u32 / 60));
    }

    Ok(())
}

fn draw_game<'a>(canvas: &mut Canvas<Surface<'a>>, view: &GameView) -> Result<()> {
    let old_color = canvas.draw_color();
    let horizontal_margin = 30;
    let vertical_margin = 10;
    let num_columns = 8;
    let num_rows = 15;
    let card_width =
        (canvas.viewport().width() - horizontal_margin * (num_columns + 1)) / num_columns;
    let card_height = (canvas.viewport().height() - vertical_margin * (num_rows + 1)) / num_rows;
    let mut renderer = SurfaceRenderer::new(Color::RGB(0, 0, 0), Color::RGBA(0, 0, 0, 0));
    renderer.bold = true;
    renderer.scale = 2;

    for (n, card) in view.free_cells.iter().enumerate() {
        if let Some(c) = card {
            draw_card(
                canvas,
                &mut renderer,
                *c,
                n,
                1,
                horizontal_margin,
                vertical_margin,
                card_width,
                card_height,
            )?;
        }
    }
    for (n, card) in view.foundations.iter().enumerate() {
        if card.rank != 0 {
            draw_card(
                canvas,
                &mut renderer,
                *card,
                n + 4,
                1,
                horizontal_margin,
                vertical_margin,
                card_width,
                card_height,
            )?;
        }
    }
    for (i, column) in view.columns.iter().enumerate() {
        for (j, card) in column.iter().enumerate() {
            draw_card(
                canvas,
                &mut renderer,
                *card,
                i,
                j + 3,
                horizontal_margin,
                vertical_margin,
                card_width,
                card_height,
            )?;
        }
    }
    canvas.set_draw_color(old_color);
    Ok(())
}

fn draw_card<'a>(
    canvas: &mut Canvas<Surface<'a>>,
    renderer: &mut SurfaceRenderer,
    card: Card,
    i: usize,
    j: usize,
    horizontal_margin: u32,
    vertical_margin: u32,
    card_width: u32,
    card_height: u32,
) -> Result<()> {
    let x = horizontal_margin + i as u32 * (horizontal_margin + card_width);
    let y = vertical_margin + j as u32 * (vertical_margin + card_height);
    let rect = Rect::new(
        x.try_into().unwrap(),
        y.try_into().unwrap(),
        card_width,
        card_height,
    );
    canvas.set_draw_color(Color::RGB(0xff, 0xff, 0xff));
    canvas
        .fill_rect(rect)
        .map_err(|e| anyhow!("filling rect: {}", e))?;
    canvas.set_draw_color(Color::RGB(0, 0, 0));
    canvas
        .draw_rect(rect)
        .map_err(|e| anyhow!("drawing rect: {}", e))?;
    renderer
        .draw(&format!("{}", card).as_str())
        .map_err(|e| anyhow!("drawing text: {}", e))?
        .blit(None, canvas.surface_mut(), rect)
        .map_err(|e| anyhow!("blit-ing text: {}", e))?;
    Ok(())
}
