use ratatui::style::Color;
use std::collections::HashMap;
use std::fs;
use std::io;
use std::path::PathBuf;

pub const CONFIG_VERSION: u32 = 9;

#[derive(Clone, Debug)]
pub struct MenuItem {
    pub key: char,
    pub icon: String,
    pub text: String,
}

#[derive(Clone, Debug)]
pub struct KeyBind {
    pub key: String,
    pub action: String,
}

pub const THEME_NAMES: &[&str] = &[
    "terminal", "catppuccin", "gruvbox", "tokyonight",
    "nord", "everforest", "dracula", "one-dark", "kanagawa",
    "monokai", "ayu", "solarized", "rose-pine", "everblush",
    "oxocarbon", "melange", "nightfox",
    "catppuccin-latte", "gruvbox-light", "tokyonight-day",
    "rose-pine-dawn", "one-light", "everforest-light", "daylight",
    "discord-dark", "discord-light",
];

#[derive(Clone, Debug)]
pub struct Theme {
    pub name: String,
    pub bg: Color, pub fg: Color, pub accent: Color, pub dim: Color,
    pub border: Color, pub status_bg: Color, pub status_fg: Color,
    pub tab_active: Color, pub tree_dir: Color, pub error: Color,
    pub cursor_line: Color, pub visual: Color, pub search: Color,
}

impl Theme {
    pub fn by_name(name: &str) -> Theme {
        match name {
            "catppuccin" => Self::catppuccin(),
            "catppuccin-latte" => Self::catppuccin_latte(),
            "gruvbox" => Self::gruvbox(),
            "gruvbox-light" => Self::gruvbox_light(),
            "tokyonight" => Self::tokyonight(),
            "tokyonight-day" => Self::tokyonight_day(),
            "nord" => Self::nord(),
            "everforest" => Self::everforest(),
            "everforest-light" => Self::everforest_light(),
            "dracula" => Self::dracula(),
            "one-dark" => Self::one_dark(),
            "one-light" => Self::one_light(),
            "kanagawa" => Self::kanagawa(),
            "daylight" => Self::daylight(),
            "monokai" => Self::monokai(),
            "ayu" => Self::ayu(),
            "solarized" => Self::solarized(),
            "rose-pine" => Self::rose_pine(),
            "rose-pine-dawn" => Self::rose_pine_dawn(),
            "everblush" => Self::everblush(),
            "oxocarbon" => Self::oxocarbon(),
            "melange" => Self::melange(),
            "nightfox" => Self::nightfox(),
            "discord-dark" => Self::discord_dark(),
            "discord-light" => Self::discord_light(),
            _ => Self::terminal(),
        }
    }

    // ============================================================
    // terminal — relies on the user's ANSI palette (0..15), so it
    // matches whatever theme their terminal emulator uses.
    // ============================================================
    pub fn terminal() -> Self { Self {
        name: "terminal".into(),
        bg: Color::Reset,
        fg: Color::Reset,
        accent: Color::Indexed(6),      // cyan
        dim: Color::Indexed(8),         // bright black / gray
        border: Color::Indexed(8),      // bright black
        status_bg: Color::Indexed(0),   // black
        status_fg: Color::Indexed(7),   // white
        tab_active: Color::Indexed(3),  // yellow
        tree_dir: Color::Indexed(6),    // cyan
        error: Color::Indexed(1),       // red
        cursor_line: Color::Indexed(0), // black (subtle bg)
        visual: Color::Indexed(8),      // bright black
        search: Color::Indexed(3),      // yellow
    }}

    pub fn catppuccin() -> Self { Self {
        name: "catppuccin".into(),
        bg: Color::Rgb(30, 30, 46), fg: Color::Rgb(205, 214, 244),
        accent: Color::Rgb(203, 166, 247), dim: Color::Rgb(127, 132, 156),
        border: Color::Rgb(69, 71, 90), status_bg: Color::Rgb(49, 50, 68),
        status_fg: Color::Rgb(205, 214, 244), tab_active: Color::Rgb(137, 180, 250),
        tree_dir: Color::Rgb(137, 220, 235), error: Color::Rgb(243, 139, 168),
        cursor_line: Color::Rgb(41, 42, 60), visual: Color::Rgb(69, 71, 90),
        search: Color::Rgb(249, 226, 175),
    }}

