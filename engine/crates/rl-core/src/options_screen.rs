//! "Opções": a port-only screen (RISING.EXE has none) for the modern settings.
//!
//! It is a separate module beside the faithful shell state machine (`shell*.rs`,
//! MenuInit): it never writes game memory, never consumes RNG and never draws on
//! the shell's persistent surface. It only READS the prepared shell session (the
//! MenuInit fonts at 0x4e2f74..76 and the animation counter 0x508f84) and draws
//! with the original primitives into its own buffer:
//! - the original menu background (TITRE.PIE, decoded by the host from its CD);
//! - MENU.CMP's three animations at AffGame's menu positions (`menu_sprites`);
//! - title and items with AffText in MenuDisplay's layout (`shell_draw::menu`).
//!
//! The "Opções" button over the main menu (2) and the pause menu (16) is drawn
//! the same way into the host's copy of the shell picture. While the screen is
//! closed only a click that starts on that button is taken; every other input
//! belongs to the original shell unchanged. While it is open it takes all input.
//! Texts come from the port (not from TEXTES.TXT), in CP1252 like the game's.
use crate::{
    render::{Cache, Context, Error, Result},
    render_assets::Assets,
    MemoriaJogo,
};
use std::{collections::BTreeMap, sync::Arc};

pub const WIDTH: usize = 800;
pub const PIXELS: usize = 480000;

