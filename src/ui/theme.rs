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
}

impl ThemeMode {
    pub fn next(self) -> Self {
        match self {
            Self::Dark => Self::Light,
            Self::Light => Self::Nord,
            Self::Nord => Self::Matrix,
            Self::Matrix => Self::Sunset,
            Self::Sunset => Self::Dark,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::Dark => "Dark",
            Self::Light => "Light",
            Self::Nord => "Nord",
            Self::Matrix => "Matrix",
            Self::Sunset => "Sunset",
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
                ok: Color::Rgb(0, 255, 0),             // bright green
                warn: Color::Rgb(128, 255, 0),         // yellow-green
                crit: Color::Rgb(255, 0, 0),           // red
                accent: Color::Rgb(0, 255, 0),
                accent_dim: Color::Rgb(0, 180, 0),
                disk_fill: Color::Rgb(0, 200, 0),
                text: Color::Rgb(0, 255, 0),
                muted: Color::Rgb(0, 100, 0),
                bg: Color::Rgb(0, 0, 0),
                selected_bg: Color::Rgb(0, 255, 0),
                selected_fg: Color::Rgb(0, 0, 0),
            },
            ThemeMode::Sunset => Self {
                ok: Color::Rgb(244, 162, 97),          // sandy orange
                warn: Color::Rgb(231, 111, 81),         // burnt orange
                crit: Color::Rgb(230, 57, 70),          // red
                accent: Color::Rgb(233, 196, 106),     // yellow
                accent_dim: Color::Rgb(244, 162, 97),
                disk_fill: Color::Rgb(224, 122, 95),
                text: Color::Rgb(244, 241, 222),       // warm white
                muted: Color::Rgb(129, 178, 154),      // sage green/gray
                bg: Color::Reset,
                selected_bg: Color::Rgb(233, 196, 106),
                selected_fg: Color::Rgb(61, 64, 91),
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
