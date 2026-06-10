use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
    Frame,
};

use crate::localization::{translate, Language};
use crate::ui::theme::Theme;

pub struct LogsViewState {
    #[allow(dead_code)]
    pub container_id: String,
    pub container_name: String,
    pub lines: Vec<String>,
    pub scroll: usize,
    pub follow: bool,
    pub lang: Language,
}

impl LogsViewState {
    pub fn new(container_id: String, container_name: String, lang: Language) -> Self {
        Self {
            container_id,
            container_name,
            lines: vec![],
            scroll: 0,
            follow: false,
            lang,
        }
    }

    pub fn scroll_up(&mut self) {
        self.scroll = self.scroll.saturating_sub(1);
        // Stop following when user scrolls up
        self.follow = false;
    }

    pub fn scroll_down(&mut self, visible_rows: usize) {
        let max = self.lines.len().saturating_sub(visible_rows);
        if self.scroll < max {
            self.scroll += 1;
        }
    }

    pub fn toggle_follow(&mut self) {
        self.follow = !self.follow;
        if self.follow {
            // Scroll to bottom
            self.scroll = self.lines.len();
        }
    }

    #[allow(dead_code)]
    pub fn push_line(&mut self, line: String, visible_rows: usize) {
        self.lines.push(line);
        if self.follow {
            self.scroll = self.lines.len().saturating_sub(visible_rows);
        }
    }
}

pub fn render(f: &mut Frame, area: Rect, state: &LogsViewState) {
    let theme = Theme::default_theme();

    let t = |key: &'static str| translate(key, state.lang);
    let title = format!(" {}: {} ", t("Logs"), state.container_name);
    let follow_indicator = if state.follow {
        Span::styled(
            format!(" [{}] ", t("Toggle auto-scroll")),
            Style::default()
                .fg(Color::Green)
                .add_modifier(Modifier::BOLD),
        )
    } else {
        Span::styled(format!(" [{}] ", t("Scroll logs")), Style::default().fg(theme.muted))
    };

    let block = Block::default()
        .title(Line::from(vec![
            Span::styled(
                title,
                Style::default()
                    .fg(theme.accent)
                    .add_modifier(Modifier::BOLD),
            ),
            follow_indicator,
        ]))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(theme.accent));
    let inner = block.inner(area);
    f.render_widget(block, area);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(1), Constraint::Length(1)])
        .split(inner);

    let log_area = chunks[0];
    let footer_area = chunks[1];
    let visible_rows = log_area.height as usize;

    // Log lines
    if state.lines.is_empty() {
        let msg = Line::from(Span::styled(
            format!("  {}…", t("NoContainers")),
            Style::default().fg(theme.muted),
        ));
        f.render_widget(Paragraph::new(vec![msg]), log_area);
    } else {
        let start = state.scroll.min(state.lines.len().saturating_sub(1));
        let end = (start + visible_rows).min(state.lines.len());

        let lines: Vec<Line> = state.lines[start..end]
            .iter()
            .map(|l| {
                let color = log_line_color(l);
                Line::from(Span::styled(l.clone(), Style::default().fg(color)))
            })
            .collect();

        f.render_widget(Paragraph::new(lines), log_area);
    }

    // Footer hint
    let hint = Line::from(vec![
        Span::styled(
            "[ESC] ",
            Style::default()
                .fg(theme.accent)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(format!("{}  ", t("BackLabel")), Style::default().fg(theme.muted)),
        Span::styled(
            "[F] ",
            Style::default()
                .fg(theme.accent)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(format!("{}  ", t("Toggle auto-scroll")), Style::default().fg(theme.muted)),
        Span::styled(
            "[↑↓] ",
            Style::default()
                .fg(theme.accent)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(format!("{}  ", t("Scroll logs")), Style::default().fg(theme.muted)),
        Span::styled(
            format!(
                " {}/{}",
                state.scroll + visible_rows.min(state.lines.len()),
                state.lines.len()
            ),
            Style::default().fg(theme.muted),
        ),
    ]);
    f.render_widget(Paragraph::new(hint), footer_area);
}

fn log_line_color(line: &str) -> Color {
    let lower = line.to_lowercase();
    if lower.contains("error") || lower.contains("fatal") || lower.contains("panic") {
        Color::Red
    } else if lower.contains("warn") {
        Color::Yellow
    } else if lower.contains("info") {
        Color::Cyan
    } else {
        Color::White
    }
}
