use ratatui::style::Color;
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicU8, Ordering};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ThemeMode {
    #[default]
    Dark,
    Light,
    Nord,
    Matrix,
    Sunset,
    Dracula,
    Gruvbox,
    TokyoNight,
}

impl ThemeMode {
    pub fn next(self) -> Self {
        match self {
            Self::Dark => Self::Light,
            Self::Light => Self::Nord,
            Self::Nord => Self::Matrix,
            Self::Matrix => Self::Sunset,
            Self::Sunset => Self::Dracula,
            Self::Dracula => Self::Gruvbox,
            Self::Gruvbox => Self::TokyoNight,
            Self::TokyoNight => Self::Dark,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::Dark => "Dark",
            Self::Light => "Light",
            Self::Nord => "Nord",
            Self::Matrix => "Matrix",
            Self::Sunset => "Sunset",
            Self::Dracula => "Dracula",
            Self::Gruvbox => "Gruvbox",
            Self::TokyoNight => "Tokyo Night",
        }
    }
}

static CURRENT_THEME: AtomicU8 = AtomicU8::new(0);

#[allow(dead_code)]
pub struct Theme {
    pub ok: Color,          // running / healthy / low usage
    pub warn: Color,        // warning / medium usage
    pub crit: Color,        // error / critical usage
    pub accent: Color,      // titles, labels, accent text (bright teal)
    pub accent_dim: Color,  // borders, structural frames, cpu bar fill
    pub disk_fill: Color,   // disk usage bar fill (coral)
    pub text: Color,        // primary text
    pub muted: Color,       // secondary text, outline, sleeping status
    pub bg: Color,          // reset — transparent background
    pub selected_bg: Color, // selected row background
    pub selected_fg: Color, // selected row foreground (very dark teal)
}

impl Theme {
    pub fn set_current_theme(mode: ThemeMode) {
        let val = match mode {
            ThemeMode::Dark => 0,
            ThemeMode::Light => 1,
            ThemeMode::Nord => 2,
            ThemeMode::Matrix => 3,
            ThemeMode::Sunset => 4,
            ThemeMode::Dracula => 5,
            ThemeMode::Gruvbox => 6,
            ThemeMode::TokyoNight => 7,
        };
        CURRENT_THEME.store(val, Ordering::Relaxed);
    }

