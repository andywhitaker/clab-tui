use crate::canvas::state::CanvasState;
use crate::clab::model::{LabTopology, NodeProfile};
use crate::ui::theme::Theme;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DrawerField {
    Name,
    Kind,
    Image,
    MgmtIpv4,
    InterfacePattern,
    AddPort,
    DeleteNode,
}

impl DrawerField {
    pub const ALL: [DrawerField; 7] = [
        DrawerField::Name,
        DrawerField::Kind,
        DrawerField::Image,
        DrawerField::MgmtIpv4,
        DrawerField::InterfacePattern,
        DrawerField::AddPort,
        DrawerField::DeleteNode,
    ];

    pub fn next(&self) -> Self {
        match self {
            Self::Name => Self::Kind,
            Self::Kind => Self::Image,
            Self::Image => Self::MgmtIpv4,
            Self::MgmtIpv4 => Self::InterfacePattern,
            Self::InterfacePattern => Self::AddPort,
            Self::AddPort => Self::DeleteNode,
            Self::DeleteNode => Self::Name,
        }
    }

    pub fn prev(&self) -> Self {
        match self {
            Self::Name => Self::DeleteNode,
            Self::Kind => Self::Name,
            Self::Image => Self::Kind,
            Self::MgmtIpv4 => Self::Image,
            Self::InterfacePattern => Self::MgmtIpv4,
            Self::AddPort => Self::InterfacePattern,
            Self::DeleteNode => Self::AddPort,
        }
    }
}

#[derive(Debug, Clone)]
pub struct InspectorDrawer {
    pub is_open: bool,
    pub focused_field: DrawerField,
    pub is_editing: bool,
    pub edit_buffer: String,
    pub target_node_name: Option<String>,
    pub kind_selector_open: bool,
    pub kind_selector_index: usize,
}

impl Default for InspectorDrawer {
    fn default() -> Self {
        Self {
            is_open: false,
            focused_field: DrawerField::Name,
            is_editing: false,
            edit_buffer: String::new(),
            target_node_name: None,
            kind_selector_open: false,
            kind_selector_index: 0,
        }
    }
}

impl InspectorDrawer {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn open_for_node(&mut self, node_name: &str) {
        self.is_open = true;
        self.target_node_name = Some(node_name.to_string());
        self.focused_field = DrawerField::Name;
        self.is_editing = false;
        self.kind_selector_open = false;
        self.kind_selector_index = 0;
        self.edit_buffer.clear();
    }

    pub fn close(&mut self) {
        self.is_open = false;
        self.is_editing = false;
        self.kind_selector_open = false;
        self.kind_selector_index = 0;
        self.edit_buffer.clear();
    }

    pub fn start_editing(&mut self, current_val: &str) {
        self.is_editing = true;
        self.edit_buffer = current_val.to_string();
        if self.focused_field == DrawerField::Kind {
            self.kind_selector_open = true;
        }
    }

    pub fn start_editing_kind(&mut self, current_val: &str, profiles: &[NodeProfile]) {
        self.focused_field = DrawerField::Kind;
        self.is_editing = true;
        self.kind_selector_open = true;
        self.edit_buffer = current_val.to_string();
        self.kind_selector_index = profiles
            .iter()
            .position(|p| p.matches_kind(current_val))
            .unwrap_or(0);
    }

    pub fn select_next_kind_profile(&mut self, profiles: &[NodeProfile]) {
        if profiles.is_empty() {
            return;
        }
        self.kind_selector_index = (self.kind_selector_index + 1) % profiles.len();
        if let Some(p) = profiles.get(self.kind_selector_index) {
            self.edit_buffer = p.kind_name.clone();
        }
    }

    pub fn select_prev_kind_profile(&mut self, profiles: &[NodeProfile]) {
        if profiles.is_empty() {
            return;
        }
        if self.kind_selector_index == 0 {
            self.kind_selector_index = profiles.len().saturating_sub(1);
        } else {
            self.kind_selector_index -= 1;
        }
        if let Some(p) = profiles.get(self.kind_selector_index) {
            self.edit_buffer = p.kind_name.clone();
        }
    }

