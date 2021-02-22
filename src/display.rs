use std::convert::{TryFrom, TryInto};
use std::time::Duration;

use anyhow::{anyhow, Result};
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

pub struct UISettings {
    columns: u32,
    // margin around entire game
    h_border: u32,
    v_border: u32,
    // vertical offset of free cells from foundations
    free_cell_offset: u32,
    // margin between free cells and tableau
    tableau_border: u32,
    // margin between columns
    col_margin: u32,
    // vertical length of covered part of card
    card_overlap: u32,
    // vertical length of uncovered part of card
    card_visible: u32,
    // width of cards
    card_width: u32,
    // height & width of window
    pub canvas_width: u32,
    pub canvas_height: u32,
    // padding inside card
    card_v_padding: u32,
    card_h_padding: u32,
    // how long to display UI text before it fades
    pub status_display_secs: Duration,
    pub window_size_display_secs: Duration,
    // how long to hold key to restart
    pub new_game_secs: Duration,
    // how long between auto-moves
    pub auto_move_secs: Duration,
    // colours
    pub background: Color,
    pub card_border: Color,
    pub card_colour: Color,
    pub victory_text: TextSettings,
    pub restart_text: TextSettings,
    pub status_text: TextSettings,
    red_card_text: TextSettings,
    black_card_text: TextSettings,
    faint_card_colour: Color,
}

#[derive(Clone)]
pub struct TextSettings {
    colour: Color,
    scale: u32,
    bold: bool,
}

impl TextSettings {
    fn renderer(&self) -> SurfaceRenderer {
        let mut renderer = SurfaceRenderer::new(self.colour, Color::RGBA(0, 0, 0, 0));
        renderer.bold = self.bold;
        renderer.scale = self.scale;
        renderer
    }
}

impl UISettings {
    pub fn update_proportions(&mut self, canvas_width: u32, canvas_height: u32) {
        self.canvas_width = canvas_width;
        self.canvas_height = canvas_height;
        let big_margin = (canvas_width as f64 / 35.0).ceil() as u32;
        let small_margin = big_margin * 3 / 4;
        self.h_border = big_margin;
        self.v_border = big_margin;
        self.tableau_border = big_margin;
        self.free_cell_offset = small_margin;
        self.col_margin = small_margin;
        let test_width: i32 = (self.canvas_width as i32
            - 2 * self.h_border as i32
            - (self.columns as i32 - 1) * self.col_margin as i32)
            / self.columns as i32;
        if test_width > 0 {
            self.card_width = test_width as u32;
            let card_height = self.card_width * 8 / 7;
            self.card_visible = card_height / 2;
            self.card_overlap = card_height / 2;
        } else {
            self.card_width = 0;
            self.card_visible = 0;
            self.card_overlap = 0;
        }
    }