/// Settings shown on the screen. The engine stores none of them: the host passes
/// its own values in and reads the changes back.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Values {
    /// Modern hotkeys (and the Shift+click order queue that follows them).
    pub hotkeys: bool,
    /// Better paths (rule bit 0, owned by the pathing front).
    pub paths: bool,
    /// Stage-start difficulty preset 0 easy, 1 normal, 2 hard.
    pub difficulty: u8,
    /// Host pace 0 = 0.5x, 1 = 1x, 2 = 2x, 3 = 4x.
    pub speed: u8,
    /// Items the host offers, one bit per `Item` (a clear bit draws it
    /// unavailable and skips it in navigation). Back is always available.
    pub available: u8,
}
impl Values {
    pub fn validate(&self) -> Result<()> {
        if self.difficulty > 2 || self.speed > 3 || self.available & !ALL != 0 {
            return Err(Error::Invalid("options values outside their ranges"));
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Language {
    PortugueseBr = 0,
    English = 1,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Item {
    Hotkeys = 0,
    ConfigureKeys = 1,
    Paths = 2,
    Difficulty = 3,
    Speed = 4,
    Back = 5,
}
pub const ITEMS: [Item; 6] = [
    Item::Hotkeys,
    Item::ConfigureKeys,
    Item::Paths,
    Item::Difficulty,
    Item::Speed,
    Item::Back,
];
pub const ALL: u8 = 0x3f;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Event {
    None,
    /// The button was clicked: the screen is now open.
    Opened,
    /// One value changed (the host applies and stores it).
    Changed(Item),
    /// "Configurar teclas": the host opens its own key dialog.
    ConfigureKeys,
    /// Back or Escape: the screen closed; the shell picture is current again.
    Closed,
}

/// One host input in original 800x600 coordinates. Keys are completed presses
/// (the host suppresses repeats), as for the shell. `held`/`released` keep the
/// original 0x5099ae/af meaning: a click is a release while held.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Input {
    pub enter: bool,
    pub up: bool,
    pub down: bool,
    pub escape: bool,
    pub left: bool,
    pub right: bool,
    pub x: i32,
    pub y: i32,
    pub held: bool,
    pub released: bool,
    /// Animation steps since the previous input (the host's shell ticks).
    pub ticks: u32,
}

/// What the screen reads from the prepared shell session (read only).
pub struct Look<'a> {
    pub memory: &'a MemoriaJogo,
    /// The shell renderer's assets: the original bitmap fonts.
    pub fonts: &'a Assets,
    /// The original MENU.CMP bank.
    pub menu: &'a Assets,
}
impl Look<'_> {
    /// MenuInit's fonts: title [0x4e2f74], item [0x4e2f75], selected [0x4e2f76].
    fn font(&self, address: u32) -> Result<u32> {
        let font = u32::from(self.memory.u8(address)?);
        if !(1..=6).contains(&font) {
            return Err(Error::Invalid("options: menu font outside banks 1..6"));
        }
        Ok(font)
    }
    fn mode(&self) -> Result<u8> {
        Ok(self.memory.u8(crate::shell_state::MODE)?)
    }
}

/// Screens that show the button: the main menu and the in-game pause menu.
pub fn has_button(mode: u8) -> bool {
    mode == 2 || mode == 16
}

// MenuDisplay 0x4693e7 rows: title [0,246,w,276], item i [0,286+25i,w,306+25i].
const TITLE_TOP: i32 = 246;
const ROW_TOP: i32 = 286;
const ROW_STEP: i32 = 25;
const ROW_HEIGHT: i32 = 20;
/// Horizontal margin of a clickable caption beyond its glyphs.
const HIT_MARGIN: i32 = 8;
/// The button's caption box (centre x, top y). Below the original items of both
/// menus and clear of MENU.CMP's animations (see the captures in the private research repository).
pub const BUTTON_CENTRE_X: i32 = 455;
pub const BUTTON_TOP: i32 = 548;

fn latin1(text: &str) -> Vec<u8> {
    text.chars()
        .map(|c| u8::try_from(u32::from(c)).expect("port texts are Latin-1 (CP1252)"))
        .collect()
}

/// The port's own text table (the game has no such texts). pt-BR is the
/// shipped language; English is prepared for the language option (private research repository issue #7).
fn text(language: Language, item: Item, values: &Values) -> String {
    let pt = language == Language::PortugueseBr;
    let yes_no = |on: bool| match (pt, on) {
        (true, true) => "SIM",
        (true, false) => "NÃO",
        (false, true) => "ON",
        (false, false) => "OFF",
    };
    match item {
        Item::Hotkeys => format!(
            "{}: {}",
            if pt {
                "ATALHOS MODERNOS"
            } else {
                "MODERN HOTKEYS"
            },
            yes_no(values.hotkeys)
        ),
        Item::ConfigureKeys => if pt {
            "CONFIGURAR TECLAS"
        } else {
            "CONFIGURE KEYS"
        }
        .into(),
        Item::Paths => format!(
            "{}: {}",
            if pt {
                "CAMINHOS MELHORES"
            } else {
                "BETTER PATHS"
            },
            yes_no(values.paths)
        ),
        Item::Difficulty => {
            let names = if pt {
                ["FÁCIL", "NORMAL", "DIFÍCIL"]
            } else {
                ["EASY", "NORMAL", "HARD"]
            };
            format!(
                "{}: {}",
                if pt { "DIFICULDADE" } else { "DIFFICULTY" },
                names[usize::from(values.difficulty.min(2))]
            )
        }
        Item::Speed => {
            let names = if pt {
                ["0,5x", "1x", "2x", "4x"]
            } else {
                ["0.5x", "1x", "2x", "4x"]
            };
            format!(
                "{}: {}",
                if pt { "VELOCIDADE" } else { "SPEED" },
                names[usize::from(values.speed.min(3))]
            )
        }
        Item::Back => if pt { "VOLTAR" } else { "BACK" }.into(),
    }
}
fn title(language: Language) -> &'static str {
    match language {
        Language::PortugueseBr => "OPÇÕES",
        Language::English => "OPTIONS",
    }
}

/// The screen's state. One per host; it outlives shell sessions.
#[derive(Clone)]
pub struct Options {
    pub values: Values,
    pub language: Language,
    background: Arc<Vec<u8>>,
    open: bool,
    selected: usize,
    animation: u32,
    /// A press that started on the button (its release belongs to us).
    armed: bool,
    /// A press that started on an item while open.
    pressed: Option<usize>,
    hover_button: bool,
}