    pub fn sync_kind_selector_from_buffer(&mut self, profiles: &[NodeProfile]) {
        let trimmed = self.edit_buffer.trim().to_lowercase();
        if trimmed.is_empty() {
            return;
        }
        if let Some(idx) = profiles.iter().position(|p| {
            p.matches_kind(&trimmed)
                || p.kind_name.to_lowercase().starts_with(&trimmed)
                || p.display_name.to_lowercase().starts_with(&trimmed)
        }) {
            self.kind_selector_index = idx;
        }
    }

    pub fn apply_edit(
        &mut self,
        canvas: &mut CanvasState,
        topo: &mut LabTopology,
    ) -> Option<String> {
        let default_profs = NodeProfile::default_profiles();
        self.apply_edit_with_profiles(canvas, topo, &default_profs)
    }

    pub fn apply_edit_with_profiles(
        &mut self,
        canvas: &mut CanvasState,
        topo: &mut LabTopology,
        profiles: &[NodeProfile],
    ) -> Option<String> {
        let node_name = self.target_node_name.clone()?;
        self.is_editing = false;
        self.kind_selector_open = false;
        let new_val = self.edit_buffer.trim().to_string();

        match self.focused_field {
            DrawerField::Name => {
                if !new_val.is_empty()
                    && new_val != node_name
                    && !canvas.nodes.contains_key(&new_val)
                {
                    if let Some(mut node) = canvas.nodes.remove(&node_name) {
                        node.name = new_val.clone();
                        canvas.nodes.insert(new_val.clone(), node);

                        // Update links
                        for link in &mut canvas.links {
                            if link.source_node == node_name {
                                link.source_node = new_val.clone();
                            }
                            if link.target_node == node_name {
                                link.target_node = new_val.clone();
                            }
                        }

                        // Update topology
                        let def = topo.topology.nodes.remove(&node_name).unwrap_or_default();
                        topo.topology.nodes.insert(new_val.clone(), def);

                        self.target_node_name = Some(new_val.clone());
                        canvas.selected_node = Some(new_val.clone());
                        canvas.selected_nodes.clear();
                        canvas.selected_nodes.insert(new_val.clone());
                        canvas.recalculate_all_links();
                        return Some(format!("Renamed node to '{}'", new_val));
                    }
                }
            }
            DrawerField::Kind => {
                if new_val.is_empty() {
                    return None;
                }
                if let Some(node) = canvas.nodes.get_mut(&node_name) {
                    let matching_prof = profiles.iter().find(|p| p.matches_kind(&new_val));

                    let canonical_kind = if let Some(prof) = matching_prof {
                        let canon = crate::clab::model::canonical_kind(&new_val);
                        if !canon.is_empty() {
                            canon.to_string()
                        } else {
                            prof.kind_name.clone()
                        }
                    } else {
                        let canon = crate::clab::model::canonical_kind(&new_val);
                        if !canon.is_empty() {
                            canon.to_string()
                        } else {
                            new_val.clone()
                        }
                    };

                    node.kind = canonical_kind.clone();

                    if let Some(prof) = matching_prof {
                        node.category = prof.node_category;
                        node.image = prof.default_image.clone();
                        node.interface_pattern = prof.interface_pattern.clone();

                        let def = topo.topology.nodes.entry(node_name.clone()).or_default();
                        def.kind = Some(canonical_kind.clone());
                        def.image = Some(node.image.clone());
                        def.interface_pattern = Some(node.interface_pattern.clone());
                    } else {
                        let category = crate::clab::model::deduce_category(&canonical_kind);
                        node.category = category;
                        let def = topo.topology.nodes.entry(node_name.clone()).or_default();
                        def.kind = Some(canonical_kind.clone());
                    }
                    return Some(format!("Updated kind to '{}'", canonical_kind));
                }
            }
            DrawerField::Image => {
                if let Some(node) = canvas.nodes.get_mut(&node_name) {
                    node.image = new_val.clone();
                    let def = topo.topology.nodes.entry(node_name.clone()).or_default();
                    def.image = Some(new_val.clone());
                    return Some(format!("Updated image to '{}'", new_val));
                }
            }
            DrawerField::MgmtIpv4 => {
                let def = topo.topology.nodes.entry(node_name.clone()).or_default();
                def.mgmt_ipv4 = if new_val.is_empty() {
                    None
                } else {
                    Some(new_val.clone())
                };
                return Some(format!("Updated mgmt IP to '{}'", new_val));
            }
            DrawerField::InterfacePattern => {
                let pattern_val = if new_val.is_empty() {
                    crate::clab::model::default_interface_pattern()
                } else {
                    new_val.clone()
                };
                if let Err(e) = crate::clab::pattern::InterfacePattern::validate(&pattern_val) {
                    return Some(format!("Invalid pattern: {}", e));
                }
                if let Some(node) = canvas.nodes.get_mut(&node_name) {
                    node.interface_pattern = pattern_val.clone();
                }
                let def = topo.topology.nodes.entry(node_name.clone()).or_default();
                def.interface_pattern = Some(pattern_val.clone());
                return Some(format!("Updated interface pattern to '{}'", pattern_val));
            }
            DrawerField::AddPort => {
                if !new_val.is_empty() {
                    if let Some(node) = canvas.nodes.get_mut(&node_name) {
                        let mut ports: Vec<String> =
                            node.ports.iter().map(|p| p.name.clone()).collect();
                        if !ports.contains(&new_val) {
                            ports.push(new_val.clone());
                            node.rebuild_ports(&ports);
                            let def = topo.topology.nodes.entry(node_name.clone()).or_default();
                            def.ports = Some(ports);
                            canvas.recalculate_all_links();
                            return Some(format!("Added port '{}'", new_val));
                        }
                    }
                }
            }
            DrawerField::DeleteNode => {
                canvas.selected_node = Some(node_name.clone());
                canvas.selected_nodes.clear();
                canvas.selected_nodes.insert(node_name.clone());
                canvas.remove_selected_node();
                self.close();
                return Some(format!("Deleted node '{}'", node_name));
            }
        }

        None
    }

