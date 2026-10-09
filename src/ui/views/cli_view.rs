use crate::clab::commands::{CliArgKind, CliCategory, CliFocusedPane, CliViewState};
use crate::ui::theme::Theme;
use crate::ui::widgets::log_viewer::{classify_log_line, LogLevel};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};

pub struct CliView;

impl CliView {
    pub fn render(state: &CliViewState, area: Rect, buf: &mut Buffer, theme: &Theme) {
        if area.width < 20 || area.height < 6 {
            return;
        }

        // Layout: Left column for Command Tree, Right column for Flags/Preview/Output
        let tree_width = if area.width >= 60 {
            (area.width * 28 / 100)
                .clamp(24, 34)
                .min(area.width.saturating_sub(30))
        } else {
            (area.width * 35 / 100)
                .max(12)
                .min(area.width.saturating_sub(15))
        };
        let tree_rect = Rect::new(area.x, area.y, tree_width, area.height);

        let right_x = area.x + tree_width;
        let right_width = area.width.saturating_sub(tree_width);
        let right_rect = Rect::new(right_x, area.y, right_width, area.height);

        Self::render_command_tree(state, tree_rect, buf, theme);
        Self::render_right_pane(state, right_rect, buf, theme);
    }

    fn render_command_tree(state: &CliViewState, area: Rect, buf: &mut Buffer, theme: &Theme) {
        let is_focused = state.focused_pane == CliFocusedPane::Commands;
        let border_style = if is_focused {
            Style::default().fg(theme.accent).bg(theme.bg)
        } else {
            Style::default().fg(theme.border).bg(theme.bg)
        };

        // Draw border
        Self::draw_box(area, buf, border_style, theme);

        // Title
        let title = " ⬢ CLI Command Tree ";
        let title_style = if is_focused {
            Style::default()
                .fg(theme.accent)
                .bg(theme.panel_bg)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default()
                .fg(theme.text_secondary)
                .bg(theme.panel_bg)
                .add_modifier(Modifier::BOLD)
        };
        Self::draw_text(
            area.x + 2,
            area.y,
            title,
            title_style,
            area.right() - 1,
            buf,
        );

        let inner_y = area.y + 1;
        let inner_h = area.height.saturating_sub(2) as usize;
        let inner_w = area.width.saturating_sub(2);
        let inner_x = area.x + 1;

        if inner_h == 0 {
            return;
        }

        // Build list of display rows: headers and command items
        struct TreeRow {
            is_header: bool,
            text: String,
            cmd_idx: Option<usize>,
        }

        let mut rows: Vec<TreeRow> = Vec::new();

        // Core section
        rows.push(TreeRow {
            is_header: true,
            text: "▼ Core Commands".to_string(),
            cmd_idx: None,
        });
        for (i, cmd) in state.commands.iter().enumerate() {
            if cmd.category == CliCategory::Core {
                rows.push(TreeRow {
                    is_header: false,
                    text: cmd.name.clone(),
                    cmd_idx: Some(i),
                });
            }
        }

        // Tools section
        rows.push(TreeRow {
            is_header: true,
            text: "▼ Tools Subcommands".to_string(),
            cmd_idx: None,
        });
        for (i, cmd) in state.commands.iter().enumerate() {
            if cmd.category == CliCategory::Tools {
                // Strip "tools " prefix for clean tree appearance
                let display_name = cmd.name.strip_prefix("tools ").unwrap_or(&cmd.name);
                rows.push(TreeRow {
                    is_header: false,
                    text: display_name.to_string(),
                    cmd_idx: Some(i),
                });
            }
        }

        // Determine scroll offset to keep selected command visible
        let selected_row_idx = rows
            .iter()
            .position(|r| r.cmd_idx == Some(state.selected_command_idx))
            .unwrap_or(0);

        let scroll_offset = if selected_row_idx >= inner_h {
            selected_row_idx.saturating_sub(inner_h - 1)
        } else {
            0
        };

        for (screen_idx, row) in rows.iter().skip(scroll_offset).take(inner_h).enumerate() {
            let row_y = inner_y + screen_idx as u16;
            if row.is_header {
                let header_style = Style::default()
                    .fg(theme.accent_alt)
                    .bg(theme.panel_bg)
                    .add_modifier(Modifier::BOLD);
                let line = format!(" {}", row.text);
                Self::draw_text(inner_x, row_y, &line, header_style, area.right() - 1, buf);
            } else if let Some(cmd_idx) = row.cmd_idx {
                let is_selected = cmd_idx == state.selected_command_idx;
                let bg_style = if is_selected {
                    Style::default().bg(theme.node_selected_bg)
                } else {
                    Style::default().bg(theme.bg)
                };

                // Clear line background
                for x in inner_x..inner_x + inner_w {
                    buf[(x, row_y)].set_char(' ').set_style(bg_style);
                }

                let icon = if is_selected { " ● " } else { "   " };
                let icon_style = if is_selected {
                    Style::default()
                        .fg(theme.accent)
                        .bg(theme.node_selected_bg)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(theme.text_muted).bg(theme.bg)
                };
                Self::draw_text(inner_x, row_y, icon, icon_style, area.right() - 1, buf);

                let text_style = if is_selected {
                    Style::default()
                        .fg(theme.text_primary)
                        .bg(theme.node_selected_bg)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(theme.text_primary).bg(theme.bg)
                };
                Self::draw_text(
                    inner_x + 3,
                    row_y,
                    &row.text,
                    text_style,
                    area.right() - 1,
                    buf,
                );
            }
        }
    }

