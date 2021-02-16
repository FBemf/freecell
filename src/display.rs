use std::convert::{TryFrom, TryInto};

use anyhow::{anyhow, Result};
use sdl2::mouse::MouseState;
use sdl2::pixels::Color;
use sdl2::rect::Rect;
use sdl2::render::Canvas;
use sdl2::surface::Surface;
use sdl2_unifont::renderer::SurfaceRenderer;

use super::engine::*;

pub struct CardRect {
    pub card: Card,
    pub rect: Rect,
    pub address: CardAddress,
    pub stack_size: Option<usize>,
}

pub fn rect_intersect(x: i32, y: i32, rect: &Rect) -> bool {
    let upper_left = (rect.x(), rect.y());
    let bottom_right = (
        upper_left.0 + i32::try_from(rect.width()).unwrap(),
        upper_left.1 + i32::try_from(rect.height()).unwrap(),
    );
    x >= upper_left.0 && y >= upper_left.1 && x <= bottom_right.0 && y <= bottom_right.1
}

pub struct DisplaySettings {
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
    card_v_margin: u32,
    card_h_margin: u32,
    pub background: Color,
    pub card_border: Color,
    pub card_color: Color,
    pub black_text: Color,
    pub red_text: Color,
    pub ui_text: Color,
    faint_card_color: Color,
}

impl DisplaySettings {
    pub fn new(canvas_width: u32, canvas_height: u32) -> Self {
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
            card_v_margin: 0,
            card_h_margin: 4,
            background: Color::RGB(0x50, 0xa0, 0x50),
            card_border: Color::RGB(0, 0, 0),
            card_color: Color::RGB(0xff, 0xff, 0xff),
            black_text: Color::RGB(0, 0, 0),
            red_text: Color::RGB(0xe0, 0x30, 0x30),
            ui_text: Color::RGB(0xff, 0xff, 0xff),
            faint_card_color: Color::RGBA(0xff, 0xff, 0xff, 0x20),
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

pub fn get_card_rects(view: &GameView, settings: &DisplaySettings) -> Vec<CardRect> {
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

pub fn get_placement_zones(settings: &DisplaySettings) -> Vec<(CardAddress, Rect)> {
    let mut zones = Vec::with_capacity(16);
    for n in 0..4 {
        let card = settings.get_free_cell(n.try_into().unwrap());
        let zone = Rect::new(
            card.x() - i32::try_from(settings.col_margin).unwrap() / 2,
            card.y() - i32::try_from(settings.free_cell_offset).unwrap(),
            card.width() + settings.col_margin,
            card.height() + settings.free_cell_offset,
        );
        zones.push((CardAddress::FreeCell(n), zone));
    }
    for n in 0..4 {
        let card = settings.get_foundation(n.try_into().unwrap());
        let zone = Rect::new(
            card.x() - i32::try_from(settings.col_margin).unwrap() / 2,
            card.y(),
            card.width() + settings.col_margin,
            card.height(),
        );
        zones.push((CardAddress::Foundation(n.try_into().unwrap()), zone));
    }
    for n in 0..8 {
        let card = settings.get_column(n.try_into().unwrap());
        let zone = Rect::new(
            card.x() - i32::try_from(settings.col_margin).unwrap() / 2,
            card.y(),
            card.width() + settings.col_margin,
            card.height(),
        );
        zones.push((CardAddress::Column(n), zone));
    }
    zones
}

pub fn get_floating_rects(
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

pub fn draw_game<'a>(
    canvas: &mut Canvas<Surface<'a>>,
    view: &GameView,
    settings: &DisplaySettings,
    mouse: MouseState,
) -> Result<()> {
    let old_color = canvas.draw_color();

    for n in 0..4 {
        let rect = settings.get_free_cell(n);
        canvas.set_draw_color(settings.faint_card_color);
        canvas
            .fill_rect(rect)
            .map_err(|e| anyhow!("filling rect: {}", e))?;
    }
    for n in 0..8 {
        let rect = settings.get_column_card(n, 0);
        canvas.set_draw_color(settings.faint_card_color);
        canvas
            .fill_rect(rect)
            .map_err(|e| anyhow!("filling rect: {}", e))?;
    }

    for card_rect in get_card_rects(view, settings) {
        draw_card(canvas, settings, card_rect.card, card_rect.rect)?;
    }
    for (card, rect) in get_floating_rects(view, settings, mouse.x(), mouse.y()) {
        draw_card(canvas, settings, card, rect)?;
    }
    canvas.set_draw_color(old_color);
    Ok(())
}

pub fn draw_card<'a>(
    canvas: &mut Canvas<Surface<'a>>,
    settings: &DisplaySettings,
    card: Card,
    rect: Rect,
) -> Result<()> {
    canvas.set_draw_color(settings.card_color);
    canvas
        .fill_rect(rect)
        .map_err(|e| anyhow!("filling rect: {}", e))?;
    canvas.set_draw_color(settings.card_border);
    canvas
        .draw_rect(rect)
        .map_err(|e| anyhow!("drawing rect: {}", e))?;
    let writing_color = match card.suit {
        Suit::Clubs | Suit::Spades => settings.black_text,
        Suit::Hearts | Suit::Diamonds => settings.red_text,
    };
    let mut renderer = SurfaceRenderer::new(writing_color, Color::RGBA(0, 0, 0, 0));
    renderer.bold = true;
    renderer.scale = 2;
    let text_rect = Rect::new(
        rect.x() + i32::try_from(settings.card_h_margin).unwrap(),
        rect.y() + i32::try_from(settings.card_v_margin).unwrap(),
        rect.width() - 2 * settings.card_h_margin,
        rect.height() - 2 * settings.card_v_margin,
    );
    renderer
        .draw(&format!("{}", card).as_str())
        .map_err(|e| anyhow!("drawing text: {}", e))?
        .blit(None, canvas.surface_mut(), text_rect)
        .map_err(|e| anyhow!("blit-ing text: {}", e))?;
    Ok(())
}

pub fn draw_text<'a>(
    canvas: &mut Canvas<Surface<'a>>,
    settings: &DisplaySettings,
    text: &str,
    renderer: &SurfaceRenderer,
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
