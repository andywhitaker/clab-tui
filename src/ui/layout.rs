use crate::ui::theme::Theme;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActiveTab {
    Canvas = 0,
    Inspect = 1,
    Logs = 2,
    Yaml = 3,
    Cli = 4,
}

impl ActiveTab {
    pub const ALL: [ActiveTab; 5] = [
        ActiveTab::Canvas,
        ActiveTab::Inspect,
        ActiveTab::Logs,
        ActiveTab::Yaml,
        ActiveTab::Cli,
    ];

    pub fn title(&self) -> &'static str {
        match self {
            ActiveTab::Canvas => "1: Canvas Designer",
            ActiveTab::Inspect => "2: Lab Inspector",
            ActiveTab::Logs => "3: Live Logs",
            ActiveTab::Yaml => "4: YAML Source",
            ActiveTab::Cli => "5: CLI Commands",
        }
    }

    pub fn next(&self) -> Self {
        match self {
            ActiveTab::Canvas => ActiveTab::Inspect,
            ActiveTab::Inspect => ActiveTab::Logs,
            ActiveTab::Logs => ActiveTab::Yaml,
            ActiveTab::Yaml => ActiveTab::Cli,
            ActiveTab::Cli => ActiveTab::Canvas,
        }
    }

    pub fn prev(&self) -> Self {
        match self {
            ActiveTab::Canvas => ActiveTab::Cli,
            ActiveTab::Inspect => ActiveTab::Canvas,
            ActiveTab::Logs => ActiveTab::Inspect,
            ActiveTab::Yaml => ActiveTab::Logs,
            ActiveTab::Cli => ActiveTab::Yaml,
        }
    }
}

#[derive(Debug, Clone)]
pub struct ConfirmModal {
    pub is_open: bool,
    pub title: String,
    pub prompt: String,
    pub action_name: String,
    pub cleanup: bool,
    pub reconfigure: bool,
    pub show_cleanup_toggle: bool,
    pub show_reconfigure_toggle: bool,
}

impl Default for ConfirmModal {
    fn default() -> Self {
        Self {
            is_open: false,
            title: "Confirm Action".to_string(),
            prompt: "Are you sure you want to proceed?".to_string(),
            action_name: "confirm".to_string(),
            cleanup: false,
            reconfigure: false,
            show_cleanup_toggle: false,
            show_reconfigure_toggle: false,
        }
    }
}

pub struct AppLayout;

impl AppLayout {
    pub fn render_header(
        lab_name: &str,
        privilege_desc: &str,
        area: Rect,
        buf: &mut Buffer,
        theme: &Theme,
    ) {
        let bg_style = Style::default().bg(theme.panel_bg);
        for x in area.x..area.right() {
            buf[(x, area.y)].set_char(' ').set_style(bg_style);
        }

        // Left: Logo and Name
        let logo = " ⬢ clab-tui ";
        let logo_style = Style::default()
            .fg(theme.accent)
            .bg(theme.panel_bg)
            .add_modifier(Modifier::BOLD);
        for (i, ch) in logo.chars().enumerate() {
            let px = area.x + i as u16;
            if px < area.right() {
                buf[(px, area.y)].set_char(ch).set_style(logo_style);
            }
        }

        // Lab Name tag
        let lab_tag = format!("| Lab: {} ", lab_name);
        let tag_style = Style::default().fg(theme.text_primary).bg(theme.panel_bg);
        for (i, ch) in lab_tag.chars().enumerate() {
            let px = area.x + logo.len() as u16 + i as u16;
            if px < area.right() {
                buf[(px, area.y)].set_char(ch).set_style(tag_style);
            }
        }

        // Right side indicators: Sudo/Root status
        let right_info = format!("[{}] ", privilege_desc);

        let start_x = area.right().saturating_sub(right_info.len() as u16);
        let info_style = Style::default().fg(theme.text_secondary).bg(theme.panel_bg);
        for (i, ch) in right_info.chars().enumerate() {
            let px = start_x + i as u16;
            if px < area.right() {
                buf[(px, area.y)].set_char(ch).set_style(info_style);
            }
        }
    }

