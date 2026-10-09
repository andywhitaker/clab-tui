use crate::ui::theme::Theme;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use std::time::{Duration, Instant};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToastKind {
    Info,
    Success,
    Warning,
    Error,
}

#[derive(Debug, Clone)]
pub struct Toast {
    pub kind: ToastKind,
    pub message: String,
    pub created_at: Instant,
    pub duration: Duration,
}

#[derive(Debug, Clone, Default)]
pub struct ToastManager {
    pub toasts: Vec<Toast>,
}

impl ToastManager {
    pub fn new() -> Self {
        Self { toasts: Vec::new() }
    }

    pub fn add(&mut self, kind: ToastKind, message: impl Into<String>) {
        self.toasts.push(Toast {
            kind,
            message: message.into(),
            created_at: Instant::now(),
            duration: Duration::from_secs(4),
        });
    }

    pub fn info(&mut self, message: impl Into<String>) {
        self.add(ToastKind::Info, message);
    }

    pub fn success(&mut self, message: impl Into<String>) {
        self.add(ToastKind::Success, message);
    }

    pub fn warning(&mut self, message: impl Into<String>) {
        self.add(ToastKind::Warning, message);
    }

    pub fn error(&mut self, message: impl Into<String>) {
        self.add(ToastKind::Error, message);
    }

    pub fn tick(&mut self) {
        self.toasts.retain(|t| t.created_at.elapsed() < t.duration);
    }

    pub fn render(&self, area: Rect, buf: &mut Buffer, theme: &Theme) {
        if self.toasts.is_empty() {
            return;
        }

        // Render up to 3 toasts stacked at top right
        let toast_width = 46.min(area.width.saturating_sub(4));
        let mut top_y = area.y + 1;

        for toast in self.toasts.iter().rev().take(3) {
            let (icon, color) = match toast.kind {
                ToastKind::Info => ("ℹ", theme.info),
                ToastKind::Success => ("✔", theme.success),
                ToastKind::Warning => ("⚠", theme.warning),
                ToastKind::Error => ("✖", theme.error),
            };

            let left_x = area.right().saturating_sub(toast_width + 2);
            let toast_rect = Rect::new(left_x, top_y, toast_width, 3);

            if toast_rect.bottom() >= area.bottom() {
                break;
            }

            let border_style = Style::default().fg(color).bg(theme.panel_bg);
            let text_style = Style::default().fg(theme.text_primary).bg(theme.panel_bg);

            // Draw box
            for y in toast_rect.y..toast_rect.bottom() {
                for x in toast_rect.x..toast_rect.right() {
                    let ch = if y == toast_rect.y && x == toast_rect.x {
                        '┌'
                    } else if y == toast_rect.y && x == toast_rect.right() - 1 {
                        '┐'
                    } else if y == toast_rect.bottom() - 1 && x == toast_rect.x {
                        '└'
                    } else if y == toast_rect.bottom() - 1 && x == toast_rect.right() - 1 {
                        '┘'
                    } else if y == toast_rect.y || y == toast_rect.bottom() - 1 {
                        '─'
                    } else if x == toast_rect.x || x == toast_rect.right() - 1 {
                        '│'
                    } else {
                        ' '
                    };

                    buf[(x, y)].set_char(ch).set_style(border_style);
                }
            }

            // Draw icon + message inside
            let text_y = toast_rect.y + 1;
            let icon_str = format!(" {} ", icon);
            for (i, c) in icon_str.chars().enumerate() {
                let x = toast_rect.x + 1 + i as u16;
                buf[(x, text_y)]
                    .set_char(c)
                    .set_style(border_style.add_modifier(Modifier::BOLD));
            }

            let msg_start = toast_rect.x + 4;
            let max_len = (toast_rect.right().saturating_sub(msg_start + 1)) as usize;
            for (i, c) in toast.message.chars().take(max_len).enumerate() {
                let x = msg_start + i as u16;
                buf[(x, text_y)].set_char(c).set_style(text_style);
            }

            top_y += 4;
        }
    }
}