    fn render_right_pane(state: &CliViewState, area: Rect, buf: &mut Buffer, theme: &Theme) {
        if area.width < 10 || area.height < 4 {
            return;
        }

        let preview_h = 3.min(area.height.saturating_sub(2));
        let available_h = area.height.saturating_sub(preview_h);
        let min_args_h = 4;
        let min_out_h = 2;

        let max_args_h = available_h.saturating_sub(min_out_h);
        let args_h = if min_args_h <= max_args_h {
            ((available_h * 50) / 100).clamp(min_args_h, max_args_h)
        } else {
            (available_h / 2).min(max_args_h).max(1)
        };
        let out_h = available_h.saturating_sub(args_h);

        let args_rect = Rect::new(area.x, area.y, area.width, args_h);
        let preview_rect = Rect::new(area.x, area.y + args_h, area.width, preview_h);
        let output_rect = Rect::new(area.x, area.y + args_h + preview_h, area.width, out_h);

        Self::render_args_pane(state, args_rect, buf, theme);
        Self::render_preview_pane(state, preview_rect, buf, theme);
        Self::render_output_pane(state, output_rect, buf, theme);
    }

    fn render_args_pane(state: &CliViewState, area: Rect, buf: &mut Buffer, theme: &Theme) {
        let is_focused = state.focused_pane == CliFocusedPane::Arguments;
        let border_style = if is_focused {
            Style::default().fg(theme.accent).bg(theme.bg)
        } else {
            Style::default().fg(theme.border).bg(theme.bg)
        };

        Self::draw_box(area, buf, border_style, theme);

        let cmd = state.selected_command();
        let title = format!(" Flags & Arguments: containerlab {} ", cmd.name);
        let title_style = if is_focused {
            Style::default()
                .fg(theme.accent)
                .bg(theme.panel_bg)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default()
                .fg(theme.text_secondary)
                .bg(theme.panel_bg)
                .add_modifier(Modifier::BOLD)
        };
        Self::draw_text(
            area.x + 2,
            area.y,
            &title,
            title_style,
            area.right() - 1,
            buf,
        );

        let args_hint = "[Space: Toggle | b/Enter: Browse | e: Edit]";
        let args_hint_style = Style::default().fg(theme.text_muted).bg(theme.panel_bg);
        let hint_x = area.right().saturating_sub(args_hint.len() as u16 + 2);
        if hint_x > area.x + title.len() as u16 + 3 {
            Self::draw_text(
                hint_x,
                area.y,
                args_hint,
                args_hint_style,
                area.right() - 1,
                buf,
            );
        }

        if area.height < 3 {
            return;
        }

        // Subtitle: command description
        let desc_line = format!(" Description: {}", cmd.description);
        let desc_style = Style::default().fg(theme.text_secondary).bg(theme.bg);
        Self::draw_text(
            area.x + 2,
            area.y + 1,
            &desc_line,
            desc_style,
            area.right() - 2,
            buf,
        );

        // Divider
        let div_y = area.y + 2;
        if div_y < area.bottom() - 1 {
            for x in area.x + 1..area.right() - 1 {
                buf[(x, div_y)]
                    .set_char('┄')
                    .set_style(Style::default().fg(theme.border).bg(theme.bg));
            }
        }

        // Argument rows
        let list_y = area.y + 3;
        let list_h = (area.bottom().saturating_sub(1).saturating_sub(list_y)) as usize;
        let list_w = area.width.saturating_sub(2);
        let list_x = area.x + 1;

        if list_h == 0 {
            return;
        }

        let scroll_offset = if state.selected_arg_idx >= list_h {
            state.selected_arg_idx.saturating_sub(list_h - 1)
        } else {
            0
        };

        for (i, arg) in cmd.args.iter().skip(scroll_offset).take(list_h).enumerate() {
            let actual_idx = scroll_offset + i;
            let row_y = list_y + i as u16;
            let is_selected = actual_idx == state.selected_arg_idx;

            let row_bg = if is_selected && is_focused {
                theme.node_selected_bg
            } else {
                theme.bg
            };

            for x in list_x..list_x + list_w {
                buf[(x, row_y)]
                    .set_char(' ')
                    .set_style(Style::default().bg(row_bg));
            }

            // Checkbox
            let (chk_text, chk_style) = if arg.enabled {
                (
                    "[x] ",
                    Style::default()
                        .fg(theme.accent)
                        .bg(row_bg)
                        .add_modifier(Modifier::BOLD),
                )
            } else {
                ("[ ] ", Style::default().fg(theme.text_muted).bg(row_bg))
            };
            Self::draw_text(
                list_x + 1,
                row_y,
                chk_text,
                chk_style,
                area.right() - 1,
                buf,
            );

            // Flag text
            let flag_str = if let Some(ref sh) = arg.short {
                format!("{} ({})", arg.flag, sh)
            } else {
                arg.flag.clone()
            };
            let flag_style = if is_selected {
                Style::default()
                    .fg(theme.text_primary)
                    .bg(row_bg)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(theme.text_primary).bg(row_bg)
            };
            Self::draw_text(
                list_x + 5,
                row_y,
                &flag_str,
                flag_style,
                area.right() - 1,
                buf,
            );

            let flag_col_w = 24.min(area.width / 3);
            let val_start_x = list_x + 5 + flag_col_w;

            match &arg.kind {
                CliArgKind::Flag => {
                    let desc_style = Style::default().fg(theme.text_secondary).bg(row_bg);
                    Self::draw_text(
                        val_start_x,
                        row_y,
                        &arg.description,
                        desc_style,
                        area.right() - 2,
                        buf,
                    );
                }
                CliArgKind::Value { placeholder } => {
                    if state.is_editing_arg && is_selected {
                        let edit_str = format!("[ {}█ ]", state.edit_buffer);
                        let edit_style = Style::default()
                            .fg(theme.warning)
                            .bg(row_bg)
                            .add_modifier(Modifier::BOLD);
                        Self::draw_text(
                            val_start_x,
                            row_y,
                            &edit_str,
                            edit_style,
                            area.right() - 1,
                            buf,
                        );

                        let hint = " (Enter to save, Esc to cancel)";
                        let hint_style = Style::default().fg(theme.accent_alt).bg(row_bg);
                        let hint_x = val_start_x + edit_str.len() as u16;
                        Self::draw_text(hint_x, row_y, hint, hint_style, area.right() - 2, buf);
                    } else {
                        let is_file = arg.is_file_arg();
                        let val_display = if arg.value.is_empty() {
                            if is_file {
                                format!("[📁 <{}> (b/Enter: browse)]", placeholder)
                            } else {
                                format!("[<{}>]", placeholder)
                            }
                        } else if is_file {
                            format!("[📁 {}]", arg.value)
                        } else {
                            format!("[{}]", arg.value)
                        };
                        let val_style = if arg.value.is_empty() {
                            Style::default().fg(theme.text_muted).bg(row_bg)
                        } else {
                            Style::default()
                                .fg(if is_file { theme.info } else { theme.accent })
                                .bg(row_bg)
                                .add_modifier(Modifier::BOLD)
                        };
                        Self::draw_text(
                            val_start_x,
                            row_y,
                            &val_display,
                            val_style,
                            area.right() - 1,
                            buf,
                        );

                        let desc_x = val_start_x + val_display.len() as u16 + 2;
                        let desc_style = Style::default().fg(theme.text_secondary).bg(row_bg);
                        Self::draw_text(
                            desc_x,
                            row_y,
                            &arg.description,
                            desc_style,
                            area.right() - 2,
                            buf,
                        );
                    }
                }
            }
        }
    }