    pub fn gruvbox() -> Self { Self {
        name: "gruvbox".into(),
        bg: Color::Rgb(40, 40, 40), fg: Color::Rgb(235, 219, 178),
        accent: Color::Rgb(250, 189, 47), dim: Color::Rgb(168, 153, 132),
        border: Color::Rgb(80, 73, 69), status_bg: Color::Rgb(60, 56, 54),
        status_fg: Color::Rgb(235, 219, 178), tab_active: Color::Rgb(184, 187, 38),
        tree_dir: Color::Rgb(131, 165, 152), error: Color::Rgb(251, 73, 52),
        cursor_line: Color::Rgb(50, 48, 47), visual: Color::Rgb(80, 73, 69),
        search: Color::Rgb(250, 189, 47),
    }}

    pub fn tokyonight() -> Self { Self {
        name: "tokyonight".into(),
        bg: Color::Rgb(26, 27, 38), fg: Color::Rgb(192, 202, 245),
        accent: Color::Rgb(187, 154, 247), dim: Color::Rgb(110, 119, 160),
        border: Color::Rgb(61, 68, 100), status_bg: Color::Rgb(36, 40, 59),
        status_fg: Color::Rgb(192, 202, 245), tab_active: Color::Rgb(122, 162, 247),
        tree_dir: Color::Rgb(125, 207, 255), error: Color::Rgb(247, 118, 142),
        cursor_line: Color::Rgb(33, 34, 48), visual: Color::Rgb(41, 46, 66),
        search: Color::Rgb(224, 175, 104),
    }}

    pub fn nord() -> Self { Self {
        name: "nord".into(),
        bg: Color::Rgb(46, 52, 64), fg: Color::Rgb(216, 222, 233),
        accent: Color::Rgb(136, 192, 208), dim: Color::Rgb(100, 112, 134),
        border: Color::Rgb(76, 86, 106), status_bg: Color::Rgb(59, 66, 82),
        status_fg: Color::Rgb(216, 222, 233), tab_active: Color::Rgb(143, 188, 187),
        tree_dir: Color::Rgb(129, 161, 193), error: Color::Rgb(191, 97, 106),
        cursor_line: Color::Rgb(53, 60, 75), visual: Color::Rgb(67, 76, 94),
        search: Color::Rgb(235, 203, 139),
    }}

    pub fn everforest() -> Self { Self {
        name: "everforest".into(),
        bg: Color::Rgb(45, 53, 59), fg: Color::Rgb(211, 198, 170),
        accent: Color::Rgb(167, 192, 128), dim: Color::Rgb(150, 162, 153),
        border: Color::Rgb(71, 82, 88), status_bg: Color::Rgb(61, 72, 78),
        status_fg: Color::Rgb(211, 198, 170), tab_active: Color::Rgb(230, 152, 117),
        tree_dir: Color::Rgb(127, 187, 179), error: Color::Rgb(230, 126, 128),
        cursor_line: Color::Rgb(53, 62, 68), visual: Color::Rgb(80, 92, 98),
        search: Color::Rgb(230, 200, 120),
    }}

    pub fn dracula() -> Self { Self {
        name: "dracula".into(),
        bg: Color::Rgb(40, 42, 54), fg: Color::Rgb(248, 248, 242),
        accent: Color::Rgb(189, 147, 249), dim: Color::Rgb(130, 145, 190),
        border: Color::Rgb(68, 71, 90), status_bg: Color::Rgb(68, 71, 90),
        status_fg: Color::Rgb(248, 248, 242), tab_active: Color::Rgb(80, 250, 123),
        tree_dir: Color::Rgb(139, 233, 253), error: Color::Rgb(255, 85, 85),
        cursor_line: Color::Rgb(52, 55, 70), visual: Color::Rgb(68, 71, 90),
        search: Color::Rgb(241, 250, 140),
    }}

