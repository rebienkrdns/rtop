pub mod theme;
pub mod views;
pub mod widgets;

use ratatui::Frame;

use crate::app::{AppState, View};
use views::{container_detail, container_logs, main_view, process_detail};

pub fn draw(f: &mut Frame, state: &AppState) {
    match state.current_view {
        View::Main => main_view::draw(f, state),
        View::ProcessDetail => {
            if let Some(proc) = state.selected_process() {
                let area = f.size();
                process_detail::render(f, area, proc);
            } else {
                main_view::draw(f, state);
            }
        }
        View::ContainerDetail => {
            if let Some(c) = state.selected_container() {
                let area = f.size();
                container_detail::render(f, area, c, state.confirm_action.as_ref());
            } else {
                main_view::draw(f, state);
            }
        }
        View::ContainerLogs => {
            if let Some(ref ls) = state.logs_state {
                let area = f.size();
                container_logs::render(f, area, ls);
            } else {
                main_view::draw(f, state);
            }
        }
    }
}