    fn render_preview_pane(state: &CliViewState, area: Rect, buf: &mut Buffer, theme: &Theme) {
        let border_style = Style::default().fg(theme.border).bg(theme.bg);
        Self::draw_box(area, buf, border_style, theme);

        let title = " Live Shell Command Preview ";
        let title_style = Style::default()
            .fg(theme.accent_alt)
            .bg(theme.panel_bg)
            .add_modifier(Modifier::BOLD);
        Self::draw_text(
            area.x + 2,
            area.y,
            title,
            title_style,
            area.right() - 1,
            buf,
        );

        if area.height < 2 {
            return;
        }

        let cmd_preview = format!("$ {}", state.preview_string("containerlab"));
        let cmd_style = Style::default()
            .fg(theme.accent)
            .bg(theme.panel_bg)
            .add_modifier(Modifier::BOLD);

        Self::draw_text(
            area.x + 2,
            area.y + 1,
            &cmd_preview,
            cmd_style,
            area.right() - 22,
            buf,
        );

        // Right hint
        let run_hint = "[r / Enter: Execute]";
        let run_style = Style::default()
            .fg(theme.success)
            .bg(theme.panel_bg)
            .add_modifier(Modifier::BOLD);
        let hint_x = area.right().saturating_sub(run_hint.len() as u16 + 2);
        Self::draw_text(
            hint_x,
            area.y + 1,
            run_hint,
            run_style,
            area.right() - 1,
            buf,
        );
    }