impl Options {
    pub fn new(values: Values, language: Language, background: &[u8]) -> Result<Self> {
        values.validate()?;
        if background.len() != PIXELS {
            return Err(Error::Invalid(
                "options background must have 480000 indices",
            ));
        }
        Ok(Self {
            values,
            language,
            background: Arc::new(background.to_vec()),
            open: false,
            selected: 0,
            animation: 0,
            armed: false,
            pressed: None,
            hover_button: false,
        })
    }
    pub fn is_open(&self) -> bool {
        self.open
    }
    pub fn selected(&self) -> Item {
        ITEMS[self.selected]
    }
    pub fn set_values(&mut self, values: Values) -> Result<()> {
        values.validate()?;
        self.values = values;
        if self.open && !self.enabled(self.selected) {
            self.selected = self.first_enabled();
        }
        Ok(())
    }
    fn enabled(&self, index: usize) -> bool {
        ITEMS[index] == Item::Back || self.values.available & (1 << index) != 0
    }
    fn first_enabled(&self) -> usize {
        (0..ITEMS.len()).find(|&i| self.enabled(i)).unwrap_or(5)
    }
    /// Opens over the current shell screen (main 2 or pause 16 only). The MENU.CMP
    /// animations continue from the shell's counter 0x508f84.
    pub fn open(&mut self, look: &Look<'_>) -> Result<()> {
        if !has_button(look.mode()?) {
            return Err(Error::Invalid(
                "options open only over the main or pause menu",
            ));
        }
        self.open = true;
        self.selected = self.first_enabled();
        self.animation = look.memory.u32(0x508f84)?;
        self.armed = false;
        self.pressed = None;
        self.hover_button = false;
        Ok(())
    }
    pub fn close(&mut self) {
        self.open = false;
        self.armed = false;
        self.pressed = None;
        self.hover_button = false;
    }

    fn width(look: &Look<'_>, text: &[u8], font: u32) -> Result<i32> {
        crate::render_text::text_width(look.fonts, text, font)
    }
    fn caption_box(look: &Look<'_>, text: &[u8], font: u32, top: i32) -> Result<[i32; 4]> {
        let w = Self::width(look, text, font)?;
        let left = (WIDTH as i32 - w) / 2;
        Ok([
            left - HIT_MARGIN,
            top,
            left + w + HIT_MARGIN,
            top + ROW_STEP,
        ])
    }
    /// The button's clickable rectangle [left, top, right, bottom) in 800x600.
    pub fn button_rect(&self, look: &Look<'_>) -> Result<[i32; 4]> {
        let caption = latin1(title(self.language));
        let w = Self::width(look, &caption, look.font(0x4e2f75)?)?;
        let left = BUTTON_CENTRE_X - w / 2;
        Ok([
            left - HIT_MARGIN,
            BUTTON_TOP,
            left + w + HIT_MARGIN,
            BUTTON_TOP + ROW_STEP,
        ])
    }
    fn item_rect(&self, look: &Look<'_>, index: usize) -> Result<[i32; 4]> {
        let caption = latin1(&text(self.language, ITEMS[index], &self.values));
        Self::caption_box(
            look,
            &caption,
            look.font(0x4e2f76)?,
            ROW_TOP + ROW_STEP * index as i32,
        )
    }
    fn item_at(&self, look: &Look<'_>, x: i32, y: i32) -> Result<Option<usize>> {
        for index in 0..ITEMS.len() {
            if inside(self.item_rect(look, index)?, x, y) && self.enabled(index) {
                return Ok(Some(index));
            }
        }
        Ok(None)
    }

