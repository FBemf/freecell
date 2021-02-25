use std::convert::{TryFrom, TryInto};
use std::time::Duration;

use anyhow::{anyhow, Result};
use sdl2::pixels::Color;
use sdl2::rect::Rect;
use sdl2::render::Canvas;
use sdl2::rwops::RWops;
use sdl2::surface::Surface;
use sdl2::ttf::{Font, Sdl2TtfContext};

use super::engine::*;

pub struct CardRect {
    pub card: Card,
    pub rect: Rect,
    pub address: CardAddress,
    pub stack_size: Option<usize>,
}

// Used to find out whether a click was on a given card
pub fn rect_intersect(x: i32, y: i32, rect: &Rect) -> bool {
    let upper_left = (rect.x(), rect.y());
    let bottom_right = (
        upper_left.0 + i32::try_from(rect.width()).unwrap(),
        upper_left.1 + i32::try_from(rect.height()).unwrap(),
    );
    x >= upper_left.0 && y >= upper_left.1 && x <= bottom_right.0 && y <= bottom_right.1
}

// holds info about the proportions of the game field
struct Dimensions {
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
    canvas_width: u32,
    canvas_height: u32,
    // padding inside card
    card_v_padding: u32,
    card_h_padding: u32,
    // the font point sizes
    corner_point_size: u16,
    centre_point_size: u16,
    card_point_size: u16,
}

impl Dimensions {
    // calculates dimensions based on canvas size
    fn find(columns: u32, canvas_width: u32, canvas_height: u32) -> Dimensions {
        let big_margin = (canvas_width as f64 / 35.0).ceil() as u32;
        let small_margin = big_margin * 3 / 4;
        let h_border = big_margin;
        let v_border = big_margin;
        let tableau_border = big_margin;
        let free_cell_offset = small_margin;
        let col_margin = small_margin;
        let test_width: i32 =
            (canvas_width as i32 - 2 * h_border as i32 - (columns as i32 - 1) * col_margin as i32)
                / columns as i32;
        let (card_width, card_visible, card_overlap) = if test_width > 0 {
            let width = test_width as u32;
            let height = width * 8 / 7;
            let visible = height / 2;
            let overlap = height / 2;
            (width, visible, overlap)
        } else {
            let width = 0;
            let visible = 0;
            let overlap = 0;
            (width, visible, overlap)
        };
        let corner_point_size = (h_border + free_cell_offset) as u16 / 2;
        let centre_point_size = canvas_height as u16 / 9;
        let card_point_size = card_visible as u16 * 3 / 4;

        Dimensions {
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
            card_v_padding: 1,
            card_h_padding: 4,
            corner_point_size,
            centre_point_size,
            card_point_size,
        }
    }

    // find rect of the nth free cell
    fn get_free_cell(&self, n: u32) -> Rect {
        Rect::new(
            i32::try_from(self.h_border + self.card_width * n + self.col_margin * n).unwrap(),
            i32::try_from(self.v_border + self.free_cell_offset).unwrap(),
            self.card_width,
            self.card_visible + self.card_overlap,
        )
    }

    // find rect of the nth foundation
    fn get_foundation(&self, n: u32) -> Rect {
        Rect::new(
            i32::try_from(self.h_border + self.card_width * (n + 4) + self.col_margin * (n + 4))
                .unwrap(),
            i32::try_from(self.v_border).unwrap(),
            self.card_width,
            self.card_visible + self.card_overlap,
        )
    }

    // find rect of the mth card in the nth column
    // note that this does not factor in overlap; you have to draw them in order for the overlap to work
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

    // gets the rect representing the entire column.
    // when a held card is dropped, these are used to determine which column it's being dropped on
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

    // gets the nth card currently held
    fn get_floating(&self, mouse_x: i32, mouse_y: i32, card: u32) -> Rect {
        Rect::new(
            mouse_x - i32::try_from(self.card_width / 2).unwrap(),
            mouse_y - i32::try_from((self.card_overlap + self.card_visible) / 2).unwrap()
                + i32::try_from(card * self.card_visible).unwrap(),
            self.card_width,
            self.card_visible + self.card_overlap,
        )
    }

