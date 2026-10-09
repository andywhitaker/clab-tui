use crate::clab::model::LabTopology;
use crate::clab::parser::TopologyParser;
use crate::ui::theme::Theme;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};

#[derive(Debug, Clone)]
pub struct YamlViewState {
    pub scroll_offset: usize,
    pub yaml_cache: String,
    pub cached_name: String,
    pub is_editing: bool,
    pub cursor_row: usize,
    pub cursor_col: usize,
    pub edit_lines: Vec<String>,
    pub error_msg: Option<String>,
}

impl Default for YamlViewState {
    fn default() -> Self {
        Self {
            scroll_offset: 0,
            yaml_cache: String::new(),
            cached_name: String::new(),
            is_editing: false,
            cursor_row: 0,
            cursor_col: 0,
            edit_lines: vec![String::new()],
            error_msg: None,
        }
    }
}

impl YamlViewState {
    /// Update internal state from topology
    pub fn update(&mut self, topo: &LabTopology) {
        if let Ok(yaml) = TopologyParser::serialize(topo) {
            self.yaml_cache = yaml.clone();
            self.cached_name = topo.name.clone();
            if !self.is_editing {
                self.edit_lines = yaml.lines().map(|s| s.to_string()).collect();
                if self.edit_lines.is_empty() {
                    self.edit_lines.push(String::new());
                }
                self.clamp_cursor();
            }
        }
    }

    /// Enter edit mode
    pub fn start_editing(&mut self) {
        self.is_editing = true;
        self.error_msg = None;
        if self.edit_lines.is_empty() {
            self.edit_lines = self.yaml_cache.lines().map(|s| s.to_string()).collect();
            if self.edit_lines.is_empty() {
                self.edit_lines.push(String::new());
            }
        }
        self.clamp_cursor();
    }

    /// Cancel edit mode and revert to last valid cache
    pub fn cancel_editing(&mut self) {
        self.is_editing = false;
        self.error_msg = None;
        self.edit_lines = self.yaml_cache.lines().map(|s| s.to_string()).collect();
        if self.edit_lines.is_empty() {
            self.edit_lines.push(String::new());
        }
        self.clamp_cursor();
    }

    /// Return full YAML text from edit lines
    pub fn get_text(&self) -> String {
        self.edit_lines.join("\n")
    }

    /// Ensure cursor is within bounds of edit lines
    pub fn clamp_cursor(&mut self) {
        if self.edit_lines.is_empty() {
            self.edit_lines.push(String::new());
        }
        if self.cursor_row >= self.edit_lines.len() {
            self.cursor_row = self.edit_lines.len() - 1;
        }
        let line_len = self.edit_lines[self.cursor_row].chars().count();
        if self.cursor_col > line_len {
            self.cursor_col = line_len;
        }
    }

    pub fn scroll_up(&mut self, amount: usize) {
        self.scroll_offset = self.scroll_offset.saturating_sub(amount);
    }

    pub fn scroll_down(&mut self, amount: usize, visible_lines: usize) {
        let lines_count = if self.is_editing {
            self.edit_lines.len()
        } else {
            self.yaml_cache.lines().count()
        };
        let max_scroll = lines_count.saturating_sub(visible_lines);
        self.scroll_offset = (self.scroll_offset + amount).min(max_scroll);
    }

    pub fn move_up(&mut self) {
        self.cursor_row = self.cursor_row.saturating_sub(1);
        self.clamp_cursor();
        if self.cursor_row < self.scroll_offset {
            self.scroll_offset = self.cursor_row;
        }
    }

    pub fn move_down(&mut self, visible_lines: usize) {
        if self.cursor_row + 1 < self.edit_lines.len() {
            self.cursor_row += 1;
        }
        self.clamp_cursor();
        let eff_visible = visible_lines.max(1);
        if self.cursor_row >= self.scroll_offset + eff_visible {
            self.scroll_offset = self.cursor_row.saturating_sub(eff_visible - 1);
        }
    }

    pub fn move_left(&mut self) {
        if self.cursor_col > 0 {
            self.cursor_col -= 1;
        } else if self.cursor_row > 0 {
            self.cursor_row -= 1;
            self.cursor_col = self.edit_lines[self.cursor_row].chars().count();
            if self.cursor_row < self.scroll_offset {
                self.scroll_offset = self.cursor_row;
            }
        }
    }