    pub fn one_dark() -> Self { Self {
        name: "one-dark".into(),
        bg: Color::Rgb(40, 44, 52), fg: Color::Rgb(171, 178, 191),
        accent: Color::Rgb(97, 175, 239), dim: Color::Rgb(120, 128, 145),
        border: Color::Rgb(62, 68, 81), status_bg: Color::Rgb(33, 37, 43),
        status_fg: Color::Rgb(171, 178, 191), tab_active: Color::Rgb(152, 195, 121),
        tree_dir: Color::Rgb(97, 175, 239), error: Color::Rgb(224, 108, 117),
        cursor_line: Color::Rgb(47, 51, 60), visual: Color::Rgb(62, 68, 81),
        search: Color::Rgb(229, 192, 123),
    }}

    pub fn kanagawa() -> Self { Self {
        name: "kanagawa".into(),
        bg: Color::Rgb(31, 31, 40), fg: Color::Rgb(220, 215, 186),
        accent: Color::Rgb(126, 156, 216), dim: Color::Rgb(140, 140, 130),
        border: Color::Rgb(84, 84, 109), status_bg: Color::Rgb(42, 42, 55),
        status_fg: Color::Rgb(220, 215, 186), tab_active: Color::Rgb(255, 158, 61),
        tree_dir: Color::Rgb(127, 180, 202), error: Color::Rgb(210, 126, 153),
        cursor_line: Color::Rgb(38, 38, 50), visual: Color::Rgb(54, 54, 70),
        search: Color::Rgb(230, 195, 132),
    }}

    pub fn monokai() -> Self { Self {
        name: "monokai".into(),
        bg: Color::Rgb(39, 40, 34), fg: Color::Rgb(248, 248, 242),
        accent: Color::Rgb(249, 38, 114), dim: Color::Rgb(145, 140, 120),
        border: Color::Rgb(73, 72, 62), status_bg: Color::Rgb(39, 40, 34),
        status_fg: Color::Rgb(248, 248, 242), tab_active: Color::Rgb(166, 226, 46),
        tree_dir: Color::Rgb(102, 217, 239), error: Color::Rgb(249, 38, 114),
        cursor_line: Color::Rgb(50, 51, 44), visual: Color::Rgb(73, 72, 62),
        search: Color::Rgb(230, 219, 116),
    }}

    pub fn ayu() -> Self { Self {
        name: "ayu".into(),
        bg: Color::Rgb(10, 14, 20), fg: Color::Rgb(203, 204, 198),
        accent: Color::Rgb(255, 180, 84), dim: Color::Rgb(120, 132, 145),
        border: Color::Rgb(29, 42, 53), status_bg: Color::Rgb(13, 16, 23),
        status_fg: Color::Rgb(203, 204, 198), tab_active: Color::Rgb(57, 186, 230),
        tree_dir: Color::Rgb(83, 189, 235), error: Color::Rgb(240, 113, 120),
        cursor_line: Color::Rgb(17, 20, 27), visual: Color::Rgb(29, 42, 53),
        search: Color::Rgb(255, 180, 84),
    }}

    pub fn solarized() -> Self { Self {
        name: "solarized".into(),
        bg: Color::Rgb(0, 43, 54), fg: Color::Rgb(131, 148, 150),
        accent: Color::Rgb(38, 139, 210), dim: Color::Rgb(110, 130, 137),
        border: Color::Rgb(7, 54, 66), status_bg: Color::Rgb(7, 54, 66),
        status_fg: Color::Rgb(147, 161, 161), tab_active: Color::Rgb(133, 153, 0),
        tree_dir: Color::Rgb(42, 161, 152), error: Color::Rgb(220, 50, 47),
        cursor_line: Color::Rgb(4, 50, 62), visual: Color::Rgb(7, 54, 66),
        search: Color::Rgb(181, 137, 0),
    }}

