use crate::ui::theme::Theme;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileEntryType {
    ParentDir,
    Directory,
    TopologyFile,
    OtherFile,
}

#[derive(Debug, Clone)]
pub struct FileBrowserEntry {
    pub name: String,
    pub path: PathBuf,
    pub entry_type: FileEntryType,
    pub is_dir: bool,
}

#[derive(Debug, Clone)]
pub struct FileBrowserModal {
    pub is_open: bool,
    pub current_dir: PathBuf,
    pub entries: Vec<FileBrowserEntry>,
    pub selected_index: usize,
    pub scroll_offset: usize,
    pub error_msg: Option<String>,
}

impl Default for FileBrowserModal {
    fn default() -> Self {
        Self::new()
    }
}

impl FileBrowserModal {
    pub fn new() -> Self {
        let current_dir = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        let mut modal = Self {
            is_open: false,
            current_dir,
            entries: Vec::new(),
            selected_index: 0,
            scroll_offset: 0,
            error_msg: None,
        };
        modal.refresh();
        modal
    }

    /// Open file browser modal at initial path or current directory
    pub fn open(&mut self, initial_path: Option<&Path>) {
        if let Some(path) = initial_path {
            if path.is_file() {
                if let Some(parent) = path.parent() {
                    if parent.exists() && parent.is_dir() {
                        self.current_dir = parent.to_path_buf();
                    }
                }
            } else if path.is_dir() && path.exists() {
                self.current_dir = path.to_path_buf();
            } else if let Some(parent) = path.parent() {
                if parent.exists() && parent.is_dir() {
                    self.current_dir = parent.to_path_buf();
                }
            }
        }

        if !self.current_dir.exists() {
            self.current_dir = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        }

        self.is_open = true;
        self.selected_index = 0;
        self.scroll_offset = 0;
        self.error_msg = None;
        self.refresh();
    }

    pub fn close(&mut self) {
        self.is_open = false;
        self.error_msg = None;
    }

    /// Refresh directory listing
    pub fn refresh(&mut self) {
        self.entries.clear();
        self.error_msg = None;

        // Add parent directory ".." entry if parent exists
        if let Some(parent) = self.current_dir.parent() {
            self.entries.push(FileBrowserEntry {
                name: "..".to_string(),
                path: parent.to_path_buf(),
                entry_type: FileEntryType::ParentDir,
                is_dir: true,
            });
        }

        let read_res = fs::read_dir(&self.current_dir);
        let rd = match read_res {
            Ok(rd) => rd,
            Err(e) => {
                self.error_msg = Some(format!("Failed to read directory: {}", e));
                return;
            }
        };

        let mut dirs = Vec::new();
        let mut topo_files = Vec::new();
        let mut other_files = Vec::new();

        for item in rd.flatten() {
            let path = item.path();
            let file_name = match item.file_name().into_string() {
                Ok(name) => name,
                Err(_) => continue,
            };

            // Skip hidden entries except ..
            if file_name.starts_with('.') {
                continue;
            }

            let is_dir = path.is_dir();
            if is_dir {
                dirs.push(FileBrowserEntry {
                    name: file_name,
                    path,
                    entry_type: FileEntryType::Directory,
                    is_dir: true,
                });
            } else {
                let lower = file_name.to_lowercase();
                let is_topo = lower.ends_with(".clab.yml")
                    || lower.ends_with(".clab.yaml")
                    || lower.ends_with(".yml")
                    || lower.ends_with(".yaml");

                if is_topo {
                    topo_files.push(FileBrowserEntry {
                        name: file_name,
                        path,
                        entry_type: FileEntryType::TopologyFile,
                        is_dir: false,
                    });
                } else {
                    other_files.push(FileBrowserEntry {
                        name: file_name,
                        path,
                        entry_type: FileEntryType::OtherFile,
                        is_dir: false,
                    });
                }
            }
        }

        dirs.sort_by_key(|a| a.name.to_lowercase());
        topo_files.sort_by_key(|a| a.name.to_lowercase());
        other_files.sort_by_key(|a| a.name.to_lowercase());

        self.entries.extend(dirs);
        self.entries.extend(topo_files);
        self.entries.extend(other_files);

        if self.selected_index >= self.entries.len() {
            self.selected_index = self.entries.len().saturating_sub(1);
        }
    }