    pub fn cycle_kind(&mut self, canvas: &mut CanvasState, topo: &mut LabTopology) {
        let default_profs = NodeProfile::default_profiles();
        self.cycle_kind_with_profiles(canvas, topo, &default_profs);
    }

    pub fn cycle_kind_with_profiles(
        &mut self,
        canvas: &mut CanvasState,
        topo: &mut LabTopology,
        profiles: &[NodeProfile],
    ) {
        let node_name = match &self.target_node_name {
            Some(n) => n.clone(),
            None => return,
        };

        if profiles.is_empty() {
            return;
        }

        if let Some(node) = canvas.nodes.get_mut(&node_name) {
            let curr_kind = &node.kind;
            let next_idx = match profiles
                .iter()
                .position(|t| t.kind_name.eq_ignore_ascii_case(curr_kind))
            {
                Some(idx) => (idx + 1) % profiles.len(),
                None => 0,
            };
            if let Some(next_tmpl) = profiles.get(next_idx) {
                node.kind = next_tmpl.kind_name.clone();
                node.image = next_tmpl.default_image.clone();
                node.category = next_tmpl.node_category;

                let def = topo.topology.nodes.entry(node_name.clone()).or_default();
                def.kind = Some(node.kind.clone());
                def.image = Some(node.image.clone());
            }
        }
    }

    pub fn render(
        &self,
        canvas: &CanvasState,
        topo: &LabTopology,
        area: Rect,
        buf: &mut Buffer,
        theme: &Theme,
    ) {
        let default_profs = NodeProfile::default_profiles();
        self.render_with_profiles(canvas, topo, &default_profs, area, buf, theme);
    }