    /// One input. Returns (consumed, event). Closed: only a click that starts on
    /// the button is consumed (the shell must not see it); everything else
    /// returns consumed = false and goes to the original shell untouched.
    pub fn input(&mut self, look: &Look<'_>, input: &Input) -> Result<(bool, Event)> {
        if !self.open {
            if !has_button(look.mode()?) {
                self.armed = false;
                self.hover_button = false;
                return Ok((false, Event::None));
            }
            let on = inside(self.button_rect(look)?, input.x, input.y);
            self.hover_button = on;
            if input.released && input.held {
                if std::mem::take(&mut self.armed) {
                    if on {
                        self.open(look)?;
                        return Ok((true, Event::Opened));
                    }
                    return Ok((true, Event::None));
                }
                return Ok((false, Event::None));
            }
            if input.held && !self.armed && on && !input.released {
                self.armed = true;
                return Ok((true, Event::None));
            }
            if !input.held {
                self.armed = false;
            }
            return Ok((false, Event::None));
        }
        self.animation = self.animation.wrapping_add(input.ticks);
        let hovered = self.item_at(look, input.x, input.y)?;
        if let Some(index) = hovered {
            // MenuInput selects the item under the pointer (zone != 0).
            self.selected = index;
        }
        if input.escape {
            self.close();
            return Ok((true, Event::Closed));
        }
        if input.enter {
            return Ok((true, self.activate(1)));
        }
        if input.up || input.down {
            self.step(if input.up { -1 } else { 1 });
            return Ok((true, Event::None));
        }
        if input.left || input.right {
            let item = self.selected();
            if matches!(
                item,
                Item::Difficulty | Item::Speed | Item::Hotkeys | Item::Paths
            ) {
                return Ok((
                    true,
                    self.change(item, if input.left { -1 } else { 1 }, false),
                ));
            }
            return Ok((true, Event::None));
        }
        if input.held && input.released {
            if let Some(index) = self.pressed.take() {
                if hovered == Some(index) {
                    self.selected = index;
                    return Ok((true, self.activate(1)));
                }
            }
            return Ok((true, Event::None));
        }
        if input.held && self.pressed.is_none() {
            self.pressed = hovered;
        } else if !input.held {
            self.pressed = None;
        }
        Ok((true, Event::None))
    }
    fn step(&mut self, direction: i32) {
        let mut index = self.selected as i32;
        loop {
            index += direction;
            if !(0..ITEMS.len() as i32).contains(&index) {
                return;
            }
            if self.enabled(index as usize) {
                self.selected = index as usize;
                return;
            }
        }
    }
    fn change(&mut self, item: Item, direction: i32, wrap: bool) -> Event {
        let cycle = |value: u8, count: i32| -> u8 {
            let next = i32::from(value) + direction;
            if wrap {
                next.rem_euclid(count) as u8
            } else {
                next.clamp(0, count - 1) as u8
            }
        };
        let before = self.values;
        match item {
            Item::Hotkeys => self.values.hotkeys = !self.values.hotkeys,
            Item::Paths => self.values.paths = !self.values.paths,
            Item::Difficulty => self.values.difficulty = cycle(self.values.difficulty, 3),
            Item::Speed => self.values.speed = cycle(self.values.speed, 4),
            _ => {}
        }
        if self.values == before {
            Event::None
        } else {
            Event::Changed(item)
        }
    }
    fn activate(&mut self, direction: i32) -> Event {
        let item = self.selected();
        if !self.enabled(self.selected) {
            return Event::None;
        }
        match item {
            Item::Back => {
                self.close();
                Event::Closed
            }
            Item::ConfigureKeys => Event::ConfigureKeys,
            _ => self.change(item, direction, true),
        }
    }

