pub mod theme;
pub mod views;
pub mod widgets;

use ratatui::layout::Alignment;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui::Frame;

use crate::app::{AppState, View};
use views::{container_detail, container_logs, main_view, process_detail};

pub fn draw(f: &mut Frame, state: &AppState) {
    let area = f.size();

    // 10.1 — Advertencia en terminales muy pequeñas
    if area.width < 80 || area.height < 24 {
        let msg = Paragraph::new(Line::from(vec![
            Span::styled(
                "⚠  Terminal muy pequeña (mínimo 80×24)",
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            ),
        ]))
        .block(Block::default().borders(Borders::ALL).title(" rtop "))
        .alignment(Alignment::Center);
        f.render_widget(msg, area);
        return;
    }

    // 10.5 — Modal de ayuda (se muestra sobre cualquier vista)
    if state.show_help {
        views::help_modal::render(f, area);
        return;
    }

    match state.current_view {
        View::Main => main_view::draw(f, state),
        View::ProcessDetail => {
            if let Some(proc) = state.selected_process() {
                process_detail::render(f, area, proc);
            } else {
                main_view::draw(f, state);
            }
        }
        View::ContainerDetail => {
            if let Some(c) = state.selected_container() {
                container_detail::render(f, area, c, state.confirm_action.as_ref());
            } else {
                main_view::draw(f, state);
            }
        }
        View::ContainerLogs => {
            if let Some(ref ls) = state.logs_state {
                container_logs::render(f, area, ls);
            } else {
                main_view::draw(f, state);
            }
        }
    }
}
