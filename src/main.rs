use std::convert::{TryFrom, TryInto};
use std::thread::sleep;
use std::time::{Duration, Instant};

use anyhow::{anyhow, Context, Result};
use sdl2::event::Event;
use sdl2::keyboard::Keycode;
use sdl2::mouse::MouseState;
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

    canvas.set_draw_color(Color::RGB(0xf0, 0xf0, 0xf0));
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
                                        Err(e @ MoveError::CannotPickUp { .. }) => {
                                            eprintln!("{}", e);
                                        }
                                        Err(_) => unreachable!(),
                                    }
                                } else {
                                    match game.pick_up_card(card_rect.address) {
                                        Ok(new_state) => {
                                            undo_stack.push(game);
                                            game = new_state;
                                            view = game.view();
                                        }
                                        Err(e @ MoveError::CannotPickUp { .. }) => {
                                            eprintln!("{}", e);
                                        }
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
                                    Err(e @ MoveError::CannotPlace { .. }) => {
                                        eprintln!("{}", e);
                                    }
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
        let frame_tex = texture_creator.create_texture_from_surface(frame.surface())?;
        if game.is_won() {
            let mut renderer =
                SurfaceRenderer::new(Color::RGB(0xff, 0xff, 0xff), Color::RGBA(0, 0, 0, 0));
            draw_text(&mut frame, &display_settings, "You Win!", &mut renderer)?;
        }
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

struct CardRect {
    card: Card,
    rect: Rect,
    address: CardAddress,
    stack_size: Option<usize>,
}

fn rect_intersect(x: i32, y: i32, rect: &Rect) -> bool {
    let upper_left = (rect.x(), rect.y());
    let bottom_right = (
        upper_left.0 + i32::try_from(rect.width()).unwrap(),
        upper_left.1 + i32::try_from(rect.height()).unwrap(),
    );
    x >= upper_left.0 && y >= upper_left.1 && x <= bottom_right.0 && y <= bottom_right.1
}

struct DisplaySettings {
    h_border: u32,
    v_border: u32,
    free_cell_offset: u32,
    tableau_border: u32,
    col_margin: u32,
    card_overlap: u32,
    card_visible: u32,
    card_width: u32,
    canvas_width: u32,
    canvas_height: u32,
}

impl DisplaySettings {
    fn new(canvas_width: u32, canvas_height: u32) -> Self {
        let columns = 8;
        let h_border = 20;
        let v_border = 20;
        let free_cell_offset = 15;
        let tableau_border = 20;
        let col_margin = 15;
        let card_visible = 40;
        let card_overlap = 40;
        let card_width = (canvas_width - 2 * h_border - (columns - 1) * col_margin) / columns;
        DisplaySettings {
            h_border,
            v_border,
            free_cell_offset,
            tableau_border,
            col_margin,
            card_overlap,
            card_visible,
            card_width,
            canvas_width,
            canvas_height,
        }
    }

    fn get_free_cell(&self, n: u32) -> Rect {
        Rect::new(
            i32::try_from(self.h_border + self.card_width * n + self.col_margin * n).unwrap(),
            i32::try_from(self.v_border + self.free_cell_offset).unwrap(),
            self.card_width,
            self.card_visible + self.card_overlap,
        )
    }

    fn get_foundation(&self, n: u32) -> Rect {
        Rect::new(
            i32::try_from(self.h_border + self.card_width * (n + 4) + self.col_margin * (n + 4))
                .unwrap(),
            i32::try_from(self.v_border).unwrap(),
            self.card_width,
            self.card_visible + self.card_overlap,
        )
    }

    fn get_column_card(&self, col: u32, card: u32) -> Rect {
        Rect::new(
            i32::try_from(self.h_border + self.card_width * col + self.col_margin * col).unwrap(),
            i32::try_from(
                self.v_border
                    + self.free_cell_offset
                    + self.tableau_border
                    + self.card_overlap
                    + self.card_visible * (1 + card),
            )
            .unwrap(),
            self.card_width,
            self.card_visible + self.card_overlap,
        )
    }

    fn get_column(&self, col: u32) -> Rect {
        let top = self.v_border
            + self.free_cell_offset
            + self.tableau_border
            + self.card_overlap
            + self.card_visible;

        Rect::new(
            i32::try_from(self.h_border + self.card_width * col + self.col_margin * col).unwrap(),
            i32::try_from(top).unwrap(),
            self.card_width,
            self.canvas_height - top,
        )
    }

    fn get_floating(&self, mouse_x: i32, mouse_y: i32, card: u32) -> Rect {
        Rect::new(
            mouse_x - i32::try_from(self.card_width / 2).unwrap(),
            mouse_y - i32::try_from((self.card_overlap + self.card_visible) / 2).unwrap()
                + i32::try_from(card * self.card_visible).unwrap(),
            self.card_width,
            self.card_visible + self.card_overlap,
        )
    }

    fn centre_surface(&self, surface: &Surface) -> Rect {
        let x = (self.canvas_width - surface.width()) / 2;
        let y = (self.canvas_height - surface.height()) / 2;
        Rect::new(
            x.try_into().unwrap(),
            y.try_into().unwrap(),
            surface.width(),
            surface.height(),
        )
    }
}

fn get_card_rects(view: &GameView, settings: &DisplaySettings) -> Vec<CardRect> {
    let mut board_rects = Vec::with_capacity(52);
    for (n, maybe_card) in view.free_cells.iter().enumerate() {
        if let Some(card) = maybe_card {
            board_rects.push(CardRect {
                card: *card,
                rect: settings.get_free_cell(n.try_into().unwrap()),
                address: CardAddress::FreeCell(n),
                stack_size: None,
            });
        }
    }
    for (n, card) in view.foundations.iter().enumerate() {
        if card.rank != 0 {
            board_rects.push(CardRect {
                card: *card,
                rect: settings.get_foundation(n.try_into().unwrap()),
                address: CardAddress::Foundation(n.try_into().unwrap()),
                stack_size: None,
            });
        }
    }
    for (i, column) in view.columns.iter().enumerate() {
        for (j, card) in column.iter().enumerate() {
            board_rects.push(CardRect {
                card: *card,
                rect: settings.get_column_card(i.try_into().unwrap(), j.try_into().unwrap()),
                address: CardAddress::Column(i),
                stack_size: Some(column.len() - j),
            });
        }
    }
    board_rects
}

fn get_placement_zones(settings: &DisplaySettings) -> Vec<(CardAddress, Rect)> {
    let mut zones = Vec::with_capacity(16);
    for n in 0..4 {
        zones.push((
            CardAddress::FreeCell(n),
            settings.get_free_cell(n.try_into().unwrap()),
        ));
    }
    for n in 0..4 {
        zones.push((
            CardAddress::Foundation(n.try_into().unwrap()),
            settings.get_foundation(n.try_into().unwrap()),
        ));
    }
    for n in 0..8 {
        zones.push((
            CardAddress::Column(n),
            settings.get_column(n.try_into().unwrap()),
        ));
    }
    zones
}

fn get_floating_rects(
    view: &GameView,
    settings: &DisplaySettings,
    mouse_x: i32,
    mouse_y: i32,
) -> Vec<(Card, Rect)> {
    let mut floating_rects = Vec::new();
    if let Some(cards) = &view.floating {
        for (i, card) in cards.iter().enumerate() {
            floating_rects.push((
                *card,
                settings.get_floating(mouse_x, mouse_y, i.try_into().unwrap()),
            ))
        }
    }
    floating_rects
}

fn draw_game<'a>(
    canvas: &mut Canvas<Surface<'a>>,
    view: &GameView,
    settings: &DisplaySettings,
    mouse: MouseState,
) -> Result<()> {
    let old_color = canvas.draw_color();

    for card_rect in get_card_rects(view, settings) {
        draw_card(canvas, card_rect.card, card_rect.rect)?;
    }
    for (card, rect) in get_floating_rects(view, settings, mouse.x(), mouse.y()) {
        draw_card(canvas, card, rect)?;
    }
    canvas.set_draw_color(old_color);
    Ok(())
}

fn draw_card<'a>(canvas: &mut Canvas<Surface<'a>>, card: Card, rect: Rect) -> Result<()> {
    canvas.set_draw_color(Color::RGB(0xff, 0xff, 0xff));
    canvas
        .fill_rect(rect)
        .map_err(|e| anyhow!("filling rect: {}", e))?;
    canvas.set_draw_color(Color::RGB(0, 0, 0));
    canvas
        .draw_rect(rect)
        .map_err(|e| anyhow!("drawing rect: {}", e))?;
    let writing_color = match card.suit {
        Suit::Clubs | Suit::Spades => Color::RGB(0, 0, 0),
        Suit::Hearts | Suit::Diamonds => Color::RGB(0xe0, 0x30, 0x30),
    };
    let mut renderer = SurfaceRenderer::new(writing_color, Color::RGBA(0, 0, 0, 0));
    renderer.bold = true;
    renderer.scale = 2;
    renderer
        .draw(&format!("{}", card).as_str())
        .map_err(|e| anyhow!("drawing text: {}", e))?
        .blit(None, canvas.surface_mut(), rect)
        .map_err(|e| anyhow!("blit-ing text: {}", e))?;
    Ok(())
}

fn draw_text<'a>(
    canvas: &mut Canvas<Surface<'a>>,
    settings: &DisplaySettings,
    text: &str,
    renderer: &mut SurfaceRenderer,
) -> Result<()> {
    let text_surf = renderer
        .draw(text)
        .map_err(|e| anyhow!("drawing text: {}", e))?;
    text_surf
        .blit(
            None,
            canvas.surface_mut(),
            settings.centre_surface(&text_surf),
        )
        .map_err(|e| anyhow!("blit-ing text: {}", e))?;
    Ok(())
}