    pub fn get_theme(mode: ThemeMode) -> Self {
        match mode {
            ThemeMode::Dark => Self {
                ok: Color::Rgb(165, 213, 102),         // #a5d566
                warn: Color::Rgb(235, 192, 109),       // #ebc06d
                crit: Color::Rgb(255, 180, 171),       // #ffb4ab
                accent: Color::Rgb(146, 227, 218),     // #92e3da
                accent_dim: Color::Rgb(118, 199, 190), // #76c7be
                disk_fill: Color::Rgb(255, 180, 166),  // #ffb4a6
                text: Color::Rgb(227, 225, 237),       // #e3e1ed
                muted: Color::Rgb(136, 147, 145),      // #889391
                bg: Color::Reset,
                selected_bg: Color::Rgb(118, 199, 190), // #76c7be
                selected_fg: Color::Rgb(0, 32, 29),     // #00201d
            },
            ThemeMode::Light => Self {
                ok: Color::Rgb(46, 125, 50),           // green
                warn: Color::Rgb(245, 124, 0),         // orange
                crit: Color::Rgb(211, 47, 47),         // red
                accent: Color::Rgb(25, 118, 210),      // blue
                accent_dim: Color::Rgb(30, 136, 229),  // lighter blue
                disk_fill: Color::Rgb(171, 71, 188),   // purple
                text: Color::Rgb(33, 33, 33),          // dark gray/black
                muted: Color::Rgb(117, 117, 117),      // gray
                bg: Color::Rgb(245, 245, 245),         // very light gray background
                selected_bg: Color::Rgb(25, 118, 210),
                selected_fg: Color::Rgb(255, 255, 255),
            },
            ThemeMode::Nord => Self {
                ok: Color::Rgb(163, 190, 140),         // nord green
                warn: Color::Rgb(235, 203, 139),       // nord yellow
                crit: Color::Rgb(191, 97, 106),        // nord red
                accent: Color::Rgb(136, 192, 208),     // nord frost cyan
                accent_dim: Color::Rgb(129, 161, 193), // nord frost blue
                disk_fill: Color::Rgb(180, 142, 173),  // nord purple
                text: Color::Rgb(236, 239, 244),       // nord white
                muted: Color::Rgb(76, 86, 106),        // nord gray
                bg: Color::Reset,
                selected_bg: Color::Rgb(136, 192, 208),
                selected_fg: Color::Rgb(46, 52, 64),
            },
            ThemeMode::Matrix => Self {
                ok: Color::Rgb(34, 179, 34),            // soft forest green
                warn: Color::Rgb(138, 179, 34),          // soft yellow-green
                crit: Color::Rgb(194, 48, 48),           // soft red
                accent: Color::Rgb(57, 194, 57),         // classic matrix green
                accent_dim: Color::Rgb(23, 115, 23),     // dark green
                disk_fill: Color::Rgb(46, 163, 46),      // muted green fill
                text: Color::Rgb(77, 219, 77),           // readable terminal green
                muted: Color::Rgb(23, 115, 23),          // dark muted green
                bg: Color::Rgb(10, 15, 10),              // very deep forest black
                selected_bg: Color::Rgb(34, 139, 34),
                selected_fg: Color::Rgb(255, 255, 255),
            },
            ThemeMode::Sunset => Self {
                ok: Color::Rgb(233, 150, 122),         // soft dark salmon
                warn: Color::Rgb(220, 118, 51),        // warm muted orange
                crit: Color::Rgb(203, 67, 53),          // soft brick red
                accent: Color::Rgb(244, 208, 111),     // pastel yellow
                accent_dim: Color::Rgb(220, 118, 51),
                disk_fill: Color::Rgb(211, 84, 0),
                text: Color::Rgb(250, 243, 224),       // cream warm white
                muted: Color::Rgb(147, 172, 149),      // soft sage
                bg: Color::Reset,
                selected_bg: Color::Rgb(244, 208, 111),
                selected_fg: Color::Rgb(46, 64, 87),
            },
            ThemeMode::Dracula => Self {
                ok: Color::Rgb(106, 224, 137),         // soft pastel green
                warn: Color::Rgb(240, 245, 170),       // soft pastel yellow
                crit: Color::Rgb(245, 120, 120),       // soft pastel red
                accent: Color::Rgb(189, 147, 249),     // dracula purple
                accent_dim: Color::Rgb(98, 114, 164),  // dracula blue-gray
                disk_fill: Color::Rgb(245, 150, 200),  // soft pink
                text: Color::Rgb(248, 248, 242),       // warm white
                muted: Color::Rgb(139, 155, 204),      // lighter blue-gray
                bg: Color::Reset,
                selected_bg: Color::Rgb(68, 71, 90),
                selected_fg: Color::Rgb(248, 248, 242),
            },
            ThemeMode::Gruvbox => Self {
                ok: Color::Rgb(184, 187, 38),          // gruvbox green
                warn: Color::Rgb(250, 189, 47),        // gruvbox yellow
                crit: Color::Rgb(251, 73, 52),         // gruvbox red
                accent: Color::Rgb(254, 128, 25),      // gruvbox orange
                accent_dim: Color::Rgb(214, 93, 14),   // gruvbox dark orange
                disk_fill: Color::Rgb(177, 98, 134),   // gruvbox purple
                text: Color::Rgb(235, 219, 178),       // gruvbox cream text
                muted: Color::Rgb(146, 131, 116),      // gruvbox gray
                bg: Color::Reset,
                selected_bg: Color::Rgb(80, 73, 69),   // gruvbox dark gray bg
                selected_fg: Color::Rgb(235, 219, 178),
            },
            ThemeMode::TokyoNight => Self {
                ok: Color::Rgb(158, 206, 106),         // tokyo night green
                warn: Color::Rgb(224, 175, 104),       // tokyo night yellow
                crit: Color::Rgb(247, 118, 142),       // tokyo night red
                accent: Color::Rgb(122, 162, 247),     // tokyo night blue
                accent_dim: Color::Rgb(61, 89, 161),   // tokyo night dark blue
                disk_fill: Color::Rgb(187, 154, 247),  // tokyo night purple
                text: Color::Rgb(169, 177, 214),       // tokyo night foreground gray
                muted: Color::Rgb(86, 95, 137),        // tokyo night muted blue-gray
                bg: Color::Reset,
                selected_bg: Color::Rgb(51, 70, 124),  // tokyo night dark selection bg
                selected_fg: Color::Rgb(192, 202, 245),
            },
        }
    }

    pub fn default_theme() -> Self {
        let theme_val = CURRENT_THEME.load(Ordering::Relaxed);
        let mode = match theme_val {
            0 => ThemeMode::Dark,
            1 => ThemeMode::Light,
            2 => ThemeMode::Nord,
            3 => ThemeMode::Matrix,
            4 => ThemeMode::Sunset,
            5 => ThemeMode::Dracula,
            6 => ThemeMode::Gruvbox,
            7 => ThemeMode::TokyoNight,
            _ => ThemeMode::Dark,
        };
        Self::get_theme(mode)
    }

    pub fn color_for_pct(pct: f64) -> Color {
        let theme = Self::default_theme();
        if pct > 85.0 {
            theme.crit
        } else if pct > 60.0 {
            theme.warn
        } else {
            theme.ok
        }
    }
}