    fn render_output_pane(state: &CliViewState, area: Rect, buf: &mut Buffer, theme: &Theme) {
        let is_focused = state.focused_pane == CliFocusedPane::Output;
        let border_style = if is_focused {
            Style::default().fg(theme.accent).bg(theme.bg)
        } else {
            Style::default().fg(theme.border).bg(theme.bg)
        };

        Self::draw_box(area, buf, border_style, theme);

        // Title and status badge
        let (status_text, status_style) = if state.is_running {
            let label = if let Some(ref exec_cmd) = state.executing_cmd {
                format!(" [● RUNNING: {}] ", exec_cmd)
            } else {
                " [● RUNNING] ".to_string()
            };
            (
                label,
                Style::default()
                    .fg(theme.warning)
                    .bg(theme.panel_bg)
                    .add_modifier(Modifier::BOLD),
            )
        } else if let Some(true) = state.selected_command().last_status {
            (
                " [✓ SUCCESS] ".to_string(),
                Style::default()
                    .fg(theme.success)
                    .bg(theme.panel_bg)
                    .add_modifier(Modifier::BOLD),
            )
        } else if let Some(false) = state.selected_command().last_status {
            (
                " [✗ FAILED] ".to_string(),
                Style::default()
                    .fg(theme.error)
                    .bg(theme.panel_bg)
                    .add_modifier(Modifier::BOLD),
            )
        } else {
            (
                " [IDLE] ".to_string(),
                Style::default().fg(theme.text_muted).bg(theme.panel_bg),
            )
        };

        let title_prefix = " Execution Console";
        let title_style = if is_focused {
            Style::default()
                .fg(theme.accent)
                .bg(theme.panel_bg)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default()
                .fg(theme.text_secondary)
                .bg(theme.panel_bg)
                .add_modifier(Modifier::BOLD)
        };

        Self::draw_text(
            area.x + 2,
            area.y,
            title_prefix,
            title_style,
            area.right() - 1,
            buf,
        );
        let badge_x = area.x + 2 + title_prefix.len() as u16;
        Self::draw_text(
            badge_x,
            area.y,
            &status_text,
            status_style,
            area.right() - 1,
            buf,
        );

        // Right hint
        let hint = "[c: Clear | ↑/↓: Scroll]";
        let hint_style = Style::default().fg(theme.text_muted).bg(theme.panel_bg);
        let hint_x = area.right().saturating_sub(hint.len() as u16 + 2);
        Self::draw_text(hint_x, area.y, hint, hint_style, area.right() - 1, buf);

        let inner_y = area.y + 1;
        let inner_h = area.height.saturating_sub(2) as usize;
        let inner_w = area.width.saturating_sub(2);
        let inner_x = area.x + 1;

        if inner_h == 0 {
            return;
        }

        let total_lines = state.output_lines.len();
        let bottom_offset = total_lines.saturating_sub(inner_h);
        let start_line = bottom_offset.saturating_sub(state.output_scroll);

        for (screen_idx, line) in state
            .output_lines
            .iter()
            .skip(start_line)
            .take(inner_h)
            .enumerate()
        {
            let row_y = inner_y + screen_idx as u16;

            // Fill line bg
            for x in inner_x..inner_x + inner_w {
                buf[(x, row_y)]
                    .set_char(' ')
                    .set_style(Style::default().bg(theme.bg));
            }

            let line_style = match classify_log_line(line) {
                LogLevel::Error => Style::default()
                    .fg(theme.error)
                    .bg(theme.bg)
                    .add_modifier(Modifier::BOLD),
                LogLevel::Warning => Style::default().fg(theme.warning).bg(theme.bg),
                LogLevel::Success => Style::default()
                    .fg(theme.success)
                    .bg(theme.bg)
                    .add_modifier(Modifier::BOLD),
                LogLevel::Info => Style::default().fg(theme.info).bg(theme.bg),
                LogLevel::Normal => {
                    if line.starts_with('$') {
                        Style::default()
                            .fg(theme.accent)
                            .bg(theme.bg)
                            .add_modifier(Modifier::BOLD)
                    } else {
                        Style::default().fg(theme.text_primary).bg(theme.bg)
                    }
                }
            };

            Self::draw_text(inner_x + 1, row_y, line, line_style, area.right() - 1, buf);
        }
    }

    fn draw_box(area: Rect, buf: &mut Buffer, border_style: Style, theme: &Theme) {
        if area.width < 2 || area.height < 2 {
            return;
        }

        let bg_style = Style::default().bg(theme.panel_bg);

        for y in area.y..area.bottom() {
            for x in area.x..area.right() {
                let ch = if y == area.y && x == area.x {
                    '╭'
                } else if y == area.y && x == area.right() - 1 {
                    '╮'
                } else if y == area.bottom() - 1 && x == area.x {
                    '╰'
                } else if y == area.bottom() - 1 && x == area.right() - 1 {
                    '╯'
                } else if y == area.y || y == area.bottom() - 1 {
                    '─'
                } else if x == area.x || x == area.right() - 1 {
                    '│'
                } else {
                    ' '
                };

                let style = if ch == ' ' { bg_style } else { border_style };
                buf[(x, y)].set_char(ch).set_style(style);
            }
        }
    }

    fn draw_text(x: u16, y: u16, text: &str, style: Style, max_x: u16, buf: &mut Buffer) {
        for (i, ch) in text.chars().enumerate() {
            let px = x + i as u16;
            if px < max_x {
                buf[(px, y)].set_char(ch).set_style(style);
            }
        }
    }
}
