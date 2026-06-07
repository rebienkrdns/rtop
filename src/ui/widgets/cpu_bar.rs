use ratatui::{
    layout::Rect,
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Gauge, Paragraph},
    Frame,
};

use crate::models::CpuData;
use crate::ui::theme::Theme;

pub fn render(f: &mut Frame, area: Rect, cpu: &CpuData) {
    if area.height < 2 {
        return;
    }
    let label_area = Rect { height: 1, ..area };
    let gauge_area = Rect {
        y: area.y + 1,
        height: 1,
        ..area
    };

    let label = Line::from(vec![
        Span::styled("CPU", Style::default().fg(Color::Cyan)),
        Span::raw(format!(
            "  {:.1}%  {} cores",
            cpu.global_usage_pct, cpu.core_count
        )),
    ]);
    f.render_widget(Paragraph::new(label), label_area);

    let gauge = Gauge::default()
        .gauge_style(
            Style::default()
                .fg(Theme::color_for_pct(cpu.global_usage_pct))
                .bg(Color::DarkGray),
        )
        .ratio((cpu.global_usage_pct / 100.0).clamp(0.0, 1.0))
        .label("");
    f.render_widget(gauge, gauge_area);
}
