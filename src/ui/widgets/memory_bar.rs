use bytesize::ByteSize;
use ratatui::{
    layout::Rect,
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Gauge, Paragraph},
    Frame,
};

use crate::models::MemoryData;
use crate::ui::theme::Theme;

pub fn render_with_loading(f: &mut Frame, area: Rect, mem: &MemoryData, data_loaded: bool) {
    if area.height < 2 {
        return;
    }
    let label_area = Rect { height: 1, ..area };
    let gauge_area = Rect {
        y: area.y + 1,
        height: 1,
        ..area
    };

    let theme = Theme::default_theme();
    let label = if data_loaded {
        Line::from(vec![
            Span::styled("RAM", Style::default().fg(theme.accent)),
            Span::styled(
                format!(
                    "  {:.1}%  {} / {}",
                    mem.usage_pct,
                    ByteSize(mem.used_bytes),
                    ByteSize(mem.total_bytes),
                ),
                Style::default().fg(theme.text),
            ),
        ])
    } else {
        Line::from(vec![
            Span::styled("RAM", Style::default().fg(theme.accent)),
            Span::styled("  [cargando…]", Style::default().fg(theme.muted)),
        ])
    };
    f.render_widget(Paragraph::new(label), label_area);

    let gauge = Gauge::default()
        .gauge_style(
            Style::default()
                .fg(Theme::color_for_pct(mem.usage_pct))
                .bg(Color::Rgb(51, 52, 61)), // #33343d surface-container-highest
        )
        .ratio((mem.usage_pct / 100.0).clamp(0.0, 1.0))
        .label("");
    f.render_widget(gauge, gauge_area);
}