    pub fn rose_pine() -> Self { Self {
        name: "rose-pine".into(),
        bg: Color::Rgb(25, 23, 36), fg: Color::Rgb(224, 222, 244),
        accent: Color::Rgb(196, 167, 231), dim: Color::Rgb(130, 126, 158),
        border: Color::Rgb(38, 35, 58), status_bg: Color::Rgb(31, 29, 46),
        status_fg: Color::Rgb(224, 222, 244), tab_active: Color::Rgb(235, 188, 186),
        tree_dir: Color::Rgb(156, 207, 216), error: Color::Rgb(235, 111, 146),
        cursor_line: Color::Rgb(31, 29, 46), visual: Color::Rgb(38, 35, 58),
        search: Color::Rgb(246, 193, 119),
    }}

    pub fn everblush() -> Self { Self {
        name: "everblush".into(),
        bg: Color::Rgb(20, 26, 27), fg: Color::Rgb(218, 218, 218),
        accent: Color::Rgb(103, 179, 178), dim: Color::Rgb(122, 140, 140),
        border: Color::Rgb(42, 51, 53), status_bg: Color::Rgb(30, 37, 38),
        status_fg: Color::Rgb(218, 218, 218), tab_active: Color::Rgb(140, 197, 116),
        tree_dir: Color::Rgb(103, 179, 178), error: Color::Rgb(232, 116, 116),
        cursor_line: Color::Rgb(26, 33, 34), visual: Color::Rgb(42, 51, 53),
        search: Color::Rgb(229, 194, 118),
    }}

    pub fn oxocarbon() -> Self { Self {
        name: "oxocarbon".into(),
        bg: Color::Rgb(22, 22, 22), fg: Color::Rgb(242, 244, 248),
        accent: Color::Rgb(130, 177, 255), dim: Color::Rgb(100, 108, 122),
        border: Color::Rgb(51, 51, 51), status_bg: Color::Rgb(38, 38, 38),
        status_fg: Color::Rgb(242, 244, 248), tab_active: Color::Rgb(66, 190, 133),
        tree_dir: Color::Rgb(130, 177, 255), error: Color::Rgb(255, 123, 114),
        cursor_line: Color::Rgb(30, 30, 30), visual: Color::Rgb(51, 51, 51),
        search: Color::Rgb(255, 193, 87),
    }}

    pub fn melange() -> Self { Self {
        name: "melange".into(),
        bg: Color::Rgb(41, 36, 34), fg: Color::Rgb(236, 224, 211),
        accent: Color::Rgb(255, 180, 84), dim: Color::Rgb(150, 130, 118),
        border: Color::Rgb(65, 57, 53), status_bg: Color::Rgb(54, 48, 45),
        status_fg: Color::Rgb(236, 224, 211), tab_active: Color::Rgb(165, 200, 130),
        tree_dir: Color::Rgb(135, 190, 220), error: Color::Rgb(224, 100, 100),
        cursor_line: Color::Rgb(48, 42, 40), visual: Color::Rgb(65, 57, 53),
        search: Color::Rgb(230, 195, 110),
    }}

    pub fn nightfox() -> Self { Self {
        name: "nightfox".into(),
        bg: Color::Rgb(19, 26, 36), fg: Color::Rgb(205, 214, 244),
        accent: Color::Rgb(113, 149, 226), dim: Color::Rgb(110, 120, 148),
        border: Color::Rgb(41, 54, 75), status_bg: Color::Rgb(23, 32, 44),
        status_fg: Color::Rgb(205, 214, 244), tab_active: Color::Rgb(157, 192, 255),
        tree_dir: Color::Rgb(99, 205, 218), error: Color::Rgb(215, 92, 125),
        cursor_line: Color::Rgb(27, 35, 48), visual: Color::Rgb(41, 54, 75),
        search: Color::Rgb(240, 200, 120),
    }}

    pub fn catppuccin_latte() -> Self { Self {
        name: "catppuccin-latte".into(),
        bg: Color::Rgb(239, 241, 245), fg: Color::Rgb(76, 79, 105),
        accent: Color::Rgb(136, 57, 239), dim: Color::Rgb(140, 143, 161),
        border: Color::Rgb(188, 192, 204), status_bg: Color::Rgb(220, 224, 232),
        status_fg: Color::Rgb(76, 79, 105), tab_active: Color::Rgb(30, 102, 245),
        tree_dir: Color::Rgb(4, 165, 229), error: Color::Rgb(210, 15, 57),
        cursor_line: Color::Rgb(230, 233, 239), visual: Color::Rgb(204, 208, 218),
        search: Color::Rgb(223, 142, 29),
    }}

