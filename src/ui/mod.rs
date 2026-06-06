pub mod theme;
pub mod views;

use ratatui::Frame;

use crate::app::AppState;
use views::main_view;

pub fn draw(f: &mut Frame, state: &AppState) {
    main_view::draw(f, state);
}