    pub fn render_with_profiles(
        &self,
        canvas: &CanvasState,
        topo: &LabTopology,
        profiles: &[NodeProfile],
        area: Rect,
        buf: &mut Buffer,
        theme: &Theme,
    ) {
        if !self.is_open || area.width < 10 || area.height < 4 {
            return;
        }

        let drawer_width = 38.min(area.width.saturating_sub(4));
        let drawer_x = area.right().saturating_sub(drawer_width);
        let drawer_rect = Rect::new(drawer_x, area.y, drawer_width, area.height);
        let max_x = drawer_rect.right();
        let max_y = drawer_rect.bottom();

        let border_style = Style::default().fg(theme.accent).bg(theme.panel_bg);

        // Fill drawer area
        for y in drawer_rect.y..drawer_rect.bottom() {
            if y >= buf.area.bottom() {
                break;
            }
            for x in drawer_rect.x..drawer_rect.right() {
                if x >= buf.area.right() {
                    break;
                }
                let ch = if x == drawer_rect.x { '│' } else { ' ' };
                buf[(x, y)].set_char(ch).set_style(border_style);
            }
        }

        let node_name = match &self.target_node_name {
            Some(n) => n,
            None => return,
        };

        let node = match canvas.nodes.get(node_name) {
            Some(n) => n,
            None => return,
        };

        let def = topo.topology.nodes.get(node_name);

        // Title
        let title = format!(" Node Properties: {} ", node_name);
        Self::draw_text(
            drawer_rect.x + 2,
            drawer_rect.y + 1,
            &title,
            Style::default()
                .fg(theme.accent)
                .bg(theme.panel_bg)
                .add_modifier(Modifier::BOLD),
            buf,
            max_x,
            max_y,
        );

        // Divider
        for x in (drawer_rect.x + 1)..drawer_rect.right() {
            if x < buf.area.right() && drawer_rect.y + 2 < buf.area.bottom() {
                buf[(x, drawer_rect.y + 2)]
                    .set_char('─')
                    .set_style(border_style);
            }
        }

        let fields: [(DrawerField, &str, &str); 7] = [
            (
                DrawerField::Name,
                "Node Name",
                if self.is_editing && self.focused_field == DrawerField::Name {
                    self.edit_buffer.as_str()
                } else {
                    node.name.as_str()
                },
            ),
            (
                DrawerField::Kind,
                "Kind (Enter to edit/select)",
                if self.is_editing && self.focused_field == DrawerField::Kind {
                    self.edit_buffer.as_str()
                } else {
                    node.kind.as_str()
                },
            ),
            (
                DrawerField::Image,
                "Docker Image",
                if self.is_editing && self.focused_field == DrawerField::Image {
                    self.edit_buffer.as_str()
                } else {
                    node.image.as_str()
                },
            ),
            (
                DrawerField::MgmtIpv4,
                "Mgmt IPv4",
                if self.is_editing && self.focused_field == DrawerField::MgmtIpv4 {
                    self.edit_buffer.as_str()
                } else {
                    def.and_then(|d| d.mgmt_ipv4.as_deref()).unwrap_or("(auto)")
                },
            ),
            (
                DrawerField::InterfacePattern,
                "Interface Pattern",
                if self.is_editing && self.focused_field == DrawerField::InterfacePattern {
                    self.edit_buffer.as_str()
                } else {
                    node.interface_pattern.as_str()
                },
            ),
            (
                DrawerField::AddPort,
                "Add Port Interface",
                if self.is_editing && self.focused_field == DrawerField::AddPort {
                    self.edit_buffer.as_str()
                } else {
                    "[Enter to type port name]"
                },
            ),
            (
                DrawerField::DeleteNode,
                "Delete Node",
                "[Press Enter to Delete]",
            ),
        ];

        let mut row_y = drawer_rect.y + 4;
        let footer_limit = drawer_rect.bottom().saturating_sub(3);
        let avail_w = max_x.saturating_sub(drawer_rect.x + 4) as usize;

        for (field, label, val) in fields {
            if row_y >= footer_limit {
                break;
            }

            let is_focused = self.focused_field == field;
            let field_style = if is_focused {
                Style::default()
                    .fg(if field == DrawerField::DeleteNode {
                        theme.error
                    } else {
                        theme.accent
                    })
                    .bg(theme.bg)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(theme.text_secondary).bg(theme.panel_bg)
            };

            let prefix = if is_focused { "▶ " } else { "  " };
            Self::draw_text(
                drawer_rect.x + 2,
                row_y,
                &format!("{}{}:", prefix, label),
                field_style,
                buf,
                max_x,
                max_y,
            );

            let val_style = if is_focused && self.is_editing {
                Style::default()
                    .fg(theme.warning)
                    .bg(theme.bg)
                    .add_modifier(Modifier::UNDERLINED | Modifier::BOLD)
            } else if field == DrawerField::DeleteNode {
                Style::default().fg(theme.error).bg(theme.panel_bg)
            } else {
                Style::default().fg(theme.text_primary).bg(theme.panel_bg)
            };

            let display_val = if is_focused && self.is_editing {
                format!("{}_", val)
            } else {
                val.to_string()
            };

            if row_y + 1 < footer_limit {
                let truncated_val: String = if display_val.chars().count() > avail_w && avail_w > 1
                {
                    display_val
                        .chars()
                        .take(avail_w.saturating_sub(1))
                        .collect::<String>()
                        + "…"
                } else {
                    display_val
                };
                Self::draw_text(
                    drawer_rect.x + 4,
                    row_y + 1,
                    &truncated_val,
                    val_style,
                    buf,
                    max_x,
                    max_y,
                );
            }

            row_y += 3;
        }

        // Footer instructions
        let footer_y = drawer_rect.bottom().saturating_sub(3);
        if footer_y > drawer_rect.y + 2 && footer_y + 1 < drawer_rect.bottom() {
            Self::draw_text(
                drawer_rect.x + 2,
                footer_y,
                "↑/↓: Select | Enter: Edit",
                Style::default().fg(theme.text_muted).bg(theme.panel_bg),
                buf,
                max_x,
                max_y,
            );
            Self::draw_text(
                drawer_rect.x + 2,
                footer_y + 1,
                "Space: Cycle Kind | Esc: Close",
                Style::default().fg(theme.text_muted).bg(theme.panel_bg),
                buf,
                max_x,
                max_y,
            );
        }

        // Render Kind Selector Popup overlay if active
        if self.kind_selector_open && self.is_editing && self.focused_field == DrawerField::Kind {
            self.render_kind_selector_popup(profiles, area, drawer_rect, buf, theme);
        }
    }