    pub fn gruvbox_light() -> Self { Self {
        name: "gruvbox-light".into(),
        bg: Color::Rgb(251, 241, 199), fg: Color::Rgb(60, 56, 54),
        accent: Color::Rgb(181, 118, 20), dim: Color::Rgb(146, 131, 116),
        border: Color::Rgb(213, 196, 161), status_bg: Color::Rgb(235, 219, 178),
        status_fg: Color::Rgb(60, 56, 54), tab_active: Color::Rgb(121, 116, 14),
        tree_dir: Color::Rgb(7, 102, 120), error: Color::Rgb(157, 0, 6),
        cursor_line: Color::Rgb(242, 229, 188), visual: Color::Rgb(213, 196, 161),
        search: Color::Rgb(215, 153, 33),
    }}

    pub fn tokyonight_day() -> Self { Self {
        name: "tokyonight-day".into(),
        bg: Color::Rgb(225, 226, 231), fg: Color::Rgb(55, 96, 191),
        accent: Color::Rgb(152, 84, 241), dim: Color::Rgb(132, 140, 168),
        border: Color::Rgb(196, 200, 212), status_bg: Color::Rgb(204, 208, 218),
        status_fg: Color::Rgb(55, 96, 191), tab_active: Color::Rgb(46, 125, 233),
        tree_dir: Color::Rgb(0, 113, 151), error: Color::Rgb(245, 42, 66),
        cursor_line: Color::Rgb(214, 216, 222), visual: Color::Rgb(196, 200, 212),
        search: Color::Rgb(143, 94, 21),
    }}

    pub fn rose_pine_dawn() -> Self { Self {
        name: "rose-pine-dawn".into(),
        bg: Color::Rgb(250, 244, 237), fg: Color::Rgb(87, 82, 121),
        accent: Color::Rgb(144, 122, 169), dim: Color::Rgb(152, 147, 165),
        border: Color::Rgb(223, 218, 217), status_bg: Color::Rgb(242, 233, 226),
        status_fg: Color::Rgb(87, 82, 121), tab_active: Color::Rgb(40, 105, 131),
        tree_dir: Color::Rgb(86, 148, 159), error: Color::Rgb(180, 99, 122),
        cursor_line: Color::Rgb(242, 233, 226), visual: Color::Rgb(223, 218, 217),
        search: Color::Rgb(234, 157, 52),
    }}

    pub fn one_light() -> Self { Self {
        name: "one-light".into(),
        bg: Color::Rgb(250, 250, 250), fg: Color::Rgb(56, 58, 66),
        accent: Color::Rgb(64, 120, 242), dim: Color::Rgb(160, 161, 167),
        border: Color::Rgb(212, 212, 214), status_bg: Color::Rgb(234, 234, 234),
        status_fg: Color::Rgb(56, 58, 66), tab_active: Color::Rgb(64, 120, 242),
        tree_dir: Color::Rgb(1, 132, 188), error: Color::Rgb(228, 86, 73),
        cursor_line: Color::Rgb(238, 238, 238), visual: Color::Rgb(212, 212, 214),
        search: Color::Rgb(193, 132, 1),
    }}

    pub fn everforest_light() -> Self { Self {
        name: "everforest-light".into(),
        bg: Color::Rgb(253, 246, 227), fg: Color::Rgb(92, 106, 114),
        accent: Color::Rgb(141, 161, 1), dim: Color::Rgb(147, 149, 134),
        border: Color::Rgb(230, 224, 207), status_bg: Color::Rgb(239, 230, 210),
        status_fg: Color::Rgb(92, 106, 114), tab_active: Color::Rgb(53, 167, 124),
        tree_dir: Color::Rgb(58, 148, 197), error: Color::Rgb(248, 85, 82),
        cursor_line: Color::Rgb(239, 230, 210), visual: Color::Rgb(230, 224, 207),
        search: Color::Rgb(230, 152, 55),
    }}