    pub fn render_tabs(active_tab: ActiveTab, area: Rect, buf: &mut Buffer, theme: &Theme) {
        let bg_style = Style::default().bg(theme.panel_bg);
        for x in area.x..area.right() {
            buf[(x, area.y)].set_char(' ').set_style(bg_style);
        }

        let mut curr_x = area.x + 2;
        for tab in ActiveTab::ALL {
            let is_active = tab == active_tab;
            let tab_style = if is_active {
                Style::default()
                    .fg(theme.accent)
                    .bg(theme.bg)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(theme.text_muted).bg(theme.panel_bg)
            };

            let title = format!("  {}  ", tab.title());
            for (i, ch) in title.chars().enumerate() {
                let px = curr_x + i as u16;
                if px < area.right() {
                    buf[(px, area.y)].set_char(ch).set_style(tab_style);
                }
            }

            curr_x += title.len() as u16 + 1;
        }

        // Divider below tabs
        let div_y = area.y + 1;
        for x in area.x..area.right() {
            buf[(x, div_y)]
                .set_char('─')
                .set_style(Style::default().fg(theme.border).bg(theme.bg));
        }
    }

    pub fn render_footer(active_tab: ActiveTab, area: Rect, buf: &mut Buffer, theme: &Theme) {
        let bg_style = Style::default().bg(theme.panel_bg);
        for x in area.x..area.right() {
            buf[(x, area.y)].set_char(' ').set_style(bg_style);
        }

        let hints = match active_tab {
            ActiveTab::Canvas => " [?] Help | [v/s] Box Select | [w] Wire | [a] Add Node | [e] Edit | [x] Delete | [d] Deploy | [Ctrl+S] Save | [q] Quit",
            ActiveTab::Inspect => " [?] Help | [r] Refresh | [d] Deploy | [D] Teardown | [q] Quit",
            ActiveTab::Logs => " [?] Help | [c] Clear Logs | [/] Filter | [Space] Toggle Auto-scroll | [q] Quit",
            ActiveTab::Yaml => " [?] Help | [e] Edit Mode | [Ctrl+S] Apply/Save | [r] Reload | [d] Deploy | [q] Quit",
            ActiveTab::Cli => " [?] Help | [Tab] Switch Pane | [Space] Toggle Flag | [e] Edit Param | [r/Enter] Execute | [c] Clear Log | [q] Quit",
        };

        let hint_style = Style::default().fg(theme.text_secondary).bg(theme.panel_bg);
        for (i, ch) in hints.chars().enumerate() {
            let px = area.x + 1 + i as u16;
            if px < area.right() {
                buf[(px, area.y)].set_char(ch).set_style(hint_style);
            }
        }
    }

