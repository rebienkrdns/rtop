use bytesize::ByteSize;
use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
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
    let theme = Theme::default_theme();
    let label_area = Rect { height: 1, ..area };
    let gauge_area = Rect {
        y: area.y + 1,
        height: 1,
        ..area
    };

    let label = if data_loaded {
        let alert = mem.usage_pct >= 90.0;
        let color = Theme::color_for_pct(mem.usage_pct);
        Line::from(vec![
            Span::styled("RAM", Style::default().fg(theme.accent)),
            Span::styled(
                format!(
                    "  {:.1}%  {} / {}",
                    mem.usage_pct,
                    ByteSize(mem.used_bytes),
                    ByteSize(mem.total_bytes),
                ),
                Style::default().fg(if alert { color } else { theme.text }),
            ),
            if alert {
                Span::styled(
                    " ⚠",
                    Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
                )
            } else {
                Span::raw("")
            },
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
                .bg(Color::Rgb(51, 52, 61)),
        )
        .ratio((mem.usage_pct / 100.0).clamp(0.0, 1.0))
        .label("");
    f.render_widget(gauge, gauge_area);

    // Fila 2: Swap (si hay espacio y swap configurado)
    if area.height >= 3 && mem.swap_total > 0 {
        let swap_area = Rect {
            y: area.y + 2,
            height: 1,
            ..area
        };
        let swap_pct = if mem.swap_total > 0 {
            (mem.swap_used as f64 / mem.swap_total as f64) * 100.0
        } else {
            0.0
        };
        let alert = swap_pct >= 60.0;
        let color = Theme::color_for_pct(swap_pct);
        let swap_line = if data_loaded {
            Line::from(vec![
                Span::styled("SWP", Style::default().fg(theme.muted)),
                Span::styled(
                    format!(
                        "  {:.1}%  {} / {}",
                        swap_pct,
                        ByteSize(mem.swap_used),
                        ByteSize(mem.swap_total)
                    ),
                    Style::default().fg(if alert { color } else { theme.muted }),
                ),
                if alert {
                    Span::styled(
                        " ⚠",
                        Style::default()
                            .fg(Color::Yellow)
                            .add_modifier(Modifier::BOLD),
                    )
                } else {
                    Span::raw("")
                },
            ])
        } else {
            Line::from(Span::styled("SWP  —", Style::default().fg(theme.muted)))
        };
        f.render_widget(Paragraph::new(swap_line), swap_area);
    }
}