    // takes a rect and centres it right in the middle of the canvas
    fn centre_rect(&self, rect: &Rect) -> Option<Rect> {
        if rect.width() <= self.canvas_width && rect.height() <= self.canvas_height {
            let x = (self.canvas_width - rect.width()) / 2;
            let y = (self.canvas_height - rect.height()) / 2;
            Some(Rect::new(
                x.try_into().unwrap(),
                y.try_into().unwrap(),
                rect.width(),
                rect.height(),
            ))
        } else {
            None
        }
    }
}

struct Fonts<'a, 'b: 'a> {
    corner_font: Font<'a, 'static>,
    centre_font: Font<'a, 'static>,
    card_font: Font<'a, 'static>,
    ttf_context: &'b Sdl2TtfContext,
}

impl<'a, 'b: 'a> Fonts<'a, 'b> {
    fn load(dimensions: &Dimensions, ttf_context: &'b Sdl2TtfContext) -> Result<Self> {
        let regular = include_bytes!("SourceCodePro-Regular.otf");
        let bold = include_bytes!("SourceCodePro-Bold.otf");
        let corner_font: Font<'a, 'static> = ttf_context
            .load_font_from_rwops(
                RWops::from_bytes(regular).map_err(|s| anyhow!("loading font: {}", s))?,
                dimensions.corner_point_size,
            )
            .map_err(|s| anyhow!("initializing font: {}", s))?;
        let centre_font: Font<'a, 'static> = ttf_context
            .load_font_from_rwops(
                RWops::from_bytes(bold).map_err(|s| anyhow!("loading font: {}", s))?,
                dimensions.centre_point_size,
            )
            .map_err(|s| anyhow!("initializing font: {}", s))?;
        let card_font: Font<'a, 'static> = ttf_context
            .load_font_from_rwops(
                RWops::from_bytes(bold).map_err(|s| anyhow!("loading font: {}", s))?,
                dimensions.card_point_size,
            )
            .map_err(|s| anyhow!("initializing font: {}", s))?;
        Ok(Fonts {
            corner_font,
            centre_font,
            card_font,
            ttf_context,
        })
    }
}

struct Colours {
    background: Color,
    card_border: Color,
    card_colour: Color,
    victory_text_colour: Color,
    restart_text_colour: Color,
    status_text_colour: Color,
    red_card_colour: Color,
    black_card_colour: Color,
    faint_card_colour: Color,
}

impl Colours {
    fn default() -> Self {
        Colours {
            //background: Color::RGB(0x50, 0xa0, 0x50),
            background: Color::RGB(0x50, 0x85, 0x50),
            card_border: Color::RGB(0, 0, 0),
            card_colour: Color::RGB(0xff, 0xff, 0xff),
            faint_card_colour: Color::RGBA(0xff, 0xff, 0xff, 0x20),
            red_card_colour: Color::RGB(0xe0, 0x30, 0x30),
            black_card_colour: Color::RGB(0, 0, 0),
            status_text_colour: Color::RGB(0xff, 0xff, 0xff),
            victory_text_colour: Color::RGB(0xff, 0xff, 0xff),
            restart_text_colour: Color::RGB(0, 0, 0),
        }
    }
}

pub struct Timings {
    // how long to display UI text before it fades
    pub status_display_secs: Duration,
    pub window_size_display_secs: Duration,
    // how long to hold key to restart
    pub new_game_secs: Duration,
    // how long between auto-moves
    pub auto_move_secs: Duration,
}

impl Timings {
    fn default() -> Self {
        Timings {
            status_display_secs: Duration::from_secs(5),
            window_size_display_secs: Duration::from_secs(1),
            new_game_secs: Duration::from_secs_f32(2.5),
            auto_move_secs: Duration::from_secs_f32(0.2),
        }
    }
}

// Holds all of the configuration info about how the game should
// be displayed. Fonts, colours, coordinates,
pub struct UISettings<'a, 'b> {
    columns: u32,
    dimensions: Dimensions,
    colours: Colours,
    timings: Timings,
    fonts: Fonts<'a, 'b>,
}