    pub fn daylight() -> Self { Self {
        name: "daylight".into(),
        bg: Color::Rgb(250, 250, 250), fg: Color::Rgb(40, 40, 40),
        accent: Color::Rgb(64, 96, 200), dim: Color::Rgb(150, 150, 150),
        border: Color::Rgb(210, 210, 210), status_bg: Color::Rgb(235, 235, 235),
        status_fg: Color::Rgb(40, 40, 40), tab_active: Color::Rgb(64, 96, 200),
        tree_dir: Color::Rgb(40, 120, 180), error: Color::Rgb(200, 60, 60),
        cursor_line: Color::Rgb(242, 242, 242), visual: Color::Rgb(210, 220, 245),
        search: Color::Rgb(255, 235, 100),
    }}

    pub fn discord_dark() -> Self { Self {
        name: "discord-dark".into(),
        bg: Color::Rgb(54, 57, 63), fg: Color::Rgb(220, 221, 222),
        accent: Color::Rgb(114, 137, 218), dim: Color::Rgb(114, 118, 125),
        border: Color::Rgb(66, 69, 76), status_bg: Color::Rgb(47, 49, 54),
        status_fg: Color::Rgb(220, 221, 222), tab_active: Color::Rgb(88, 101, 242),
        tree_dir: Color::Rgb(87, 242, 135), error: Color::Rgb(237, 66, 69),
        cursor_line: Color::Rgb(66, 70, 77), visual: Color::Rgb(74, 78, 87),
        search: Color::Rgb(254, 231, 92),
    }}

    pub fn discord_light() -> Self { Self {
        name: "discord-light".into(),
        bg: Color::Rgb(255, 255, 255), fg: Color::Rgb(46, 51, 56),
        accent: Color::Rgb(88, 101, 242), dim: Color::Rgb(120, 125, 130),
        border: Color::Rgb(232, 234, 237), status_bg: Color::Rgb(242, 243, 245),
        status_fg: Color::Rgb(46, 51, 56), tab_active: Color::Rgb(88, 101, 242),
        tree_dir: Color::Rgb(35, 165, 90), error: Color::Rgb(218, 55, 60),
        cursor_line: Color::Rgb(245, 246, 248), visual: Color::Rgb(222, 225, 232),
        search: Color::Rgb(200, 160, 0),
    }}
}

pub struct Config {
    pub version: u32,
    pub theme: String,
    pub tree: bool,
    pub tree_width: u16,
    pub tree_follow: bool,
    pub mouse: bool,
    pub tagline: String,
    pub logo: Vec<String>,
    pub menu: Vec<MenuItem>,
    pub colors: HashMap<String, Color>,
    pub term_side_width: u16,
    pub term_bottom_height: u16,
    pub keybinds: Vec<KeyBind>,
    pub music_enabled: bool,
    pub path: PathBuf,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            version: CONFIG_VERSION,
            theme: "catppuccin".into(),
            tree: true,
            tree_width: 34,
            tree_follow: true,
            mouse: true,
            tagline: "a lightweight modal editor".into(),
            logo: default_logo(),
            menu: default_menu(),
            colors: HashMap::new(),
            term_side_width: 44,
            term_bottom_height: 12,
            keybinds: default_keybinds(),
            music_enabled: true,
            path: default_path(),
        }
    }
}

fn default_path() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".into());
    PathBuf::from(home).join(".config").join("anvil").join("config.ini")
}

fn default_logo() -> Vec<String> {
    vec![
        r" █████  ███    ██ ██    ██ ██ ██      ".into(),
        r"██   ██ ████   ██ ██    ██ ██ ██      ".into(),
        r"███████ ██ ██  ██ ██    ██ ██ ██      ".into(),
        r"██   ██ ██  ██ ██  ██  ██  ██ ██      ".into(),
        r"██   ██ ██   ████   ████   ██ ███████ ".into(),
    ]
}