    pub fn move_right(&mut self, visible_lines: usize) {
        let line_len = self.edit_lines[self.cursor_row].chars().count();
        if self.cursor_col < line_len {
            self.cursor_col += 1;
        } else if self.cursor_row + 1 < self.edit_lines.len() {
            self.cursor_row += 1;
            self.cursor_col = 0;
            let eff_visible = visible_lines.max(1);
            if self.cursor_row >= self.scroll_offset + eff_visible {
                self.scroll_offset = self.cursor_row.saturating_sub(eff_visible - 1);
            }
        }
    }

    pub fn move_home(&mut self) {
        self.cursor_col = 0;
    }

    pub fn move_end(&mut self) {
        self.cursor_col = self.edit_lines[self.cursor_row].chars().count();
    }

    pub fn insert_char(&mut self, c: char) {
        self.clamp_cursor();
        let line = &mut self.edit_lines[self.cursor_row];
        let mut chars: Vec<char> = line.chars().collect();
        chars.insert(self.cursor_col, c);
        *line = chars.into_iter().collect();
        self.cursor_col += 1;
        self.error_msg = None;
    }

    pub fn insert_tab(&mut self) {
        self.insert_char(' ');
        self.insert_char(' ');
    }

    pub fn insert_newline(&mut self, visible_lines: usize) {
        self.clamp_cursor();
        let current_line = &self.edit_lines[self.cursor_row];
        let indent: String = current_line.chars().take_while(|c| *c == ' ').collect();
        let chars: Vec<char> = current_line.chars().collect();
        let before: String = chars[..self.cursor_col].iter().collect();
        let after: String = chars[self.cursor_col..].iter().collect();

        self.edit_lines[self.cursor_row] = before;
        let indent_len = indent.len();
        let next_line = format!("{}{}", indent, after);
        self.edit_lines.insert(self.cursor_row + 1, next_line);
        self.cursor_row += 1;
        self.cursor_col = indent_len;
        self.error_msg = None;

        let eff_visible = visible_lines.max(1);
        if self.cursor_row >= self.scroll_offset + eff_visible {
            self.scroll_offset = self.cursor_row.saturating_sub(eff_visible - 1);
        }
    }

    pub fn backspace(&mut self) {
        self.clamp_cursor();
        if self.cursor_col > 0 {
            let line = &mut self.edit_lines[self.cursor_row];
            let mut chars: Vec<char> = line.chars().collect();
            chars.remove(self.cursor_col - 1);
            *line = chars.into_iter().collect();
            self.cursor_col -= 1;
            self.error_msg = None;
        } else if self.cursor_row > 0 {
            let current = self.edit_lines.remove(self.cursor_row);
            self.cursor_row -= 1;
            let prev_len = self.edit_lines[self.cursor_row].chars().count();
            self.edit_lines[self.cursor_row].push_str(&current);
            self.cursor_col = prev_len;
            self.error_msg = None;
            if self.cursor_row < self.scroll_offset {
                self.scroll_offset = self.cursor_row;
            }
        }
    }

    pub fn delete(&mut self) {
        self.clamp_cursor();
        let line_len = self.edit_lines[self.cursor_row].chars().count();
        if self.cursor_col < line_len {
            let line = &mut self.edit_lines[self.cursor_row];
            let mut chars: Vec<char> = line.chars().collect();
            chars.remove(self.cursor_col);
            *line = chars.into_iter().collect();
            self.error_msg = None;
        } else if self.cursor_row + 1 < self.edit_lines.len() {
            let next_line = self.edit_lines.remove(self.cursor_row + 1);
            self.edit_lines[self.cursor_row].push_str(&next_line);
            self.error_msg = None;
        }
    }
}

pub struct YamlView;