    /// The whole screen while open: background, MENU.CMP animations, title and
    /// items. Writes `pixels` (800x600 indices); the palette is the shell's.
    pub fn draw(&mut self, look: &Look<'_>, pixels: &mut [u8]) -> Result<()> {
        if !self.open {
            return Err(Error::Invalid("options screen is closed"));
        }
        if pixels.len() < PIXELS {
            return Err(Error::Invalid("options draw needs 480000 indices"));
        }
        let title_font = look.font(0x4e2f74)?;
        let item_font = look.font(0x4e2f75)?;
        let selected_font = look.font(0x4e2f76)?;
        let mut surface = self.background.to_vec();
        {
            // AffGame's menu body (shell_animation::menu_sprites) on its MENU.CMP bank.
            let frame = self.animation;
            let (mut scratch, mut cache) = (Scratch::default(), no_cache());
            let mut c = scratch_context(look, look.menu, &mut scratch, &mut surface, &mut cache);
            c.bob(35, 134, ((frame >> 1) & 15) + 1, false, None)?;
            c.bob(702, 0, ((frame >> 1) & 15) + 17, false, None)?;
            c.bob(528, 315, ((frame >> 1) & 7) + 33, false, None)?;
        }
        let mut lines = vec![(
            latin1(title(self.language)),
            [0, TITLE_TOP, WIDTH as i32, TITLE_TOP + 30],
            title_font,
            false,
        )];
        for (index, &item) in ITEMS.iter().enumerate() {
            let top = ROW_TOP + ROW_STEP * index as i32;
            // Unavailable items are engraved (shadow table) in the item font:
            // the original has no disabled menu style, so this is the port's.
            let font = if index == self.selected && self.enabled(index) {
                selected_font
            } else {
                item_font
            };
            lines.push((
                latin1(&text(self.language, item, &self.values)),
                [0, top, WIDTH as i32, top + ROW_HEIGHT],
                font,
                !self.enabled(index),
            ));
        }
        let (mut scratch, mut cache) = (Scratch::default(), no_cache());
        let mut c = scratch_context(look, look.fonts, &mut scratch, &mut surface, &mut cache);
        for (mut line, rect, font, engraved) in lines {
            line.push(0);
            if engraved {
                c.afftext_shadow(&line, rect, font)?;
            } else {
                c.afftext(&line, rect, font, [0, 0])?;
            }
        }
        pixels[..PIXELS].copy_from_slice(&surface);
        Ok(())
    }

    /// The button over a copy of the shell picture (main 2 / pause 16 only;
    /// other screens are left untouched). The selected font while hovered.
    pub fn overlay(&mut self, look: &Look<'_>, pixels: &mut [u8]) -> Result<bool> {
        if self.open || !has_button(look.mode()?) {
            return Ok(false);
        }
        if pixels.len() < PIXELS {
            return Err(Error::Invalid("options overlay needs 480000 indices"));
        }
        let font = look.font(if self.hover_button {
            0x4e2f76
        } else {
            0x4e2f75
        })?;
        let rect = self.button_rect(look)?;
        let mut surface = pixels[..PIXELS].to_vec();
        let (mut scratch, mut cache) = (Scratch::default(), no_cache());
        let mut c = scratch_context(look, look.fonts, &mut scratch, &mut surface, &mut cache);
        let mut line = latin1(title(self.language));
        line.push(0);
        c.afftext(
            &line,
            [
                rect[0] + HIT_MARGIN,
                BUTTON_TOP,
                rect[2] - HIT_MARGIN,
                BUTTON_TOP + ROW_HEIGHT,
            ],
            font,
            [0, 0],
        )?;
        pixels[..PIXELS].copy_from_slice(&surface);
        Ok(true)
    }
}