impl<'a, 'b: 'a> UISettings<'a, 'b> {
    pub fn new(
        canvas_width: u32,
        canvas_height: u32,
        ttf_context: &'b Sdl2TtfContext,
    ) -> Result<Self> {
        let columns = 8;
        let dimensions = Dimensions::find(columns, canvas_width, canvas_height);
        let fonts: Fonts<'a, 'b> = Fonts::load(&dimensions, ttf_context)?;
        let colours = Colours::default();
        let timings = Timings::default();

        Ok(UISettings {
            columns,
            dimensions,
            timings,
            colours,
            fonts,
        })
    }

    // update all the proportions and font sizes
    // used when the window size changes
    pub fn update_proportions(&mut self, canvas_width: u32, canvas_height: u32) -> Result<()> {
        self.dimensions = Dimensions::find(self.columns, canvas_width, canvas_height);
        self.fonts = Fonts::load(&self.dimensions, self.fonts.ttf_context)?;
        Ok(())
    }

    pub fn timings(&self) -> &Timings {
        &self.timings
    }
}

// get all the rects representing all the cards
// they're ordered in draw order, which means cards later in the list are on top
pub fn get_card_rects(view: &GameView, settings: &UISettings) -> Vec<CardRect> {
    let mut board_rects = Vec::with_capacity(52);
    for (n, maybe_card) in view.free_cells.iter().enumerate() {
        if let Some(card) = maybe_card {
            board_rects.push(CardRect {
                card: *card,
                rect: settings.dimensions.get_free_cell(n.try_into().unwrap()),
                address: CardAddress::FreeCell(n),
                stack_size: None,
            });
        }
    }
    for (n, card) in view.foundations.iter().enumerate() {
        if card.rank != 0 {
            board_rects.push(CardRect {
                card: *card,
                rect: settings.dimensions.get_foundation(n.try_into().unwrap()),
                address: CardAddress::Foundation(n.try_into().unwrap()),
                stack_size: None,
            });
        }
    }
    for (i, column) in view.columns.iter().enumerate() {
        for (j, card) in column.iter().enumerate() {
            board_rects.push(CardRect {
                card: *card,
                rect: settings
                    .dimensions
                    .get_column_card(i.try_into().unwrap(), j.try_into().unwrap()),
                address: CardAddress::Column(i),
                stack_size: Some(column.len() - j),
            });
        }
    }
    board_rects
}

// get rects representing the areas you can place held cards
pub fn get_placement_zones(settings: &UISettings) -> Vec<(CardAddress, Rect)> {
    let mut zones = Vec::with_capacity(16);
    for n in 0..4 {
        let card = settings.dimensions.get_free_cell(n.try_into().unwrap());
        let zone = Rect::new(
            card.x() - i32::try_from(settings.dimensions.col_margin).unwrap() / 2,
            card.y() - i32::try_from(settings.dimensions.free_cell_offset).unwrap(),
            card.width() + settings.dimensions.col_margin,
            card.height() + settings.dimensions.free_cell_offset,
        );
        zones.push((CardAddress::FreeCell(n), zone));
    }
    for n in 0..4 {
        let card = settings.dimensions.get_foundation(n.try_into().unwrap());
        let zone = Rect::new(
            card.x() - i32::try_from(settings.dimensions.col_margin).unwrap() / 2,
            card.y(),
            card.width() + settings.dimensions.col_margin,
            card.height(),
        );
        zones.push((CardAddress::Foundation(n.try_into().unwrap()), zone));
    }
    for n in 0..8 {
        let card = settings.dimensions.get_column(n.try_into().unwrap());
        let zone = Rect::new(
            card.x() - i32::try_from(settings.dimensions.col_margin).unwrap() / 2,
            card.y(),
            card.width() + settings.dimensions.col_margin,
            card.height(),
        );
        zones.push((CardAddress::Column(n), zone));
    }
    zones
}

// get rects representing the cards currently held
// they're ordered in draw order, which means cards later in the list are on top
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
                settings
                    .dimensions
                    .get_floating(mouse_x, mouse_y, i.try_into().unwrap()),
            ))
        }
    }
    floating_rects
}

pub fn draw_background(canvas: &mut Canvas<sdl2::video::Window>, settings: &UISettings) {
    let old_colour = canvas.draw_color();
    canvas.set_draw_color(settings.colours.background);
    canvas.clear();
    canvas.set_draw_color(old_colour);
}