    pub fn render_confirm_modal(modal: &ConfirmModal, area: Rect, buf: &mut Buffer, theme: &Theme) {
        if !modal.is_open {
            return;
        }

        let has_options = modal.show_cleanup_toggle || modal.show_reconfigure_toggle;
        let option_lines = (if modal.show_cleanup_toggle { 1 } else { 0 })
            + (if modal.show_reconfigure_toggle { 1 } else { 0 });

        let width = 62.min(area.width.saturating_sub(4));
        let height =
            (if has_options { 8 + option_lines } else { 8 }).min(area.height.saturating_sub(2));

        let x = area.x + (area.width.saturating_sub(width)) / 2;
        let y = area.y + (area.height.saturating_sub(height)) / 2;
        let modal_rect = Rect::new(x, y, width, height);

        let border_style = Style::default().fg(theme.error).bg(theme.panel_bg);

        // Fill background & draw border
        for py in modal_rect.y..modal_rect.bottom() {
            for px in modal_rect.x..modal_rect.right() {
                let ch = if py == modal_rect.y && px == modal_rect.x {
                    '╭'
                } else if py == modal_rect.y && px == modal_rect.right() - 1 {
                    '╮'
                } else if py == modal_rect.bottom() - 1 && px == modal_rect.x {
                    '╰'
                } else if py == modal_rect.bottom() - 1 && px == modal_rect.right() - 1 {
                    '╯'
                } else if py == modal_rect.y || py == modal_rect.bottom() - 1 {
                    '─'
                } else if px == modal_rect.x || px == modal_rect.right() - 1 {
                    '│'
                } else {
                    ' '
                };

                buf[(px, py)].set_char(ch).set_style(border_style);
            }
        }

        // Title
        let title = format!(" ⚠ {} ", modal.title);
        let title_style = Style::default()
            .fg(theme.error)
            .bg(theme.panel_bg)
            .add_modifier(Modifier::BOLD);
        for (i, ch) in title.chars().enumerate() {
            let px = modal_rect.x + 3 + i as u16;
            if px < modal_rect.right() - 1 {
                buf[(px, modal_rect.y)].set_char(ch).set_style(title_style);
            }
        }

        // Prompt text
        let prompt_y = modal_rect.y + 2;
        let prompt_style = Style::default().fg(theme.text_primary).bg(theme.panel_bg);
        for (i, ch) in modal.prompt.chars().enumerate() {
            let px = modal_rect.x + 3 + i as u16;
            if px < modal_rect.right() - 2 {
                buf[(px, prompt_y)].set_char(ch).set_style(prompt_style);
            }
        }

        let mut curr_y = modal_rect.y + 4;
        if modal.show_cleanup_toggle {
            let mark = if modal.cleanup { "[x]" } else { "[ ]" };
            let line = format!("  {} Cleanup (-c)                  [c: toggle]", mark);
            let style = if modal.cleanup {
                Style::default()
                    .fg(theme.accent)
                    .bg(theme.panel_bg)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(theme.text_muted).bg(theme.panel_bg)
            };
            for (i, ch) in line.chars().enumerate() {
                let px = modal_rect.x + 2 + i as u16;
                if px < modal_rect.right() - 2 {
                    buf[(px, curr_y)].set_char(ch).set_style(style);
                }
            }
            curr_y += 1;
        }

        if modal.show_reconfigure_toggle {
            let mark = if modal.reconfigure { "[x]" } else { "[ ]" };
            let line = format!("  {} Reconfigure (--reconfigure)   [r: toggle]", mark);
            let style = if modal.reconfigure {
                Style::default()
                    .fg(theme.accent)
                    .bg(theme.panel_bg)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(theme.text_muted).bg(theme.panel_bg)
            };
            for (i, ch) in line.chars().enumerate() {
                let px = modal_rect.x + 2 + i as u16;
                if px < modal_rect.right() - 2 {
                    buf[(px, curr_y)].set_char(ch).set_style(style);
                }
            }
            curr_y += 1;
        }

        // Action prompt
        let preferred_action_y = if has_options {
            curr_y + 1
        } else {
            modal_rect.y + 4
        };
        let action_y = if preferred_action_y < modal_rect.bottom().saturating_sub(1) {
            preferred_action_y
        } else {
            modal_rect.bottom().saturating_sub(2)
        };
        if action_y < modal_rect.bottom() - 1 && (action_y >= curr_y || !has_options) {
            let action_prompt = "Press [y / Enter] to Confirm, or [n / Esc] to Cancel";
            let action_style = Style::default()
                .fg(theme.warning)
                .bg(theme.panel_bg)
                .add_modifier(Modifier::BOLD);
            for (i, ch) in action_prompt.chars().enumerate() {
                let px = modal_rect.x + 3 + i as u16;
                if px < modal_rect.right() - 2 {
                    buf[(px, action_y)].set_char(ch).set_style(action_style);
                }
            }
        }
    }
}