fn default_menu() -> Vec<MenuItem> {
    vec![
        MenuItem { key: 'n', icon: "\u{f15b}".into(), text: "New File".into() },
        MenuItem { key: 'f', icon: "\u{f002}".into(), text: "Find File".into() },
        MenuItem { key: 'F', icon: "\u{f015}".into(), text: "Find (home)".into() },
        MenuItem { key: 'g', icon: "\u{f002}".into(), text: "Find Text".into() },
        MenuItem { key: 'e', icon: "\u{f07b}".into(), text: "File Tree".into() },
        MenuItem { key: 't', icon: "\u{e22b}".into(), text: "Theme".into() },
        MenuItem { key: 'h', icon: "\u{f059}".into(), text: "Help".into() },
        MenuItem { key: 'q', icon: "\u{f08b}".into(), text: "Quit".into() },
    ]
}

fn default_keybinds() -> Vec<KeyBind> { Vec::new() }

fn parse_color(s: &str) -> Option<Color> {
    let s = s.trim();
    if let Ok(n) = s.parse::<u8>() { return Some(Color::Indexed(n)); }
    if let Some(hex) = s.strip_prefix('#') {
        if hex.len() == 6 {
            let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
            let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
            let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
            return Some(Color::Rgb(r, g, b));
        }
    }
    match s.to_lowercase().as_str() {
        "black" => Some(Color::Black),
        "red" => Some(Color::Red),
        "green" => Some(Color::Green),
        "yellow" => Some(Color::Yellow),
        "blue" => Some(Color::Blue),
        "magenta" => Some(Color::Magenta),
        "cyan" => Some(Color::Cyan),
        "gray" | "grey" => Some(Color::Gray),
        "darkgray" => Some(Color::DarkGray),
        "white" => Some(Color::White),
        "reset" => Some(Color::Reset),
        _ => None,
    }
}

fn color_to_hex(c: Color) -> String {
    match c {
        Color::Rgb(r, g, b) => format!("#{:02x}{:02x}{:02x}", r, g, b),
        Color::Indexed(n) => n.to_string(),
        _ => "reset".into(),
    }
}

impl Config {
    pub fn load() -> Self {
        let mut cfg = Config::default();
        let path = default_path();
        if let Ok(content) = fs::read_to_string(&path) {
            cfg.parse(&content);
        }
        cfg
    }

    pub fn reload(&mut self) {
        if let Ok(content) = fs::read_to_string(&self.path) {
            let mut fresh = Config::default();
            fresh.path = self.path.clone();
            fresh.parse(&content);
            *self = fresh;
        }
    }

    pub fn save(&self) -> io::Result<()> {
        if let Some(dir) = self.path.parent() {
            fs::create_dir_all(dir)?;
        }
        let mut out = String::new();
        out.push_str("; anvil config - autosaved\n");
        out.push_str(&format!("version            = {}\n", self.version));
        out.push_str(&format!("theme              = {}\n", self.theme));
        out.push_str(&format!("tree               = {}\n", self.tree));
        out.push_str(&format!("tree_width         = {}\n", self.tree_width));
        out.push_str(&format!("tree_follow        = {}\n", self.tree_follow));
        out.push_str(&format!("mouse              = {}\n", self.mouse));
        out.push_str(&format!("term_side_width    = {}\n", self.term_side_width));
        out.push_str(&format!("term_bottom_height = {}\n", self.term_bottom_height));
        out.push_str(&format!("tagline            = {}\n", self.tagline));
        out.push_str(&format!("music_enabled      = {}\n", self.music_enabled));
        out.push('\n');
        out.push_str("[logo]\n");
        for l in &self.logo { out.push_str(l); out.push('\n'); }
        out.push_str("[/logo]\n\n");
        for m in &self.menu {
            out.push_str(&format!("menu = {} | {} | {}\n", m.key, m.icon, m.text));
        }
        out.push('\n');
        out.push_str("; custom keybinds (key.<binding> = <action>)\n");
        out.push_str("; actions: term, term:<dir>, sterm, sterm:<dir>,\n");
        out.push_str(";          run:<cmd>, edit:<path>, cd:<dir>, new, save\n");
        for kb in &self.keybinds {
            out.push_str(&format!("key.{} = {}\n", kb.key, kb.action));
        }
        if !self.colors.is_empty() {
            out.push('\n');
            for (k, v) in &self.colors {
                out.push_str(&format!("color.{} = {}\n", k, color_to_hex(*v)));
            }
        }
        fs::write(&self.path, out)
    }

