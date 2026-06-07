use bytesize::ByteSize;
use ratatui::{
    layout::Rect,
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Gauge, Paragraph},
    Frame,
};

use crate::models::DiskData;
use crate::ui::theme::Theme;

pub fn render(f: &mut Frame, area: Rect, disk: &DiskData) {
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
        Span::styled(disk.mount_point.as_str(), Style::default().fg(Color::Cyan)),
        Span::raw(format!(
            "  {:.1}%  {} / {}",
            disk.usage_pct,
            ByteSize(disk.used_bytes),
            ByteSize(disk.total_bytes),
        )),
    ]);
    f.render_widget(Paragraph::new(label), label_area);

    let gauge = Gauge::default()
        .gauge_style(
            Style::default()
                .fg(Theme::color_for_pct(disk.usage_pct))
                .bg(Color::DarkGray),
        )
        .ratio((disk.usage_pct / 100.0).clamp(0.0, 1.0))
        .label("");
    f.render_widget(gauge, gauge_area);
}
