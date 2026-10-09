use crate::canvas::render::CanvasRenderer;
use crate::canvas::state::CanvasState;
use crate::clab::model::{LabTopology, NodeCategory, NodeProfile};
use crate::ui::theme::Theme;
use crate::ui::widgets::drawer::InspectorDrawer;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};

#[derive(Debug, Clone)]
pub struct AddNodeModal {
    pub is_open: bool,
    pub selected_index: usize,
    pub spawn_pos: (f64, f64),
}

impl Default for AddNodeModal {
    fn default() -> Self {
        Self {
            is_open: false,
            selected_index: 0,
            spawn_pos: (20.0, 10.0),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProfileField {
    KindName,
    DisplayName,
    Image,
    Ports,
    InterfacePattern,
    Category,
    Description,
    Save,
    Cancel,
}

impl ProfileField {
    pub fn next(self) -> Self {
        match self {
            ProfileField::KindName => ProfileField::DisplayName,
            ProfileField::DisplayName => ProfileField::Image,
            ProfileField::Image => ProfileField::Ports,
            ProfileField::Ports => ProfileField::InterfacePattern,
            ProfileField::InterfacePattern => ProfileField::Category,
            ProfileField::Category => ProfileField::Description,
            ProfileField::Description => ProfileField::Save,
            ProfileField::Save => ProfileField::Cancel,
            ProfileField::Cancel => ProfileField::KindName,
        }
    }

    pub fn prev(self) -> Self {
        match self {
            ProfileField::KindName => ProfileField::Cancel,
            ProfileField::DisplayName => ProfileField::KindName,
            ProfileField::Image => ProfileField::DisplayName,
            ProfileField::Ports => ProfileField::Image,
            ProfileField::InterfacePattern => ProfileField::Ports,
            ProfileField::Category => ProfileField::InterfacePattern,
            ProfileField::Description => ProfileField::Category,
            ProfileField::Save => ProfileField::Description,
            ProfileField::Cancel => ProfileField::Save,
        }
    }
}

#[derive(Debug, Clone)]
pub struct ProfileEditModal {
    pub is_open: bool,
    pub is_new: bool,
    pub editing_index: Option<usize>,
    pub focused_field: ProfileField,
    pub kind_name: String,
    pub display_name: String,
    pub image: String,
    pub ports: String,
    pub interface_pattern: String,
    pub category: NodeCategory,
    pub description: String,
    pub error_msg: Option<String>,
}

impl Default for ProfileEditModal {
    fn default() -> Self {
        Self {
            is_open: false,
            is_new: true,
            editing_index: None,
            focused_field: ProfileField::KindName,
            kind_name: String::new(),
            display_name: String::new(),
            image: "alpine:latest".to_string(),
            ports: "eth1, eth2".to_string(),
            interface_pattern: "eth{n:1}".to_string(),
            category: NodeCategory::Router,
            description: String::new(),
            error_msg: None,
        }
    }
}

impl ProfileEditModal {
    pub fn open_for_new(&mut self) {
        self.is_open = true;
        self.is_new = true;
        self.editing_index = None;
        self.focused_field = ProfileField::KindName;
        self.kind_name = String::new();
        self.display_name = String::new();
        self.image = "alpine:latest".to_string();
        self.ports = "eth1, eth2".to_string();
        self.interface_pattern = "eth{n:1}".to_string();
        self.category = NodeCategory::Router;
        self.description = String::new();
        self.error_msg = None;
    }

    pub fn open_for_edit(&mut self, index: usize, profile: &NodeProfile) {
        self.is_open = true;
        self.is_new = false;
        self.editing_index = Some(index);
        self.focused_field = ProfileField::KindName;
        self.kind_name = profile.kind_name.clone();
        self.display_name = profile.display_name.clone();
        self.image = profile.default_image.clone();
        self.ports = profile.default_ports.join(", ");
        self.interface_pattern = profile.interface_pattern.clone();
        self.category = profile.node_category;
        self.description = profile.description.clone();
        self.error_msg = None;
    }

    pub fn to_profile(&self) -> Result<NodeProfile, String> {
        let kind = self.kind_name.trim();
        if kind.is_empty() {
            return Err("Kind name cannot be empty".to_string());
        }
        if !kind
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
        {
            return Err(
                "Kind name must only contain alphanumeric, dash, or underscore".to_string(),
            );
        }

        let display = if self.display_name.trim().is_empty() {
            kind.to_string()
        } else {
            self.display_name.trim().to_string()
        };

        let image = if self.image.trim().is_empty() {
            "alpine:latest".to_string()
        } else {
            self.image.trim().to_string()
        };

        let ports: Vec<String> = self
            .ports
            .split([',', ' ', ';'])
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();

        let ports = if ports.is_empty() {
            vec!["eth1".to_string(), "eth2".to_string()]
        } else {
            ports
        };

        let pattern = if self.interface_pattern.trim().is_empty() {
            crate::clab::model::default_interface_pattern()
        } else {
            self.interface_pattern.trim().to_string()
        };
        crate::clab::pattern::InterfacePattern::validate(&pattern)?;

        let description = if self.description.trim().is_empty() {
            format!("{} node profile", display)
        } else {
            self.description.trim().to_string()
        };

        Ok(NodeProfile {
            kind_name: kind.to_string(),
            display_name: display,
            default_image: image,
            default_ports: ports,
            interface_pattern: pattern,
            description,
            node_category: self.category,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LinkEditField {
    SourcePort,
    TargetPort,
    Save,
    Cancel,
}

impl LinkEditField {
    pub fn next(self) -> Self {
        match self {
            LinkEditField::SourcePort => LinkEditField::TargetPort,
            LinkEditField::TargetPort => LinkEditField::Save,
            LinkEditField::Save => LinkEditField::Cancel,
            LinkEditField::Cancel => LinkEditField::SourcePort,
        }
    }

    pub fn prev(self) -> Self {
        match self {
            LinkEditField::SourcePort => LinkEditField::Cancel,
            LinkEditField::TargetPort => LinkEditField::SourcePort,
            LinkEditField::Save => LinkEditField::TargetPort,
            LinkEditField::Cancel => LinkEditField::Save,
        }
    }
}

#[derive(Debug, Clone)]
pub struct LinkEditModal {
    pub is_open: bool,
    pub link_index: usize,
    pub source_node: String,
    pub target_node: String,
    pub source_port: String,
    pub target_port: String,
    pub focused_field: LinkEditField,
    pub error_msg: Option<String>,
}

impl Default for LinkEditModal {
    fn default() -> Self {
        Self {
            is_open: false,
            link_index: 0,
            source_node: String::new(),
            target_node: String::new(),
            source_port: String::new(),
            target_port: String::new(),
            focused_field: LinkEditField::SourcePort,
            error_msg: None,
        }
    }
}

impl LinkEditModal {
    pub fn open_for_link(
        &mut self,
        link_index: usize,
        source_node: &str,
        source_port: &str,
        target_node: &str,
        target_port: &str,
    ) {
        self.is_open = true;
        self.link_index = link_index;
        self.source_node = source_node.to_string();
        self.source_port = source_port.to_string();
        self.target_node = target_node.to_string();
        self.target_port = target_port.to_string();
        self.focused_field = LinkEditField::SourcePort;
        self.error_msg = None;
    }

    pub fn close(&mut self) {
        self.is_open = false;
        self.error_msg = None;
    }
}

pub struct CanvasViewParams<'a> {
    pub canvas: &'a CanvasState,
    pub topo: &'a LabTopology,
    pub drawer: &'a InspectorDrawer,
    pub add_modal: &'a AddNodeModal,
    pub profile_modal: Option<&'a ProfileEditModal>,
    pub link_modal: Option<&'a LinkEditModal>,
    pub profiles: &'a [NodeProfile],
    pub theme: &'a Theme,
}

pub struct CanvasView;

impl CanvasView {
    pub fn render(params: CanvasViewParams<'_>, area: Rect, buf: &mut Buffer) {
        if area.width < 10 || area.height < 5 {
            return;
        }

        // Render 2D network canvas (Unicode/Braille mode)
        CanvasRenderer::render(params.canvas, area, buf, params.theme);

        // Render Inspector Drawer if open
        if params.drawer.is_open {
            params.drawer.render_with_profiles(
                params.canvas,
                params.topo,
                params.profiles,
                area,
                buf,
                params.theme,
            );
        }

        // Render Add Node Modal if open
        if params.add_modal.is_open {
            Self::render_add_modal(params.add_modal, params.profiles, area, buf, params.theme);
        }

        // Render Profile Edit Modal if open
        if let Some(p_modal) = params.profile_modal {
            if p_modal.is_open {
                Self::render_profile_modal(p_modal, area, buf, params.theme);
            }
        }

        // Render Link Edit Modal if open
        if let Some(l_modal) = params.link_modal {
            if l_modal.is_open {
                Self::render_link_modal(l_modal, area, buf, params.theme);
            }
        }
    }

    fn render_add_modal(
        modal: &AddNodeModal,
        profiles: &[NodeProfile],
        area: Rect,
        buf: &mut Buffer,
        theme: &Theme,
    ) {
        let modal_width = (area.width * 85 / 100)
            .clamp(82, 116)
            .min(area.width.saturating_sub(4));
        let modal_height = (profiles.len() as u16 + 6)
            .min(area.height.saturating_sub(2))
            .max(8);

        let modal_x = area.x + (area.width.saturating_sub(modal_width)) / 2;
        let modal_y = area.y + (area.height.saturating_sub(modal_height)) / 2;
        let modal_rect = Rect::new(modal_x, modal_y, modal_width, modal_height);

        let border_style = Style::default().fg(theme.accent).bg(theme.panel_bg);

        // Fill background & draw border
        for y in modal_rect.y..modal_rect.bottom() {
            for x in modal_rect.x..modal_rect.right() {
                let ch = if y == modal_rect.y && x == modal_rect.x {
                    '╭'
                } else if y == modal_rect.y && x == modal_rect.right() - 1 {
                    '╮'
                } else if y == modal_rect.bottom() - 1 && x == modal_rect.x {
                    '╰'
                } else if y == modal_rect.bottom() - 1 && x == modal_rect.right() - 1 {
                    '╯'
                } else if y == modal_rect.y || y == modal_rect.bottom() - 1 {
                    '─'
                } else if x == modal_rect.x || x == modal_rect.right() - 1 {
                    '│'
                } else {
                    ' '
                };

                buf[(x, y)].set_char(ch).set_style(border_style);
            }
        }

        // Title
        let title = " Add Network Node (Select Profile) ";
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

        // List profiles with scrolling window so selected item is always visible
        let max_visible = (modal_rect.height.saturating_sub(4) as usize).max(1);
        let scroll_offset = if modal.selected_index >= max_visible {
            modal.selected_index - max_visible + 1
        } else {
            0
        };

        for (display_idx, (idx, prof)) in profiles
            .iter()
            .enumerate()
            .skip(scroll_offset)
            .take(max_visible)
            .enumerate()
        {
            let row_y = modal_rect.y + 2 + display_idx as u16;
            if row_y >= modal_rect.bottom() - 2 {
                break;
            }

            let is_sel = modal.selected_index == idx;
            let row_style = if is_sel {
                Style::default()
                    .fg(theme.accent)
                    .bg(theme.bg)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(theme.text_primary).bg(theme.panel_bg)
            };

            let pointer = if is_sel { "▶ " } else { "  " };
            let line_text = format!(
                "{}{:<22} {:<24} [{:<8}] {}",
                pointer, prof.kind_name, prof.display_name, prof.node_category, prof.description
            );

            for (i, ch) in line_text
                .chars()
                .take((modal_rect.width - 4) as usize)
                .enumerate()
            {
                let px = modal_rect.x + 2 + i as u16;
                buf[(px, row_y)].set_char(ch).set_style(row_style);
            }
        }

        // Footer instructions
        let footer_y = modal_rect.bottom() - 2;
        let footer = "Enter: Place | n: New Profile | e: Edit | d: Delete | Esc: Cancel";
        let footer_style = Style::default().fg(theme.text_muted).bg(theme.panel_bg);
        for (i, ch) in footer.chars().enumerate() {
            let px = modal_rect.x + 3 + i as u16;
            if px < modal_rect.right() - 1 {
                buf[(px, footer_y)].set_char(ch).set_style(footer_style);
            }
        }
    }

    fn render_profile_modal(modal: &ProfileEditModal, area: Rect, buf: &mut Buffer, theme: &Theme) {
        let modal_width = (area.width * 80 / 100)
            .clamp(78, 100)
            .min(area.width.saturating_sub(4));
        let modal_height = 22.min(area.height.saturating_sub(2));

        let modal_x = area.x + (area.width.saturating_sub(modal_width)) / 2;
        let modal_y = area.y + (area.height.saturating_sub(modal_height)) / 2;
        let modal_rect = Rect::new(modal_x, modal_y, modal_width, modal_height);

        let border_style = Style::default().fg(theme.accent).bg(theme.panel_bg);

        // Fill background & draw border
        for y in modal_rect.y..modal_rect.bottom() {
            for x in modal_rect.x..modal_rect.right() {
                let ch = if y == modal_rect.y && x == modal_rect.x {
                    '╭'
                } else if y == modal_rect.y && x == modal_rect.right() - 1 {
                    '╮'
                } else if y == modal_rect.bottom() - 1 && x == modal_rect.x {
                    '╰'
                } else if y == modal_rect.bottom() - 1 && x == modal_rect.right() - 1 {
                    '╯'
                } else if y == modal_rect.y || y == modal_rect.bottom() - 1 {
                    '─'
                } else if x == modal_rect.x || x == modal_rect.right() - 1 {
                    '│'
                } else {
                    ' '
                };

                buf[(x, y)].set_char(ch).set_style(border_style);
            }
        }

        // Title
        let title = if modal.is_new {
            " New Node Profile "
        } else {
            " Edit Node Profile "
        };
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

        let cat_str = format!("< {} > (Space/Tab to cycle)", modal.category);
        let fields = [
            (ProfileField::KindName, "Kind Name:   ", &modal.kind_name),
            (
                ProfileField::DisplayName,
                "Display Name:",
                &modal.display_name,
            ),
            (ProfileField::Image, "Default Img: ", &modal.image),
            (ProfileField::Ports, "Ports (csv): ", &modal.ports),
            (
                ProfileField::InterfacePattern,
                "Pattern:     ",
                &modal.interface_pattern,
            ),
            (ProfileField::Category, "Category:    ", &cat_str),
            (
                ProfileField::Description,
                "Description: ",
                &modal.description,
            ),
        ];

        let content_start_y = modal_rect.y + 2;
        for (idx, (field, label, value)) in fields.iter().enumerate() {
            let row_y = content_start_y + (idx as u16) * 2;
            if row_y >= modal_rect.bottom() - 4 {
                break;
            }

            let is_focused = modal.focused_field == *field;
            let label_style = if is_focused {
                Style::default()
                    .fg(theme.accent)
                    .bg(theme.panel_bg)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(theme.text_secondary).bg(theme.panel_bg)
            };

            let val_style = if is_focused {
                Style::default()
                    .fg(Color::Black)
                    .bg(theme.accent)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(theme.text_primary).bg(theme.bg)
            };

            // Draw label
            for (i, ch) in label.chars().enumerate() {
                let px = modal_rect.x + 3 + i as u16;
                if px < modal_rect.right() - 1 {
                    buf[(px, row_y)].set_char(ch).set_style(label_style);
                }
            }

            // Draw value box
            let val_x = modal_rect.x + 18;
            let val_w = (modal_rect.width.saturating_sub(21)) as usize;
            for i in 0..val_w {
                let px = val_x + i as u16;
                if px < modal_rect.right() - 1 {
                    buf[(px, row_y)].set_char(' ').set_style(val_style);
                }
            }

            let val_str = if is_focused && *field != ProfileField::Category {
                format!("{}_", value)
            } else {
                value.to_string()
            };

            for (i, ch) in val_str.chars().take(val_w).enumerate() {
                let px = val_x + i as u16;
                if px < modal_rect.right() - 1 {
                    buf[(px, row_y)].set_char(ch).set_style(val_style);
                }
            }
        }

        // Action buttons: [ Save ]  [ Cancel ]
        let btn_y = modal_rect.bottom() - 3;
        let save_focused = modal.focused_field == ProfileField::Save;
        let cancel_focused = modal.focused_field == ProfileField::Cancel;

        let save_style = if save_focused {
            Style::default()
                .fg(Color::Black)
                .bg(theme.success)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(theme.success).bg(theme.panel_bg)
        };

        let cancel_style = if cancel_focused {
            Style::default()
                .fg(Color::Black)
                .bg(theme.error)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(theme.text_muted).bg(theme.panel_bg)
        };

        let save_txt = " [ Save Profile (Enter) ] ";
        let cancel_txt = " [ Cancel (Esc) ] ";

        let save_x = modal_rect.x + 4;
        for (i, ch) in save_txt.chars().enumerate() {
            let px = save_x + i as u16;
            if px < modal_rect.right() - 1 {
                buf[(px, btn_y)].set_char(ch).set_style(save_style);
            }
        }

        let cancel_x = save_x + save_txt.len() as u16 + 2;
        for (i, ch) in cancel_txt.chars().enumerate() {
            let px = cancel_x + i as u16;
            if px < modal_rect.right() - 1 {
                buf[(px, btn_y)].set_char(ch).set_style(cancel_style);
            }
        }

        // Error message if any
        if let Some(ref err) = modal.error_msg {
            let err_y = modal_rect.bottom() - 2;
            let err_txt = format!(" ⚠ {}", err);
            let err_style = Style::default()
                .fg(theme.error)
                .bg(theme.panel_bg)
                .add_modifier(Modifier::BOLD);
            for (i, ch) in err_txt
                .chars()
                .take((modal_rect.width - 4) as usize)
                .enumerate()
            {
                let px = modal_rect.x + 3 + i as u16;
                if px < modal_rect.right() - 1 {
                    buf[(px, err_y)].set_char(ch).set_style(err_style);
                }
            }
        } else {
            // Footer helper
            let helper_y = modal_rect.bottom() - 2;
            let helper_txt = "Tab/Shift+Tab: Navigate | Enter: Save | Esc: Cancel";
            let helper_style = Style::default().fg(theme.text_muted).bg(theme.panel_bg);
            for (i, ch) in helper_txt
                .chars()
                .take((modal_rect.width - 4) as usize)
                .enumerate()
            {
                let px = modal_rect.x + 3 + i as u16;
                if px < modal_rect.right() - 1 {
                    buf[(px, helper_y)].set_char(ch).set_style(helper_style);
                }
            }
        }
    }

    fn render_link_modal(modal: &LinkEditModal, area: Rect, buf: &mut Buffer, theme: &Theme) {
        let modal_width = 64.min(area.width.saturating_sub(4));
        let modal_height = 14.min(area.height.saturating_sub(2));

        let modal_x = area.x + (area.width.saturating_sub(modal_width)) / 2;
        let modal_y = area.y + (area.height.saturating_sub(modal_height)) / 2;
        let modal_rect = Rect::new(modal_x, modal_y, modal_width, modal_height);

        let border_style = Style::default().fg(theme.accent).bg(theme.panel_bg);

        // Fill background & draw border
        for y in modal_rect.y..modal_rect.bottom() {
            for x in modal_rect.x..modal_rect.right() {
                let ch = if y == modal_rect.y && x == modal_rect.x {
                    '╭'
                } else if y == modal_rect.y && x == modal_rect.right() - 1 {
                    '╮'
                } else if y == modal_rect.bottom() - 1 && x == modal_rect.x {
                    '╰'
                } else if y == modal_rect.bottom() - 1 && x == modal_rect.right() - 1 {
                    '╯'
                } else if y == modal_rect.y || y == modal_rect.bottom() - 1 {
                    '─'
                } else if x == modal_rect.x || x == modal_rect.right() - 1 {
                    '│'
                } else {
                    ' '
                };

                buf[(x, y)].set_char(ch).set_style(border_style);
            }
        }

        // Title
        let title = " Edit Link Interfaces ";
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

        // Top info: Source and Target nodes
        let info = format!(
            "Link between: {} ↔ {}",
            modal.source_node, modal.target_node
        );
        let info_style = Style::default().fg(theme.text_secondary).bg(theme.panel_bg);
        for (i, ch) in info
            .chars()
            .take((modal_rect.width.saturating_sub(6)) as usize)
            .enumerate()
        {
            let px = modal_rect.x + 3 + i as u16;
            if px < modal_rect.right() - 1 {
                buf[(px, modal_rect.y + 2)]
                    .set_char(ch)
                    .set_style(info_style);
            }
        }

        let label_src = format!("{}:", modal.source_node);
        let label_tgt = format!("{}:", modal.target_node);
        let fields = [
            (LinkEditField::SourcePort, label_src, &modal.source_port),
            (LinkEditField::TargetPort, label_tgt, &modal.target_port),
        ];

        let content_start_y = modal_rect.y + 4;
        for (idx, (field, label, value)) in fields.iter().enumerate() {
            let row_y = content_start_y + (idx as u16) * 2;
            let is_focused = modal.focused_field == *field;
            let label_style = if is_focused {
                Style::default()
                    .fg(theme.accent)
                    .bg(theme.panel_bg)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(theme.text_secondary).bg(theme.panel_bg)
            };

            let val_style = if is_focused {
                Style::default()
                    .fg(Color::Black)
                    .bg(theme.accent)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(theme.text_primary).bg(theme.bg)
            };

            // Draw label
            for (i, ch) in label.chars().take(16).enumerate() {
                let px = modal_rect.x + 3 + i as u16;
                if px < modal_rect.right() - 1 {
                    buf[(px, row_y)].set_char(ch).set_style(label_style);
                }
            }

            // Draw value box
            let val_x = modal_rect.x + 20;
            let val_w = (modal_rect.width.saturating_sub(23)) as usize;
            for i in 0..val_w {
                let px = val_x + i as u16;
                if px < modal_rect.right() - 1 {
                    buf[(px, row_y)].set_char(' ').set_style(val_style);
                }
            }

            let val_str = if is_focused {
                format!("{}_", value)
            } else {
                value.to_string()
            };

            for (i, ch) in val_str.chars().take(val_w).enumerate() {
                let px = val_x + i as u16;
                if px < modal_rect.right() - 1 {
                    buf[(px, row_y)].set_char(ch).set_style(val_style);
                }
            }
        }

        // Action buttons: [ Save ]  [ Cancel ]
        let btn_y = modal_rect.bottom() - 3;
        let save_focused = modal.focused_field == LinkEditField::Save;
        let cancel_focused = modal.focused_field == LinkEditField::Cancel;

        let save_style = if save_focused {
            Style::default()
                .fg(Color::Black)
                .bg(theme.success)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(theme.success).bg(theme.panel_bg)
        };

        let cancel_style = if cancel_focused {
            Style::default()
                .fg(Color::Black)
                .bg(theme.error)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(theme.text_muted).bg(theme.panel_bg)
        };

        let save_txt = " [ Save Link (Enter) ] ";
        let cancel_txt = " [ Cancel (Esc) ] ";

        let save_x = modal_rect.x + 4;
        for (i, ch) in save_txt.chars().enumerate() {
            let px = save_x + i as u16;
            if px < modal_rect.right() - 1 {
                buf[(px, btn_y)].set_char(ch).set_style(save_style);
            }
        }

        let cancel_x = save_x + save_txt.len() as u16 + 2;
        for (i, ch) in cancel_txt.chars().enumerate() {
            let px = cancel_x + i as u16;
            if px < modal_rect.right() - 1 {
                buf[(px, btn_y)].set_char(ch).set_style(cancel_style);
            }
        }

        // Error message or footer
        if let Some(ref err) = modal.error_msg {
            let err_y = modal_rect.bottom() - 2;
            let err_txt = format!(" ⚠ {}", err);
            let err_style = Style::default()
                .fg(theme.error)
                .bg(theme.panel_bg)
                .add_modifier(Modifier::BOLD);
            for (i, ch) in err_txt
                .chars()
                .take((modal_rect.width.saturating_sub(4)) as usize)
                .enumerate()
            {
                let px = modal_rect.x + 3 + i as u16;
                if px < modal_rect.right() - 1 {
                    buf[(px, err_y)].set_char(ch).set_style(err_style);
                }
            }
        } else {
            let helper_y = modal_rect.bottom() - 2;
            let helper_txt = "Tab: Navigate | Enter: Save | Esc: Cancel";
            let helper_style = Style::default().fg(theme.text_muted).bg(theme.panel_bg);
            for (i, ch) in helper_txt
                .chars()
                .take((modal_rect.width.saturating_sub(4)) as usize)
                .enumerate()
            {
                let px = modal_rect.x + 3 + i as u16;
                if px < modal_rect.right() - 1 {
                    buf[(px, helper_y)].set_char(ch).set_style(helper_style);
                }
            }
        }
    }
}
