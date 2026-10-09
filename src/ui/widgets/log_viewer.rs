use crate::ui::theme::Theme;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};

#[derive(Debug, Clone)]
pub struct LogViewer {
    pub lines: Vec<String>,
    pub scroll_offset: usize,
    pub auto_scroll: bool,
    pub filter: String,
    pub is_filtering: bool,
}

impl Default for LogViewer {
    fn default() -> Self {
        Self {
            lines: Vec::new(),
            scroll_offset: 0,
            auto_scroll: true,
            filter: String::new(),
            is_filtering: false,
        }
    }
}

impl LogViewer {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn push_line(&mut self, line: impl Into<String>) {
        self.lines.push(line.into());
        if self.auto_scroll {
            self.scroll_to_bottom();
        }
    }

    pub fn clear(&mut self) {
        self.lines.clear();
        self.scroll_offset = 0;
    }

    pub fn scroll_up(&mut self, amount: usize) {
        self.auto_scroll = false;
        self.scroll_offset = self.scroll_offset.saturating_sub(amount);
    }

    pub fn scroll_down(&mut self, amount: usize, visible_height: usize) {
        let filtered_count = self.filtered_lines().len();
        let max_scroll = filtered_count.saturating_sub(visible_height);
        self.scroll_offset = (self.scroll_offset + amount).min(max_scroll);
        if self.scroll_offset >= max_scroll {
            self.auto_scroll = true;
        }
    }

    pub fn scroll_to_bottom(&mut self) {
        self.auto_scroll = true;
        let count = self.filtered_lines().len();
        self.scroll_offset = count.saturating_sub(20);
    }

    pub fn filtered_lines(&self) -> Vec<&str> {
        if self.filter.is_empty() {
            self.lines.iter().map(|s| s.as_str()).collect()
        } else {
            self.lines
                .iter()
                .filter(|s| s.to_lowercase().contains(&self.filter.to_lowercase()))
                .map(|s| s.as_str())
                .collect()
        }
    }