    pub fn select_next(&mut self) {
        if !self.entries.is_empty() {
            self.selected_index = (self.selected_index + 1) % self.entries.len();
        }
    }

    pub fn select_prev(&mut self) {
        if !self.entries.is_empty() {
            if self.selected_index == 0 {
                self.selected_index = self.entries.len() - 1;
            } else {
                self.selected_index -= 1;
            }
        }
    }

    /// Navigate to parent directory
    pub fn navigate_up(&mut self) {
        if let Some(parent) = self.current_dir.parent() {
            self.current_dir = parent.to_path_buf();
            self.selected_index = 0;
            self.scroll_offset = 0;
            self.refresh();
        }
    }

    /// Activate currently selected entry: enters directory or returns chosen file path
    pub fn activate_selected(&mut self) -> Option<PathBuf> {
        let entry = self.entries.get(self.selected_index)?.clone();
        match entry.entry_type {
            FileEntryType::ParentDir => {
                self.current_dir = entry.path;
                self.selected_index = 0;
                self.scroll_offset = 0;
                self.refresh();
                None
            }
            FileEntryType::Directory => {
                self.current_dir = entry.path;
                self.selected_index = 0;
                self.scroll_offset = 0;
                self.refresh();
                None
            }
            FileEntryType::TopologyFile | FileEntryType::OtherFile => {
                self.is_open = false;
                Some(entry.path)
            }
        }
    }

