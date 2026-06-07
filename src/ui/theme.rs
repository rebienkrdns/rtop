use ratatui::style::Color;

#[allow(dead_code)]
pub struct Theme {
    pub ok: Color,          // #a5d566 — running / healthy / low usage
    pub warn: Color,        // #ebc06d — warning / medium usage
    pub crit: Color,        // #ffb4ab — error / critical usage
    pub accent: Color,      // #92e3da — titles, labels, accent text (bright teal)
    pub accent_dim: Color,  // #76c7be — borders, structural frames, cpu bar fill
    pub disk_fill: Color,   // #ffb4a6 — disk usage bar fill (coral)
    pub text: Color,        // #e3e1ed — primary text
    pub muted: Color,       // #889391 — secondary text, outline, sleeping status
    pub bg: Color,          // reset — transparent background
    pub selected_bg: Color, // #76c7be — selected row background
    pub selected_fg: Color, // #00201d — selected row foreground (very dark teal)
}

impl Theme {
    pub fn default_theme() -> Self {
        Self {
            ok:          Color::Rgb(165, 213, 102), // #a5d566
            warn:        Color::Rgb(235, 192, 109), // #ebc06d
            crit:        Color::Rgb(255, 180, 171), // #ffb4ab
            accent:      Color::Rgb(146, 227, 218), // #92e3da
            accent_dim:  Color::Rgb(118, 199, 190), // #76c7be
            disk_fill:   Color::Rgb(255, 180, 166), // #ffb4a6
            text:        Color::Rgb(227, 225, 237), // #e3e1ed
            muted:       Color::Rgb(136, 147, 145), // #889391
            bg:          Color::Reset,
            selected_bg: Color::Rgb(118, 199, 190), // #76c7be
            selected_fg: Color::Rgb(0, 32, 29),     // #00201d
        }
    }

    pub fn color_for_pct(pct: f64) -> Color {
        if pct > 85.0 {
            Color::Rgb(255, 180, 171) // crit  #ffb4ab
        } else if pct > 60.0 {
            Color::Rgb(235, 192, 109) // warn  #ebc06d
        } else {
            Color::Rgb(165, 213, 102) // ok    #a5d566
        }
    }
}