impl YamlView {
    pub fn render(state: &YamlViewState, area: Rect, buf: &mut Buffer, theme: &Theme) {
        if area.width < 10 || area.height < 4 {
            return;
        }

        let bg_style = Style::default().bg(theme.panel_bg);
        for y in area.y..area.bottom() {
            for x in area.x..area.right() {
                buf[(x, y)].set_char(' ').set_style(bg_style);
            }
        }

        // Header bar
        let header = if state.is_editing {
            format!(
                " Containerlab Topology YAML ({}.clab.yml) [EDIT MODE]  Ctrl+S: Apply/Save | Esc: Exit Edit",
                state.cached_name
            )
        } else {
            format!(
                " Containerlab Topology YAML ({}.clab.yml)  e: Edit | s: Save | r: Reload",
                state.cached_name
            )
        };

        let header_style = if state.is_editing {
            Style::default()
                .fg(theme.warning)
                .bg(theme.panel_bg)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default()
                .fg(theme.accent)
                .bg(theme.panel_bg)
                .add_modifier(Modifier::BOLD)
        };

        for (i, ch) in header.chars().enumerate() {
            let px = area.x + 1 + i as u16;
            if px < area.right() {
                buf[(px, area.y)].set_char(ch).set_style(header_style);
            }
        }

        let div_y = area.y + 1;
        for x in area.x..area.right() {
            buf[(x, div_y)]
                .set_char('─')
                .set_style(Style::default().fg(theme.border).bg(theme.panel_bg));
        }

        let has_error = state.error_msg.is_some();
        let footer_rows = if has_error { 2 } else { 0 };

        let lines: &[String] = &state.edit_lines;
        let content_y = area.y + 2;
        let content_h = (area.height.saturating_sub(2 + footer_rows)) as usize;

        let start_idx = state
            .scroll_offset
            .min(lines.len().saturating_sub(content_h));
        let slice = &lines[start_idx..lines.len().min(start_idx + content_h)];

        for (row_idx, line) in slice.iter().enumerate() {
            let y = content_y + row_idx as u16;
            if y >= area.bottom().saturating_sub(footer_rows) {
                break;
            }

            let absolute_row = start_idx + row_idx;
            let line_num_str = format!("{:03} │ ", absolute_row + 1);
            let num_style = if state.is_editing && absolute_row == state.cursor_row {
                Style::default()
                    .fg(theme.accent)
                    .bg(theme.panel_bg)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(theme.text_muted).bg(theme.panel_bg)
            };

            for (i, ch) in line_num_str.chars().enumerate() {
                let px = area.x + 1 + i as u16;
                if px < area.right() {
                    buf[(px, y)].set_char(ch).set_style(num_style);
                }
            }

            // Syntax highlighting: keys in cyan/accent, strings/values in yellow/white
            let line_style = if line.trim_start().starts_with('#') {
                Style::default().fg(theme.text_muted).bg(theme.panel_bg)
            } else if line.trim_start().starts_with('-') {
                Style::default().fg(theme.warning).bg(theme.panel_bg)
            } else if line.contains(':') {
                Style::default().fg(theme.accent).bg(theme.panel_bg)
            } else {
                Style::default().fg(theme.text_primary).bg(theme.panel_bg)
            };

            let text_start_x = area.x + 1 + line_num_str.len() as u16;
            let max_w = (area.right().saturating_sub(text_start_x)) as usize;

            for (i, ch) in line.chars().take(max_w).enumerate() {
                let px = text_start_x + i as u16;
                buf[(px, y)].set_char(ch).set_style(line_style);
            }

            // Cursor rendering when editing
            if state.is_editing && absolute_row == state.cursor_row {
                let cursor_px = text_start_x + state.cursor_col as u16;
                if cursor_px < area.right() {
                    let cell = &mut buf[(cursor_px, y)];
                    let cur_char = cell.symbol().chars().next().unwrap_or(' ');
                    let cursor_style = Style::default()
                        .fg(Color::Black)
                        .bg(theme.accent)
                        .add_modifier(Modifier::BOLD);
                    cell.set_char(cur_char).set_style(cursor_style);
                }
            }
        }

        // Render error bar at bottom if present
        if let Some(ref err) = state.error_msg {
            let err_y = area.bottom().saturating_sub(1);
            let err_text = format!(" ⚠ {}", err);
            let err_style = Style::default()
                .fg(Color::White)
                .bg(theme.error)
                .add_modifier(Modifier::BOLD);

            for x in area.x..area.right() {
                buf[(x, err_y)].set_char(' ').set_style(err_style);
            }
            for (i, ch) in err_text.chars().enumerate() {
                let px = area.x + i as u16;
                if px < area.right() {
                    buf[(px, err_y)].set_char(ch).set_style(err_style);
                }
            }
        }
    }
}