    fn parse(&mut self, content: &str) {
        let mut in_logo = false;
        let mut logo_lines: Vec<String> = Vec::new();
        let mut menu_lines: Vec<MenuItem> = Vec::new();
        let mut colors: HashMap<String, Color> = HashMap::new();
        let mut keybinds: Vec<KeyBind> = Vec::new();

        for raw in content.lines() {
            let line = raw;
            let trimmed = line.trim();

            if in_logo {
                if trimmed == "[/logo]" { in_logo = false; continue; }
                if !trimmed.starts_with(';') { logo_lines.push(line.to_string()); }
                continue;
            }
            if trimmed == "[logo]" { in_logo = true; continue; }
            if trimmed.is_empty() || trimmed.starts_with(';') { continue; }

            if let Some(eq) = trimmed.find('=') {
                let key = trimmed[..eq].trim().to_string();
                let val = trimmed[eq + 1..].trim().to_string();
                match key.as_str() {
                    "theme" => self.theme = val,
                    "tree" => self.tree = val.eq_ignore_ascii_case("true"),
                    "tree_width" => if let Ok(n) = val.parse() { self.tree_width = n; },
                    "tree_follow" => self.tree_follow = val.eq_ignore_ascii_case("true"),
                    "mouse" => self.mouse = val.eq_ignore_ascii_case("true"),
                    "term_side_width" => if let Ok(n) = val.parse() { self.term_side_width = n; },
                    "term_bottom_height" => if let Ok(n) = val.parse() { self.term_bottom_height = n; },
                    "music_enabled" => self.music_enabled = val.eq_ignore_ascii_case("true"),
                    "tagline" => self.tagline = val,
                    "logo" => { logo_lines = vec![val]; }
                    "menu" => {
                        let parts: Vec<&str> = val.split('|').map(|s| s.trim()).collect();
                        if parts.len() >= 3 {
                            if let Some(c) = parts[0].chars().next() {
                                menu_lines.push(MenuItem {
                                    key: c,
                                    icon: parts[1].to_string(),
                                    text: parts[2].to_string(),
                                });
                            }
                        }
                    }
                    k if k.starts_with("key.") => {
                        let binding = k.trim_start_matches("key.").to_string();
                        keybinds.push(KeyBind { key: binding, action: val });
                    }
                    k if k.starts_with("color.") => {
                        let name = k.trim_start_matches("color.").to_string();
                        if let Some(c) = parse_color(&val) { colors.insert(name, c); }
                    }
                    _ => {}
                }
            }
        }

        if !logo_lines.is_empty() { self.logo = logo_lines; }
        if !menu_lines.is_empty() { self.menu = menu_lines; }
        self.colors = colors;
        self.keybinds = keybinds;
        self.version = CONFIG_VERSION;
    }

    pub fn apply_color_overrides(&self, theme: &mut Theme) {
        if let Some(c) = self.colors.get("bg") { theme.bg = *c; }
        if let Some(c) = self.colors.get("fg") { theme.fg = *c; }
        if let Some(c) = self.colors.get("accent") { theme.accent = *c; }
        if let Some(c) = self.colors.get("dim") { theme.dim = *c; }
        if let Some(c) = self.colors.get("border") { theme.border = *c; }
        if let Some(c) = self.colors.get("status_bg") { theme.status_bg = *c; }
        if let Some(c) = self.colors.get("status_fg") { theme.status_fg = *c; }
        if let Some(c) = self.colors.get("tab_active") { theme.tab_active = *c; }
        if let Some(c) = self.colors.get("tree_dir") { theme.tree_dir = *c; }
        if let Some(c) = self.colors.get("error") { theme.error = *c; }
        if let Some(c) = self.colors.get("cursor_line") { theme.cursor_line = *c; }
        if let Some(c) = self.colors.get("visual") { theme.visual = *c; }
        if let Some(c) = self.colors.get("search") { theme.search = *c; }
    }

    pub fn save_default_if_missing(&self) {
        if self.path.exists() { return; }
        let _ = self.save();
    }
}