    pub fn new(canvas_width: u32, canvas_height: u32) -> Self {
        let columns = 8;

        let red_card_text = TextSettings {
            colour: Color::RGB(0xe0, 0x30, 0x30),
            bold: true,
            scale: 2,
        };
        let black_card_text = TextSettings {
            colour: Color::RGB(0, 0, 0),
            bold: true,
            scale: 2,
        };
        let status_text = TextSettings {
            colour: Color::RGB(0xff, 0xff, 0xff),
            bold: false,
            scale: 1,
        };
        let victory_text = TextSettings {
            colour: Color::RGB(0xff, 0xff, 0xff),
            bold: true,
            scale: 8,
        };
        let restart_text = TextSettings {
            colour: Color::RGB(0, 0, 0),
            bold: true,
            scale: 5,
        };

        let mut result = UISettings {
            columns,
            h_border: 0,
            v_border: 0,
            free_cell_offset: 0, //
            tableau_border: 0,   //
            col_margin: 0,       //
            card_overlap: 0,     // these will be set dynamically, below
            card_visible: 0,     //
            card_width: 0,       //
            canvas_width: 0,     //
            canvas_height: 0,    //
            card_v_padding: 0,
            card_h_padding: 4,
            status_display_secs: Duration::from_secs(5),
            window_size_display_secs: Duration::from_secs(1),
            new_game_secs: Duration::from_secs_f32(2.5),
            auto_move_secs: Duration::from_secs_f32(0.2),
            background: Color::RGB(0x50, 0xa0, 0x50),
            card_border: Color::RGB(0, 0, 0),
            card_colour: Color::RGB(0xff, 0xff, 0xff),
            faint_card_colour: Color::RGBA(0xff, 0xff, 0xff, 0x20),
            red_card_text,
            black_card_text,
            victory_text,
            restart_text,
            status_text,
        };

        result.update_proportions(canvas_width, canvas_height);
        result
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

    fn centre_surface(&self, surface: &Surface) -> Option<Rect> {
        if surface.width() <= self.canvas_width && surface.height() <= self.canvas_height {
            let x = (self.canvas_width - surface.width()) / 2;
            let y = (self.canvas_height - surface.height()) / 2;
            Some(Rect::new(
                x.try_into().unwrap(),
                y.try_into().unwrap(),
                surface.width(),
                surface.height(),
            ))
        } else {
            None
        }
    }
}

pub fn get_card_rects(view: &GameView, settings: &UISettings) -> Vec<CardRect> {
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

pub fn get_placement_zones(settings: &UISettings) -> Vec<(CardAddress, Rect)> {
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
    settings: &UISettings,
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
    settings: &UISettings,
    mouse: (i32, i32),
) -> Result<()> {
    let old_colour = canvas.draw_color();

    for n in 0..4 {
        let rect = settings.get_free_cell(n);
        canvas.set_draw_color(settings.faint_card_colour);
        canvas
            .fill_rect(rect)
            .map_err(|e| anyhow!("filling rect: {}", e))?;
    }
    for n in 0..8 {
        let rect = settings.get_column_card(n, 0);
        canvas.set_draw_color(settings.faint_card_colour);
        canvas
            .fill_rect(rect)
            .map_err(|e| anyhow!("filling rect: {}", e))?;
    }

    for card_rect in get_card_rects(view, settings) {
        draw_card(canvas, settings, card_rect.card, card_rect.rect)?;
    }
    for (card, rect) in get_floating_rects(view, settings, mouse.0, mouse.1) {
        draw_card(canvas, settings, card, rect)?;
    }
    canvas.set_draw_color(old_colour);
    Ok(())
}

pub fn draw_card<'a>(
    canvas: &mut Canvas<Surface<'a>>,
    settings: &UISettings,
    card: Card,
    rect: Rect,
) -> Result<()> {
    canvas.set_draw_color(settings.card_colour);
    canvas
        .fill_rect(rect)
        .map_err(|e| anyhow!("filling rect: {}", e))?;
    canvas.set_draw_color(settings.card_border);
    canvas
        .draw_rect(rect)
        .map_err(|e| anyhow!("drawing rect: {}", e))?;
    let text_settings = match card.suit {
        Suit::Clubs | Suit::Spades => &settings.black_card_text,
        Suit::Hearts | Suit::Diamonds => &settings.red_card_text,
    };
    if rect.width() > 2 * settings.card_h_padding && rect.height() > 2 * settings.card_v_padding {
        let text_rect = Rect::new(
            rect.x() + i32::try_from(settings.card_h_padding).unwrap(),
            rect.y() + i32::try_from(settings.card_v_padding).unwrap(),
            rect.width() - 2 * settings.card_h_padding,
            rect.height() - 2 * settings.card_v_padding,
        );
        draw_text_rect(
            canvas,
            text_settings,
            text_rect,
            &format!("{}", card).as_str(),
        )
    } else {
        Ok(())
    }
}

pub fn draw_text_centred<'a>(
    canvas: &mut Canvas<Surface<'a>>,
    ui_settings: &UISettings,
    text_settings: &TextSettings,
    text: &str,
) -> Result<()> {
    let mut dyn_text_settings = text_settings.clone();
    while dyn_text_settings.scale > 0 {
        let renderer = dyn_text_settings.renderer();
        let surface = renderer
            .draw(text)
            .map_err(|e| anyhow!("drawing text: {}", e))?;
        if let Some(rect) = ui_settings.centre_surface(&surface) {
            surface
                .blit(None, canvas.surface_mut(), rect)
                .map_err(|e| anyhow!("blit-ing text: {}", e))?;
            return Ok(());
        } else {
            if dyn_text_settings.bold {
                dyn_text_settings.bold = false;
            } else {
                dyn_text_settings.scale -= 1;
                if text_settings.bold {
                    dyn_text_settings.bold = true;
                }
            }
        }
    }
    return Ok(());
}

pub fn draw_text_rect<'a>(
    canvas: &mut Canvas<Surface<'a>>,
    text_settings: &TextSettings,
    rect: Rect,
    text: &str,
) -> Result<()> {
    let mut dyn_text_settings = text_settings.clone();
    while dyn_text_settings.scale > 0 {
        let renderer = dyn_text_settings.renderer();
        let surface = renderer
            .draw(text)
            .map_err(|e| anyhow!("drawing text: {}", e))?;
        if surface.width() <= rect.width() && surface.height() <= rect.height() {
            surface
                .blit(None, canvas.surface_mut(), rect)
                .map_err(|e| anyhow!("blit-ing text: {}", e))?;
            return Ok(());
        } else {
            if dyn_text_settings.bold {
                dyn_text_settings.bold = false;
            } else {
                dyn_text_settings.scale -= 1;
                if text_settings.bold {
                    dyn_text_settings.bold = true;
                }
            }
        }
    }
    return Ok(());
}