    pub fn popup_rect(area: Rect, drawer_rect: Rect, profiles_len: usize) -> Option<Rect> {
        if area.width < 20 || area.height < 6 {
            return None;
        }

        let max_w = area.width.saturating_sub(4);
        let popup_w = 70.min(max_w);
        let max_h = area.height.saturating_sub(4).min(18);
        if max_h < 4 {
            return None;
        }
        let min_h = 4.min(max_h);
        let popup_h = (profiles_len as u16 + 6).clamp(min_h, max_h);

        // Position popup to the left of the drawer if space permits, else centered
        let popup_x = if drawer_rect.x >= area.x + popup_w + 2 {
            drawer_rect.x.saturating_sub(popup_w + 1)
        } else {
            area.x + (area.width.saturating_sub(popup_w)) / 2
        };
        let popup_y = (drawer_rect.y + 3).min(area.bottom().saturating_sub(popup_h + 1));
        Some(Rect::new(popup_x, popup_y, popup_w, popup_h))
    }

    fn render_kind_selector_popup(
        &self,
        profiles: &[NodeProfile],
        area: Rect,
        drawer_rect: Rect,
        buf: &mut Buffer,
        theme: &Theme,
    ) {
        let popup_rect = match Self::popup_rect(area, drawer_rect, profiles.len()) {
            Some(r) => r,
            None => return,
        };

        let bg_style = Style::default().fg(theme.text_primary).bg(theme.panel_bg);
        let border_style = Style::default().fg(theme.accent).bg(theme.panel_bg);
        let max_x = popup_rect.right();
        let max_y = popup_rect.bottom();

        // Fill background and draw border
        for y in popup_rect.y..popup_rect.bottom() {
            if y >= buf.area.bottom() {
                break;
            }
            for x in popup_rect.x..popup_rect.right() {
                if x >= buf.area.right() {
                    break;
                }
                let is_top = y == popup_rect.y;
                let is_bottom = y == popup_rect.bottom() - 1;
                let is_left = x == popup_rect.x;
                let is_right = x == popup_rect.right() - 1;

                let ch = match (is_top, is_bottom, is_left, is_right) {
                    (true, _, true, _) => '┌',
                    (true, _, _, true) => '┐',
                    (_, true, true, _) => '└',
                    (_, true, _, true) => '┘',
                    (true, _, _, _) | (_, true, _, _) => '─',
                    (_, _, true, _) | (_, _, _, true) => '│',
                    _ => ' ',
                };
                let style = if is_top || is_bottom || is_left || is_right {
                    border_style
                } else {
                    bg_style
                };
                buf[(x, y)].set_char(ch).set_style(style);
            }
        }

        // Title
        let title = " Select or Type Node Kind ";
        Self::draw_text(
            popup_rect.x + 2,
            popup_rect.y,
            title,
            Style::default()
                .fg(theme.accent)
                .bg(theme.panel_bg)
                .add_modifier(Modifier::BOLD),
            buf,
            max_x,
            max_y,
        );

        // Input line
        let input_label = "Type: ";
        Self::draw_text(
            popup_rect.x + 2,
            popup_rect.y + 1,
            input_label,
            Style::default().fg(theme.text_secondary).bg(theme.panel_bg),
            buf,
            max_x,
            max_y,
        );
        let input_val = format!("{}_", self.edit_buffer);
        Self::draw_text(
            popup_rect.x + 8,
            popup_rect.y + 1,
            &input_val,
            Style::default()
                .fg(theme.warning)
                .bg(theme.bg)
                .add_modifier(Modifier::BOLD | Modifier::UNDERLINED),
            buf,
            max_x,
            max_y,
        );

        // Separator
        for x in (popup_rect.x + 1)..(popup_rect.right() - 1) {
            if x < buf.area.right() && popup_rect.y + 2 < buf.area.bottom() {
                buf[(x, popup_rect.y + 2)]
                    .set_char('─')
                    .set_style(border_style);
            }
        }

        // Available profiles list with scrolling window
        let list_top = popup_rect.y + 3;
        let list_bottom = popup_rect.bottom().saturating_sub(2);
        let max_visible = (list_bottom.saturating_sub(list_top) as usize).max(1);
        let scroll_offset = if self.kind_selector_index >= max_visible {
            self.kind_selector_index - max_visible + 1
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
            let row_y = list_top + display_idx as u16;
            if row_y >= list_bottom {
                break;
            }

            let is_sel = self.kind_selector_index == idx;
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
                "{}{:<22} {:<24} [{}]",
                pointer, prof.kind_name, prof.display_name, prof.node_category
            );
            Self::draw_text(
                popup_rect.x + 2,
                row_y,
                &line_text,
                row_style,
                buf,
                max_x.saturating_sub(1),
                max_y,
            );
        }

        // Footer instructions inside popup
        let footer_y = popup_rect.bottom().saturating_sub(1);
        let footer_text = "↑/↓: Pick | Enter: Apply | Esc: Cancel";
        Self::draw_text(
            popup_rect.x + 2,
            footer_y,
            footer_text,
            Style::default().fg(theme.text_muted).bg(theme.panel_bg),
            buf,
            max_x.saturating_sub(1),
            max_y,
        );
    }

    fn draw_text(
        x: u16,
        y: u16,
        text: &str,
        style: Style,
        buf: &mut Buffer,
        max_x: u16,
        max_y: u16,
    ) {
        if y >= max_y || y >= buf.area.bottom() {
            return;
        }
        for (i, ch) in text.chars().enumerate() {
            let px = x + i as u16;
            if px < max_x && px < buf.area.right() {
                buf[(px, y)].set_char(ch).set_style(style);
            }
        }
    }
}