    pub fn render(&self, area: Rect, buf: &mut Buffer, theme: &Theme) {
        if !self.is_open || area.width < 20 || area.height < 8 {
            return;
        }

        let width = 74.min(area.width.saturating_sub(4));
        let height = 22.min(area.height.saturating_sub(2));

        let x = area.x + (area.width.saturating_sub(width)) / 2;
        let y = area.y + (area.height.saturating_sub(height)) / 2;
        let modal_rect = Rect::new(x, y, width, height);

        let border_style = Style::default().fg(theme.accent).bg(theme.panel_bg);
        let bg_style = Style::default().bg(theme.panel_bg);

        // Fill background & borders
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

                let style = if py == modal_rect.y
                    || py == modal_rect.bottom() - 1
                    || px == modal_rect.x
                    || px == modal_rect.right() - 1
                {
                    border_style
                } else {
                    bg_style
                };

                buf[(px, py)].set_char(ch).set_style(style);
            }
        }

        // Title
        let title = " Select File / Topology ";
        let title_style = Style::default()
            .fg(theme.accent)
            .bg(theme.panel_bg)
            .add_modifier(Modifier::BOLD);
        for (i, ch) in title.chars().enumerate() {
            let px = modal_rect.x + 3 + i as u16;
            if px < modal_rect.right() - 1 {
                buf[(px, modal_rect.y)].set_char(ch).set_style(title_style);
            }
        }

        // Current directory path row
        let cur_dir_str = format!("Directory: {}", self.current_dir.display());
        let cur_dir_style = Style::default().fg(theme.info).bg(theme.panel_bg);
        let max_dir_w = modal_rect.width.saturating_sub(6) as usize;
        for (i, ch) in cur_dir_str.chars().take(max_dir_w).enumerate() {
            let px = modal_rect.x + 3 + i as u16;
            if px < modal_rect.right() - 2 {
                buf[(px, modal_rect.y + 1)]
                    .set_char(ch)
                    .set_style(cur_dir_style);
            }
        }

        // Divider
        for px in (modal_rect.x + 1)..(modal_rect.right() - 1) {
            buf[(px, modal_rect.y + 2)]
                .set_char('─')
                .set_style(border_style);
        }

        // Entries list area
        let list_y = modal_rect.y + 3;
        let list_h = modal_rect.height.saturating_sub(6) as usize;

        if let Some(ref err) = self.error_msg {
            let err_style = Style::default().fg(theme.error).bg(theme.panel_bg);
            for (i, ch) in err.chars().enumerate() {
                let px = modal_rect.x + 3 + i as u16;
                if px < modal_rect.right() - 2 {
                    buf[(px, list_y)].set_char(ch).set_style(err_style);
                }
            }
        } else if self.entries.is_empty() {
            let empty_msg = "(Empty directory)";
            let empty_style = Style::default().fg(theme.text_muted).bg(theme.panel_bg);
            for (i, ch) in empty_msg.chars().enumerate() {
                let px = modal_rect.x + 3 + i as u16;
                if px < modal_rect.right() - 2 {
                    buf[(px, list_y)].set_char(ch).set_style(empty_style);
                }
            }
        } else {
            let scroll = if self.selected_index >= list_h {
                self.selected_index.saturating_sub(list_h - 1)
            } else {
                0
            };

            for row in 0..list_h {
                let entry_idx = scroll + row;
                if entry_idx >= self.entries.len() {
                    break;
                }

                let entry = &self.entries[entry_idx];
                let is_sel = entry_idx == self.selected_index;
                let cur_y = list_y + row as u16;

                let row_bg = if is_sel { theme.accent } else { theme.panel_bg };
                let (icon, name_fg) = match entry.entry_type {
                    FileEntryType::ParentDir => (
                        "📁 .. (Parent)",
                        if is_sel {
                            Color::Black
                        } else {
                            theme.accent_alt
                        },
                    ),
                    FileEntryType::Directory => {
                        ("📁 ", if is_sel { Color::Black } else { theme.info })
                    }
                    FileEntryType::TopologyFile => {
                        ("🗎 ", if is_sel { Color::Black } else { theme.success })
                    }
                    FileEntryType::OtherFile => (
                        "📄 ",
                        if is_sel {
                            Color::Black
                        } else {
                            theme.text_secondary
                        },
                    ),
                };

                let item_style = Style::default().fg(name_fg).bg(row_bg).add_modifier(
                    if is_sel || entry.entry_type == FileEntryType::TopologyFile {
                        Modifier::BOLD
                    } else {
                        Modifier::empty()
                    },
                );

                let prefix = if is_sel { "▶ " } else { "  " };
                let display_text = if entry.entry_type == FileEntryType::ParentDir {
                    format!("{}{}", prefix, icon)
                } else if entry.is_dir {
                    format!("{}{}{}/", prefix, icon, entry.name)
                } else if entry.entry_type == FileEntryType::TopologyFile {
                    format!("{}{}{} [Topology]", prefix, icon, entry.name)
                } else {
                    format!("{}{}{}", prefix, icon, entry.name)
                };

                let avail_w = modal_rect.width.saturating_sub(6) as usize;
                // Clear row background
                for col in 0..avail_w {
                    let px = modal_rect.x + 3 + col as u16;
                    if px < modal_rect.right() - 2 {
                        buf[(px, cur_y)].set_char(' ').set_style(item_style);
                    }
                }

                for (i, ch) in display_text.chars().take(avail_w).enumerate() {
                    let px = modal_rect.x + 3 + i as u16;
                    if px < modal_rect.right() - 2 {
                        buf[(px, cur_y)].set_char(ch).set_style(item_style);
                    }
                }
            }
        }

        // Bottom footer divider
        let footer_div_y = modal_rect.bottom().saturating_sub(3);
        for px in (modal_rect.x + 1)..(modal_rect.right() - 1) {
            buf[(px, footer_div_y)]
                .set_char('─')
                .set_style(border_style);
        }

        // Footer instructions
        let instructions =
            "↑/↓/j/k: Navigate | Enter/l: Select/Open | Backspace/h: Up | Esc: Cancel";
        let inst_style = Style::default().fg(theme.text_muted).bg(theme.panel_bg);
        for (i, ch) in instructions.chars().enumerate() {
            let px = modal_rect.x + 3 + i as u16;
            if px < modal_rect.right() - 2 {
                buf[(px, modal_rect.bottom() - 2)]
                    .set_char(ch)
                    .set_style(inst_style);
            }
        }
    }
}
