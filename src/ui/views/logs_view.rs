use crate::ui::theme::Theme;
use crate::ui::widgets::log_viewer::LogViewer;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;

pub struct LogsView;

impl LogsView {
    pub fn render(log_viewer: &LogViewer, area: Rect, buf: &mut Buffer, theme: &Theme) {
        log_viewer.render(area, buf, theme);
    }
}
