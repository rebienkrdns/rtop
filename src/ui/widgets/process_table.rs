use bytesize::ByteSize;
use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
    Frame,
};

use crate::models::ProcessData;
use crate::ui::theme::Theme;

fn fmt_rate(rate: Option<f64>) -> String {
    match rate {
        Some(v) => format!("{}/s", ByteSize(v as u64)),
        None => "–".to_string(),
    }
}

pub fn render(f: &mut Frame, area: Rect, processes: &[ProcessData]) {
    let mut lines = vec![Line::from(vec![
        Span::styled(format!("{:<20}", "Nombre"), Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
        Span::styled(format!("{:>6}", "CPU%"), Style::default().fg(Color::Cyan)),
        Span::raw("  "),
        Span::styled(format!("{:>10}", "RAM"), Style::default().fg(Color::Cyan)),
        Span::raw("  "),
        Span::styled(format!("{:>10}", "Disco R"), Style::default().fg(Color::Cyan)),
        Span::raw("  "),
        Span::styled(format!("{:>10}", "Disco W"), Style::default().fg(Color::Cyan)),
    ])];

    for p in processes.iter().take(area.height.saturating_sub(1) as usize) {
        lines.push(Line::from(vec![
            Span::styled(format!("{:<20}", p.name), Style::default().fg(Color::White)),
            Span::styled(format!("{:>6.1}", p.cpu_pct), Style::default().fg(Theme::color_for_pct(p.cpu_pct))),
            Span::raw("  "),
            Span::styled(format!("{:>10}", ByteSize(p.memory_bytes)), Style::default().fg(Color::White)),
            Span::raw("  "),
            Span::styled(format!("{:>10}", fmt_rate(p.disk_read_per_sec)), Style::default().fg(Color::Blue)),
            Span::raw("  "),
            Span::styled(format!("{:>10}", fmt_rate(p.disk_write_per_sec)), Style::default().fg(Color::Yellow)),
        ]));
    }

    f.render_widget(Paragraph::new(lines), area);
}
