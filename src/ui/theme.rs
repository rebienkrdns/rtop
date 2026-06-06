use ratatui::style::Color;

#[allow(dead_code)]
pub struct Theme {
    pub ok: Color,
    pub warn: Color,
    pub crit: Color,
    pub accent: Color,
    pub text: Color,
    pub muted: Color,
    pub bg: Color,
}

impl Theme {
    pub fn default_theme() -> Self {
        Self {
            ok: Color::Green,
            warn: Color::Yellow,
            crit: Color::Red,
            accent: Color::Cyan,
            text: Color::White,
            muted: Color::Gray,
            bg: Color::Reset,
        }
    }

    #[allow(dead_code)]
    pub fn color_for_pct(pct: f64) -> Color {
        if pct > 85.0 {
            Color::Red
        } else if pct > 60.0 {
            Color::Yellow
        } else {
            Color::Green
        }
    }
}