fn inside(r: [i32; 4], x: i32, y: i32) -> bool {
    x >= r[0] && x < r[2] && y >= r[1] && y < r[3]
}
/// Private scratch of one drawing: no saved background, no recorded text.
#[derive(Default)]
struct Scratch {
    background: Option<crate::render::SavedBackground>,
    text: Option<std::collections::VecDeque<crate::render::RecordedTextBoundary>>,
}
/// Menu drawing never reads the terrain cache (only terrain blits do).
fn no_cache() -> Cache {
    Cache {
        keys: Vec::new(),
        raw: Vec::new(),
        cursor: 0,
    }
}
fn scratch_context<'a>(
    look: &'a Look<'_>,
    assets: &'a Assets,
    scratch: &'a mut Scratch,
    surface: &'a mut Vec<u8>,
    cache: &'a mut Cache,
) -> Context<'a> {
    Context {
        background: &mut scratch.background,
        text_boundaries: &mut scratch.text,
        text_checked: 0,
        tick: None,
        m: look.memory,
        overrides: BTreeMap::new(),
        surface,
        cache,
        assets,
        observations: Vec::new(),
        audit: false,
        clip: [0, 0, 799, 599],
        camera: 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::render_assets::Sprite;

    fn sprite(width: usize, value: u8) -> Sprite {
        Sprite {
            width,
            height: 10,
            pixels: vec![value; width * 10],
            mask: vec![255; width * 10],
            raw: Vec::new(),
        }
    }
    fn assets(count: usize, width: usize) -> Assets {
        Assets {
            sprites: (0..count)
                .map(|i| sprite(width, (i % 200) as u8 + 1))
                .collect(),
            palette: [7; 768],
            shadow: [0; 256],
            light: [0; 256],
        }
    }
    fn memory(mode: u8) -> MemoriaJogo {
        MemoriaJogo::from_blocks([
            (0x4e1d90, vec![0; 16]),
            (0x4e2f74, vec![1, 3, 4, 0, mode]),
            (0x508f84, 9u32.to_le_bytes().to_vec()),
        ])
        .unwrap()
    }
    fn values() -> Values {
        Values {
            hotkeys: true,
            paths: false,
            difficulty: 1,
            speed: 1,
            available: ALL,
        }
    }
    fn click(o: &mut Options, look: &Look<'_>, [l, t, r, b]: [i32; 4]) -> Vec<(bool, Event)> {
        let (x, y) = ((l + r) / 2, (t + b) / 2);
        let mut i = Input {
            x,
            y,
            ..Input::default()
        };
        let mut out = vec![o.input(look, &i).unwrap()];
        i.held = true;
        out.push(o.input(look, &i).unwrap());
        i.released = true;
        out.push(o.input(look, &i).unwrap());
        out
    }

    #[test]
    fn options_button_navigation_and_values() {
        let (fonts, menu) = (assets(0x2835 + 224, 6), assets(40, 4));
        let m = memory(2);
        let look = Look {
            memory: &m,
            fonts: &fonts,
            menu: &menu,
        };
        let mut o = Options::new(values(), Language::PortugueseBr, &vec![3; PIXELS]).unwrap();
        // Closed: a click away from the button is the shell's, untouched.
        let away = [10, 10, 12, 12];
        assert!(click(&mut o, &look, away).iter().all(|&(c, _)| !c));
        // Keys are never taken while closed.
        let keys = Input {
            enter: true,
            ..Input::default()
        };
        assert_eq!(o.input(&look, &keys).unwrap(), (false, Event::None));
        let button = o.button_rect(&look).unwrap();
        assert!(
            button[1] >= ROW_TOP + ROW_STEP * 6,
            "below every original item"
        );
        let clicked = click(&mut o, &look, button);
        assert_eq!(clicked[1], (true, Event::None));
        assert_eq!(clicked[2], (true, Event::Opened));
        assert!(o.is_open());
        assert_eq!(o.selected(), Item::Hotkeys);
        // Enter toggles the selected item; Down skips nothing when all are offered.
        let enter = Input {
            enter: true,
            ..Input::default()
        };
        let down = Input {
            down: true,
            ..Input::default()
        };
        assert_eq!(
            o.input(&look, &enter).unwrap(),
            (true, Event::Changed(Item::Hotkeys))
        );
        assert!(!o.values.hotkeys);
        o.input(&look, &down).unwrap();
        assert_eq!(
            o.input(&look, &enter).unwrap(),
            (true, Event::ConfigureKeys)
        );
        o.input(&look, &down).unwrap();
        assert_eq!(
            o.input(&look, &enter).unwrap(),
            (true, Event::Changed(Item::Paths))
        );
        assert!(o.values.paths);
        o.input(&look, &down).unwrap();
        let right = Input {
            right: true,
            ..Input::default()
        };
        assert_eq!(
            o.input(&look, &right).unwrap(),
            (true, Event::Changed(Item::Difficulty))
        );
        assert_eq!(o.values.difficulty, 2);
        assert_eq!(
            o.input(&look, &right).unwrap(),
            (true, Event::None),
            "clamped"
        );
        assert_eq!(
            o.input(&look, &enter).unwrap(),
            (true, Event::Changed(Item::Difficulty))
        );
        assert_eq!(o.values.difficulty, 0, "Enter cycles");
        o.input(&look, &down).unwrap();
        for expected in [2, 3, 0, 1] {
            o.input(&look, &enter).unwrap();
            assert_eq!(o.values.speed, expected);
        }
        let left = Input {
            left: true,
            ..Input::default()
        };
        o.input(&look, &left).unwrap();
        assert_eq!(o.values.speed, 0);
        // A mouse click on Back closes; Escape closes too.
        let back = o.item_rect(&look, 5).unwrap();
        let clicked = click(&mut o, &look, back);
        assert_eq!(clicked[2], (true, Event::Closed));
        assert!(!o.is_open());
        o.open(&look).unwrap();
        let escape = Input {
            escape: true,
            ..Input::default()
        };
        assert_eq!(o.input(&look, &escape).unwrap(), (true, Event::Closed));
    }

    #[test]
    fn options_unavailable_items_are_skipped_and_frames_are_deterministic() {
        let (fonts, menu) = (assets(0x2835 + 224, 6), assets(40, 4));
        let m = memory(16);
        let look = Look {
            memory: &m,
            fonts: &fonts,
            menu: &menu,
        };
        let mut v = values();
        v.available = ALL & !(1 << Item::ConfigureKeys as u8);
        let mut o = Options::new(v, Language::PortugueseBr, &vec![3; PIXELS]).unwrap();
        o.open(&look).unwrap();
        let down = Input {
            down: true,
            ..Input::default()
        };
        o.input(&look, &down).unwrap();
        assert_eq!(o.selected(), Item::Paths, "ConfigureKeys skipped");
        // Hovering an unavailable item does not select it.
        let keys = o.item_rect(&look, 1).unwrap();
        let hover = Input {
            x: (keys[0] + keys[2]) / 2,
            y: keys[1] + 2,
            ..Input::default()
        };
        o.input(&look, &hover).unwrap();
        assert_eq!(o.selected(), Item::Paths);
        let (mut a, mut b) = (vec![0; PIXELS], vec![1; PIXELS]);
        o.draw(&look, &mut a).unwrap();
        o.draw(&look, &mut b).unwrap();
        assert_eq!(a, b);
        assert_ne!(a, vec![3; PIXELS]);
        // Pause 16 shows the button; result 19 does not, and nothing is taken there.
        o.close();
        let mut picture = vec![3; PIXELS];
        assert!(o.overlay(&look, &mut picture).unwrap());
        assert_ne!(picture, vec![3; PIXELS]);
        let m19 = memory(19);
        let look19 = Look {
            memory: &m19,
            fonts: &fonts,
            menu: &menu,
        };
        let mut untouched = vec![3; PIXELS];
        assert!(!o.overlay(&look19, &mut untouched).unwrap());
        assert_eq!(untouched, vec![3; PIXELS]);
        let button = o.button_rect(&look19).unwrap();
        assert!(click(&mut o, &look19, button).iter().all(|&(c, _)| !c));
        assert!(o.open(&look19).is_err());
        assert!(Options::new(
            Values {
                speed: 4,
                ..values()
            },
            Language::English,
            &vec![0; PIXELS]
        )
        .is_err());
    }
}
