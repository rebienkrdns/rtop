use ratatui::{
    layout::Rect,
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Gauge, Paragraph},
    Frame,
};

use crate::models::CpuData;
use crate::ui::theme::Theme;

pub fn render_with_loading(f: &mut Frame, area: Rect, cpu: &CpuData, data_loaded: bool) {
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
    let pct_str = if data_loaded {
        format!("{:.1}%", cpu.global_usage_pct)
    } else {
        "[cargando…]".to_string()
    };
    let label = Line::from(vec![
        Span::styled("CPU", Style::default().fg(theme.accent)),
        Span::styled(
            format!("  {}  {} cores", pct_str, cpu.core_count),
            Style::default().fg(theme.text),
        ),
    ]);
    f.render_widget(Paragraph::new(label), label_area);

    let gauge = Gauge::default()
        .gauge_style(
            Style::default()
                .fg(Theme::color_for_pct(cpu.global_usage_pct))
                .bg(Color::Rgb(51, 52, 61)), // #33343d surface-container-highest
        )
        .ratio((cpu.global_usage_pct / 100.0).clamp(0.0, 1.0))
        .label("");
    f.render_widget(gauge, gauge_area);
}