// draw the background & all the cards
pub fn draw_game<'a>(
    canvas: &mut Canvas<Surface<'a>>,
    view: &GameView,
    settings: &UISettings,
    mouse: (i32, i32),
) -> Result<()> {
    let old_colour = canvas.draw_color();

    canvas.set_draw_color(settings.colours.faint_card_colour);
    for n in 0..4 {
        let rect = settings.dimensions.get_free_cell(n);
        canvas
            .fill_rect(rect)
            .map_err(|e| anyhow!("filling rect: {}", e))?;
    }
    for n in 0..8 {
        let rect = settings.dimensions.get_column_card(n, 0);
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

fn draw_card<'a>(
    canvas: &mut Canvas<Surface<'a>>,
    settings: &UISettings,
    card: Card,
    rect: Rect,
) -> Result<()> {
    canvas.set_draw_color(settings.colours.card_colour);
    canvas
        .fill_rect(rect)
        .map_err(|e| anyhow!("filling rect: {}", e))?;
    canvas.set_draw_color(settings.colours.card_border);
    canvas
        .draw_rect(rect)
        .map_err(|e| anyhow!("drawing rect: {}", e))?;
    let text_colour = match card.suit {
        Suit::Clubs | Suit::Spades => settings.colours.black_card_colour,
        Suit::Hearts | Suit::Diamonds => settings.colours.red_card_colour,
    };
    if rect.width() > 2 * settings.dimensions.card_h_padding
        && rect.height() > 2 * settings.dimensions.card_v_padding
    {
        draw_text(
            canvas,
            &settings.fonts.card_font,
            text_colour,
            rect.x() + i32::try_from(settings.dimensions.card_h_padding).unwrap(),
            rect.y() + i32::try_from(settings.dimensions.card_v_padding).unwrap(),
            &format!("{}", card).as_str(),
            Some(settings.colours.card_colour),
        )
    } else {
        Ok(())
    }
}

fn draw_text_centred<'a>(
    ui_settings: &UISettings,
    canvas: &'a mut Canvas<Surface>,
    font: &Font,
    colour: Color,
    text: &str,
    background: Option<Color>,
) -> Result<()> {
    let surface = create_text_surface(font, colour, text, background)?;
    let rect =
        ui_settings
            .dimensions
            .centre_rect(&Rect::new(0, 0, surface.width(), surface.height()));
    surface
        .blit(None, canvas.surface_mut(), rect)
        .map_err(|s| anyhow!("rendering text to surface: {}", s))?;
    Ok(())
}

fn draw_text<'a>(
    canvas: &'a mut Canvas<Surface>,
    font: &Font,
    colour: Color,
    x: i32,
    y: i32,
    text: &str,
    background: Option<Color>,
) -> Result<()> {
    let surface = create_text_surface(font, colour, text, background)?;
    let rect = Rect::new(x, y, surface.width(), surface.height());
    surface
        .blit(None, canvas.surface_mut(), rect)
        .map_err(|s| anyhow!("rendering text to surface: {}", s))?;
    Ok(())
}

fn create_text_surface(
    font: &Font,
    colour: Color,
    text: &str,
    background: Option<Color>,
) -> Result<Surface<'static>> {
    if let Some(bkg) = background {
        font.render(text)
            .shaded(colour, bkg)
            .map_err(|s| anyhow!("rendering text to surface: {}", s))
    } else {
        font.render(text)
            .blended(colour)
            .map_err(|s| anyhow!("rendering text to surface: {}", s))
    }
}

pub fn draw_victory_text<'a>(
    ui_settings: &UISettings,
    canvas: &'a mut Canvas<Surface>,
    text: &str,
) -> Result<()> {
    draw_text_centred(
        ui_settings,
        canvas,
        &ui_settings.fonts.centre_font,
        ui_settings.colours.victory_text_colour,
        text,
        Some(ui_settings.colours.background),
    )
}

pub fn draw_reset_text<'a>(
    ui_settings: &UISettings,
    canvas: &'a mut Canvas<Surface>,
    text: &str,
) -> Result<()> {
    draw_text_centred(
        ui_settings,
        canvas,
        &ui_settings.fonts.centre_font,
        ui_settings.colours.restart_text_colour,
        text,
        None,
    )
}

pub fn draw_status_text<'a>(
    ui_settings: &UISettings,
    canvas: &'a mut Canvas<Surface>,
    text: &str,
) -> Result<()> {
    draw_text(
        canvas,
        &ui_settings.fonts.corner_font,
        ui_settings.colours.status_text_colour,
        0,
        0,
        text,
        Some(ui_settings.colours.background),
    )
}