    pub fn render(&self, area: Rect, buf: &mut Buffer, theme: &Theme) {
        if area.width < 4 || area.height < 2 {
            return;
        }

        let border_style = Style::default().fg(theme.border).bg(theme.panel_bg);
        let bg_style = Style::default().bg(theme.panel_bg);

        // Fill background
        for y in area.y..area.bottom() {
            for x in area.x..area.right() {
                buf[(x, y)].set_char(' ').set_style(bg_style);
            }
        }

        // Header / Controls bar
        let autoscroll_tag = if self.auto_scroll {
            "[Auto-scroll: ON]"
        } else {
            "[Auto-scroll: OFF]"
        };
        let filter_tag = if self.filter.is_empty() {
            "[/ Filter: (none)]".to_string()
        } else {
            format!("[/ Filter: '{}']", self.filter)
        };
        let header = format!(
            " Live Operations Log ({} lines) {} {} [c Clear]",
            self.lines.len(),
            autoscroll_tag,
            filter_tag
        );

        for (i, ch) in header.chars().enumerate() {
            let x = area.x + 1 + i as u16;
            if x < area.right() {
                buf[(x, area.y)].set_char(ch).set_style(
                    Style::default()
                        .fg(theme.accent)
                        .bg(theme.panel_bg)
                        .add_modifier(Modifier::BOLD),
                );
            }
        }

        // Divider
        let div_y = area.y + 1;
        for x in area.x..area.right() {
            buf[(x, div_y)].set_char('─').set_style(border_style);
        }

        let content_y = area.y + 2;
        let content_h = (area.height.saturating_sub(2)) as usize;
        let filtered = self.filtered_lines();

        let start_idx = self
            .scroll_offset
            .min(filtered.len().saturating_sub(content_h));
        let slice = &filtered[start_idx..filtered.len().min(start_idx + content_h)];

        for (row_idx, line) in slice.iter().enumerate() {
            let y = content_y + row_idx as u16;
            if y >= area.bottom() {
                break;
            }

            // Determine line style
            let line_style = match classify_log_line(line) {
                LogLevel::Error => Style::default()
                    .fg(theme.error)
                    .bg(theme.panel_bg)
                    .add_modifier(Modifier::BOLD),
                LogLevel::Warning => Style::default().fg(theme.warning).bg(theme.panel_bg),
                LogLevel::Success => Style::default()
                    .fg(theme.success)
                    .bg(theme.panel_bg)
                    .add_modifier(Modifier::BOLD),
                LogLevel::Info => Style::default().fg(theme.info).bg(theme.panel_bg),
                LogLevel::Normal => Style::default().fg(theme.text_primary).bg(theme.panel_bg),
            };

            let prefix = format!("{:04} │ ", start_idx + row_idx + 1);
            let prefix_style = Style::default().fg(theme.text_muted).bg(theme.panel_bg);

            for (i, ch) in prefix.chars().enumerate() {
                let x = area.x + 1 + i as u16;
                if x < area.right() {
                    buf[(x, y)].set_char(ch).set_style(prefix_style);
                }
            }

            let text_start_x = area.x + 1 + prefix.len() as u16;
            let max_w = (area.right().saturating_sub(text_start_x)) as usize;

            for (i, ch) in line.chars().take(max_w).enumerate() {
                let x = text_start_x + i as u16;
                buf[(x, y)].set_char(ch).set_style(line_style);
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogLevel {
    Error,
    Warning,
    Success,
    Info,
    Normal,
}

pub fn classify_log_line(line: &str) -> LogLevel {
    let clean = line
        .strip_prefix("[STDERR] ")
        .or_else(|| line.strip_prefix("[STDERR]"))
        .unwrap_or(line)
        .trim();

    // 1. Success / completion indicators (green)
    if clean.contains("successfully")
        || clean.starts_with("[SUCCESS]")
        || clean.contains("Lab successfully deployed")
        || clean.contains("Lab successfully destroyed")
        || clean.contains("All containers created")
        || clean.contains("Configuration applied successfully")
    {
        return LogLevel::Success;
    }

    // 2. Genuine errors (red)
    let is_err = clean.starts_with("ERRO[")
        || clean.starts_with("FATAL[")
        || clean.starts_with("PANIC[")
        || clean.starts_with("ERROR")
        || clean.starts_with("[ERROR]")
        || clean.starts_with("[FAILED]")
        || clean.starts_with("Error:")
        || clean.starts_with("error:")
        || clean.starts_with("Fatal:")
        || clean.starts_with("fatal:")
        || clean.starts_with("panic:")
        || clean.contains("level=error")
        || clean.contains("level=fatal")
        || clean.contains("failed to create")
        || clean.contains("failed to deploy")
        || clean.contains("failed to start")
        || clean.contains("permission denied")
        || clean.contains("cannot create");

    if is_err {
        return LogLevel::Error;
    }

    // 3. Warnings (yellow)
    if clean.starts_with("WARN[")
        || clean.starts_with("WARNING")
        || clean.starts_with("[WARN]")
        || clean.starts_with("Warning:")
        || clean.contains("level=warning")
        || clean.contains("level=warn")
    {
        return LogLevel::Warning;
    }

    // 4. Info / Debug / Progress / Containerlab table / Docker logs (cyan/info)
    if clean.starts_with("INFO[")
        || clean.starts_with("[INFO]")
        || clean.starts_with("DEBU[")
        || clean.starts_with("[DEBUG]")
        || clean.contains("level=info")
        || clean.contains("level=debug")
        || clean.starts_with('+')
        || clean.starts_with('|')
        || clean.contains("Pulling fs layer")
        || clean.contains("Downloading")
        || clean.contains("Extracting")
        || clean.contains("Pull complete")
        || clean.contains("Download complete")
        || clean.contains("Already exists")
        || clean.contains("Verifying Checksum")
        || clean.contains("Pulling from")
        || clean.contains("Waiting")
        || clean.contains("Digest:")
        || clean.contains("Status:")
        || clean.contains("Creating container")
        || clean.contains("Creating docker network")
        || clean.contains("Creating lab directory")
        || clean.contains("Parsing & validating topology")
        || clean.contains("Removing lab directory")
    {
        return LogLevel::Info;
    }

    LogLevel::Normal
}
