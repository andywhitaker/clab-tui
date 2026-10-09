use crate::canvas::link::{CanvasLink, OrthogonalRouter, Rect, RoutingStyle};
use crate::canvas::node::CanvasNode;
use crate::clab::model::{
    LabTopology, LinkDefinition, NodeDefinition, NodeProfile, KIND_TEMPLATES,
};
use std::collections::{BTreeMap, HashSet};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CanvasMode {
    Normal,
    Wiring,
    DraggingNode,
    PanningCanvas,
    InspectorDrawer,
    BoxSelection,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BadgePos {
    pub x: i32,
    pub y: i32,
    pub len: usize,
}

#[derive(Debug, Clone)]
pub struct CanvasState {
    pub nodes: BTreeMap<String, CanvasNode>,
    pub links: Vec<CanvasLink>,
    pub routing_style: RoutingStyle,
    pub offset_x: f64,
    pub offset_y: f64,
    pub zoom: f64,
    pub grid_snap: f64,

    // Selection
    pub selected_node: Option<String>,
    pub selected_nodes: HashSet<String>,
    pub selected_link: Option<usize>,
    pub selection_box: Option<((f64, f64), (f64, f64))>,
    pub hover_node: Option<String>,
    pub hover_port: Option<String>,
    pub cursor_pos: (f64, f64),

    // Wiring State
    pub mode: CanvasMode,
    pub wiring_source: Option<(String, String)>,
    pub wiring_cursor: (f64, f64),

    // Dragging
    pub drag_start_mouse: (f64, f64),
    pub drag_start_node_pos: (f64, f64),
    pub drag_start_offset: (f64, f64),
}

impl Default for CanvasState {
    fn default() -> Self {
        Self {
            nodes: BTreeMap::new(),
            links: Vec::new(),
            routing_style: RoutingStyle::Orthogonal,
            offset_x: 0.0,
            offset_y: 0.0,
            zoom: 1.0,
            grid_snap: 2.0,

            selected_node: None,
            selected_nodes: HashSet::new(),
            selected_link: None,
            selection_box: None,
            hover_node: None,
            hover_port: None,
            cursor_pos: (20.0, 10.0),

            mode: CanvasMode::Normal,
            wiring_source: None,
            wiring_cursor: (0.0, 0.0),

            drag_start_mouse: (0.0, 0.0),
            drag_start_node_pos: (0.0, 0.0),
            drag_start_offset: (0.0, 0.0),
        }
    }
}

impl CanvasState {
    pub fn new() -> Self {
        Self::default()
    }

    /// Load state from LabTopology
    pub fn load_from_topology(&mut self, topo: &LabTopology) {
        self.nodes.clear();
        self.links.clear();

        // Place nodes
        let mut auto_x = 10.0;
        let mut auto_y = 6.0;

        for (name, def) in &topo.topology.nodes {
            let x = def.canvas_x().unwrap_or(auto_x);
            let y = def.canvas_y().unwrap_or(auto_y);
            let node = CanvasNode::new(name, def, x, y);
            self.nodes.insert(name.clone(), node);

            if def.canvas_x().is_none() {
                auto_x += 30.0;
                if auto_x > 90.0 {
                    auto_x = 10.0;
                    auto_y += 12.0;
                }
            }
        }

        // Add links
        for link_def in &topo.topology.links {
            if let (Some((src_node, src_port)), Some((tgt_node, tgt_port))) =
                (link_def.source(), link_def.target())
            {
                self.add_link_internal(src_node, src_port, tgt_node, tgt_port);
            }
        }

        self.recalculate_all_links();

        // Default selection
        if self.selected_node.is_none() {
            self.selected_node = self.nodes.keys().next().cloned();
        }
        self.selected_nodes.clear();
        if let Some(ref sel) = self.selected_node {
            self.selected_nodes.insert(sel.clone());
        }
    }

    /// Save canvas coordinates and structure back into LabTopology
    pub fn sync_to_topology(&self, topo: &mut LabTopology) {
        // Sync node positions and definitions
        for (name, node) in &self.nodes {
            let def = topo
                .topology
                .nodes
                .entry(name.clone())
                .or_insert_with(|| NodeDefinition {
                    kind: Some(node.kind.clone()),
                    image: Some(node.image.clone()),
                    interface_pattern: Some(node.interface_pattern.clone()),
                    ..Default::default()
                });
            def.set_canvas_pos(node.x, node.y);
            def.kind = Some(node.kind.clone());
            def.image = Some(node.image.clone());
            def.interface_pattern = Some(node.interface_pattern.clone());
        }

        // Remove deleted nodes from topology
        topo.topology
            .nodes
            .retain(|k, _| self.nodes.contains_key(k));

        // Sync links
        topo.topology.links = self
            .links
            .iter()
            .map(|l| {
                LinkDefinition::new(
                    &l.source_node,
                    &l.source_port,
                    &l.target_node,
                    &l.target_port,
                )
            })
            .collect();
    }

    fn add_link_internal(
        &mut self,
        src_node: String,
        src_port: String,
        tgt_node: String,
        tgt_port: String,
    ) {
        // Check for duplicate link
        let exists = self.links.iter().any(|l| {
            (l.source_node == src_node
                && l.source_port == src_port
                && l.target_node == tgt_node
                && l.target_port == tgt_port)
                || (l.source_node == tgt_node
                    && l.source_port == tgt_port
                    && l.target_node == src_node
                    && l.target_port == src_port)
        });

        if !exists {
            self.links.push(CanvasLink {
                source_node: src_node,
                source_port: src_port,
                target_node: tgt_node,
                target_port: tgt_port,
                waypoints: Vec::new(),
                routing_style: self.routing_style,
            });
        }
    }

    /// Add a new link between two ports and recalculate route
    pub fn add_link(
        &mut self,
        src_node: &str,
        src_port: &str,
        tgt_node: &str,
        tgt_port: &str,
    ) -> bool {
        if src_node == tgt_node {
            return false;
        }

        if !self.nodes.contains_key(src_node) || !self.nodes.contains_key(tgt_node) {
            return false;
        }

        self.add_link_internal(
            src_node.to_string(),
            src_port.to_string(),
            tgt_node.to_string(),
            tgt_port.to_string(),
        );

        self.recalculate_all_links();
        true
    }

    /// Allocate next available interface name for a node based on its pattern and used links
    pub fn allocate_next_interface(&self, node_name: &str) -> String {
        let mut used_ports = Vec::new();
        for link in &self.links {
            if link.source_node == node_name {
                used_ports.push(link.source_port.as_str());
            }
            if link.target_node == node_name {
                used_ports.push(link.target_port.as_str());
            }
        }

        let pattern_str = if let Some(node) = self.nodes.get(node_name) {
            if !node.interface_pattern.is_empty() {
                node.interface_pattern.clone()
            } else {
                crate::clab::model::NodeProfile::default_pattern_for_kind(&node.kind)
            }
        } else {
            "eth{n:1}".to_string()
        };

        let pattern = crate::clab::pattern::InterfacePattern::parse(&pattern_str);
        pattern.allocate_next(&used_ports)
    }

    /// Update ports on an existing link by index with full validation
    pub fn update_link_ports(
        &mut self,
        link_idx: usize,
        src_port: &str,
        tgt_port: &str,
    ) -> Result<(), String> {
        if link_idx >= self.links.len() {
            return Err("Link index out of bounds".to_string());
        }

        let s_port = src_port.trim();
        let t_port = tgt_port.trim();

        if s_port.is_empty() || t_port.is_empty() {
            return Err("Interface names cannot be empty".to_string());
        }

        if s_port.starts_with('-') || t_port.starts_with('-') {
            return Err("Interface name cannot start with a hyphen '-'".to_string());
        }

        // Validate interface characters
        let is_valid_char = |c: char| {
            c.is_ascii_alphanumeric() || c == '/' || c == '-' || c == '_' || c == '.' || c == ':'
        };
        if !s_port.chars().all(is_valid_char) || !t_port.chars().all(is_valid_char) {
            return Err("Interface names contain invalid characters".to_string());
        }

        let src_node = self.links[link_idx].source_node.clone();
        let tgt_node = self.links[link_idx].target_node.clone();

        if src_node == tgt_node && s_port == t_port {
            return Err("Cannot connect an interface to itself on the same node".to_string());
        }

        // Check for duplicate link collision and interface collisions across other links
        for (i, other) in self.links.iter().enumerate() {
            if i == link_idx {
                continue;
            }

            if (other.source_node == src_node
                && other.source_port == s_port
                && other.target_node == tgt_node
                && other.target_port == t_port)
                || (other.source_node == tgt_node
                    && other.source_port == t_port
                    && other.target_node == src_node
                    && other.target_port == s_port)
            {
                return Err(
                    "Duplicate link: a link with these endpoints already exists".to_string()
                );
            }

            if (other.source_node == src_node && other.source_port == s_port)
                || (other.target_node == src_node && other.target_port == s_port)
            {
                return Err(format!(
                    "Interface '{}' is already in use on node '{}'",
                    s_port, src_node
                ));
            }

            if (other.source_node == tgt_node && other.source_port == t_port)
                || (other.target_node == tgt_node && other.target_port == t_port)
            {
                return Err(format!(
                    "Interface '{}' is already in use on node '{}'",
                    t_port, tgt_node
                ));
            }
        }

        self.links[link_idx].source_port = s_port.to_string();
        self.links[link_idx].target_port = t_port.to_string();
        self.recalculate_all_links();
        Ok(())
    }

    /// Remove selected link
    pub fn remove_selected_link(&mut self) -> bool {
        if let Some(idx) = self.selected_link {
            if idx < self.links.len() {
                self.links.remove(idx);
                self.selected_link = None;
                return true;
            }
        }
        false
    }

    /// Remove all currently selected nodes and any associated links
    pub fn remove_selected_nodes(&mut self) -> Vec<String> {
        let set = if !self.selected_nodes.is_empty() {
            std::mem::take(&mut self.selected_nodes)
        } else if let Some(node_name) = self.selected_node.take() {
            let mut s = HashSet::new();
            s.insert(node_name);
            s
        } else {
            HashSet::new()
        };
        if set.is_empty() {
            return Vec::new();
        }

        let mut to_remove: Vec<String> = set.into_iter().collect();
        to_remove.sort();

        for node_name in &to_remove {
            self.nodes.remove(node_name);
        }
        self.links
            .retain(|l| !to_remove.contains(&l.source_node) && !to_remove.contains(&l.target_node));
        self.selected_nodes.clear();
        self.selected_node = self.nodes.keys().next().cloned();
        if let Some(ref sel) = self.selected_node {
            self.selected_nodes.insert(sel.clone());
        }
        self.recalculate_all_links();
        to_remove
    }

    /// Remove selected node and any associated links
    pub fn remove_selected_node(&mut self) -> Option<String> {
        if self.selected_nodes.len() > 1 {
            let removed = self.remove_selected_nodes();
            return removed.into_iter().next();
        }
        if let Some(node_name) = self.selected_node.take() {
            self.selected_nodes.remove(&node_name);
            self.nodes.remove(&node_name);
            self.links
                .retain(|l| l.source_node != node_name && l.target_node != node_name);
            self.selected_node = self.nodes.keys().next().cloned();
            if let Some(ref sel) = self.selected_node {
                self.selected_nodes.insert(sel.clone());
            }
            self.recalculate_all_links();
            Some(node_name)
        } else {
            let removed = self.remove_selected_nodes();
            removed.into_iter().next()
        }
    }

    /// Add a new node of given kind template at canvas position
    pub fn add_node_from_template(&mut self, kind_name: &str, x: f64, y: f64) -> String {
        let tmpl = KIND_TEMPLATES
            .iter()
            .find(|t| t.matches_kind(kind_name))
            .copied()
            .unwrap_or(KIND_TEMPLATES[0]);

        // Generate unique name
        let mut idx = 1;
        let mut name = format!("{}{}", tmpl.kind_name, idx);
        while self.nodes.contains_key(&name) {
            idx += 1;
            name = format!("{}{}", tmpl.kind_name, idx);
        }

        let mut def = NodeDefinition {
            kind: Some(tmpl.kind_name.to_string()),
            image: Some(tmpl.default_image.to_string()),
            ports: Some(tmpl.default_ports.iter().map(|s| s.to_string()).collect()),
            interface_pattern: Some(tmpl.interface_pattern.to_string()),
            ..Default::default()
        };
        def.set_canvas_pos(x, y);

        let node = CanvasNode::new(&name, &def, x, y);
        self.nodes.insert(name.clone(), node);
        self.selected_node = Some(name.clone());
        self.selected_nodes.clear();
        self.selected_nodes.insert(name.clone());
        self.recalculate_all_links();
        name
    }

    /// Add a new node of given dynamic NodeProfile at canvas position
    pub fn add_node_from_profile(&mut self, profile: &NodeProfile, x: f64, y: f64) -> String {
        // Generate unique name
        let mut idx = 1;
        let mut name = format!("{}{}", profile.kind_name, idx);
        while self.nodes.contains_key(&name) {
            idx += 1;
            name = format!("{}{}", profile.kind_name, idx);
        }

        let mut def = NodeDefinition {
            kind: Some(profile.kind_name.clone()),
            image: Some(profile.default_image.clone()),
            ports: Some(profile.default_ports.clone()),
            interface_pattern: Some(profile.interface_pattern.clone()),
            ..Default::default()
        };
        def.set_canvas_pos(x, y);

        let node = CanvasNode::from_profile(&name, profile, x, y);
        self.nodes.insert(name.clone(), node);
        self.selected_node = Some(name.clone());
        self.selected_nodes.clear();
        self.selected_nodes.insert(name.clone());
        self.recalculate_all_links();
        name
    }

    /// Clone selected node
    pub fn clone_selected_node(&mut self) -> Option<String> {
        let selected_name = self.selected_node.clone()?;
        let orig = self.nodes.get(&selected_name)?;

        let new_x = orig.x + 6.0;
        let new_y = orig.y + 6.0;
        let kind = orig.kind.clone();
        let image = orig.image.clone();

        let mut idx = 1;
        let mut new_name = format!("{}_copy{}", selected_name, idx);
        while self.nodes.contains_key(&new_name) {
            idx += 1;
            new_name = format!("{}_copy{}", selected_name, idx);
        }

        let def = NodeDefinition {
            kind: Some(kind),
            image: Some(image),
            ..Default::default()
        };

        let node = CanvasNode::new(&new_name, &def, new_x, new_y);
        self.nodes.insert(new_name.clone(), node);
        self.selected_node = Some(new_name.clone());
        self.selected_nodes.clear();
        self.selected_nodes.insert(new_name.clone());
        self.recalculate_all_links();
        Some(new_name)
    }

    /// Move selected node(s) by delta
    pub fn move_selected_node(&mut self, dx: f64, dy: f64) {
        if self.selected_nodes.len() > 1 {
            for name in &self.selected_nodes {
                if let Some(node) = self.nodes.get_mut(name) {
                    let new_x = ((node.x + dx) / self.grid_snap).round() * self.grid_snap;
                    let new_y = ((node.y + dy) / self.grid_snap).round() * self.grid_snap;
                    node.set_pos(new_x.max(0.0), new_y.max(0.0));
                }
            }
            self.recalculate_all_links();
            return;
        }
        if let Some(ref name) = self.selected_node {
            if let Some(node) = self.nodes.get_mut(name) {
                let new_x = ((node.x + dx) / self.grid_snap).round() * self.grid_snap;
                let new_y = ((node.y + dy) / self.grid_snap).round() * self.grid_snap;
                node.set_pos(new_x.max(0.0), new_y.max(0.0));
            }
            self.recalculate_all_links();
        }
    }

    /// Recalculate paths for all links, separating parallel links between the same nodes
    pub fn recalculate_all_links(&mut self) {
        // Group link indices by canonical unordered node pair
        let mut link_groups: std::collections::HashMap<(String, String), Vec<usize>> =
            std::collections::HashMap::new();

        for (i, link) in self.links.iter().enumerate() {
            let pair_key = if link.source_node <= link.target_node {
                (link.source_node.clone(), link.target_node.clone())
            } else {
                (link.target_node.clone(), link.source_node.clone())
            };
            link_groups.entry(pair_key).or_default().push(i);
        }

        // For each node pair, calculate distinct perimeter attachment points and paths
        for ((node_a_name, node_b_name), group_indices) in link_groups {
            let k_total = group_indices.len();
            let (node_a_opt, node_b_opt) = (
                self.nodes.get(&node_a_name).cloned(),
                self.nodes.get(&node_b_name).cloned(),
            );

            if let (Some(node_a), Some(node_b)) = (node_a_opt, node_b_opt) {
                let a_cx = node_a.x + node_a.width / 2.0;
                let a_cy = node_a.y + node_a.height / 2.0;
                let b_cx = node_b.x + node_b.width / 2.0;
                let b_cy = node_b.y + node_b.height / 2.0;

                // Base connection directions facing each other
                let ((_, _), dir_a) = node_a.perimeter_connection_towards(b_cx, b_cy);
                let ((_, _), dir_b) = node_b.perimeter_connection_towards(a_cx, a_cy);

                for (rank, &link_idx) in group_indices.iter().enumerate() {
                    let link = &self.links[link_idx];

                    // Centered offset across parallel links: e.g. -0.5 and +0.5 for k_total=2
                    let center_offset = if k_total <= 1 {
                        0.0
                    } else {
                        rank as f64 - (k_total as f64 - 1.0) / 2.0
                    };

                    // Compute perimeter offset for node A
                    let offset_a = match dir_a {
                        crate::canvas::node::Direction::East
                        | crate::canvas::node::Direction::West => {
                            let v_step = if k_total == 2 {
                                2.0
                            } else {
                                (3.2 / (k_total as f64 - 1.0)).clamp(0.4, 2.0)
                            };
                            center_offset * v_step
                        }
                        crate::canvas::node::Direction::North
                        | crate::canvas::node::Direction::South => {
                            let h_step = if k_total == 2 {
                                6.0
                            } else {
                                (14.0 / (k_total as f64 - 1.0)).clamp(1.5, 6.0)
                            };
                            center_offset * h_step
                        }
                    };

                    // Compute perimeter offset for node B
                    let offset_b = match dir_b {
                        crate::canvas::node::Direction::East
                        | crate::canvas::node::Direction::West => {
                            let v_step = if k_total == 2 {
                                2.0
                            } else {
                                (3.2 / (k_total as f64 - 1.0)).clamp(0.4, 2.0)
                            };
                            center_offset * v_step
                        }
                        crate::canvas::node::Direction::North
                        | crate::canvas::node::Direction::South => {
                            let h_step = if k_total == 2 {
                                6.0
                            } else {
                                (14.0 / (k_total as f64 - 1.0)).clamp(1.5, 6.0)
                            };
                            center_offset * h_step
                        }
                    };

                    let (pt_a, _) =
                        node_a.perimeter_connection_towards_with_offset(b_cx, b_cy, offset_a);
                    let (pt_b, _) =
                        node_b.perimeter_connection_towards_with_offset(a_cx, a_cy, offset_b);

                    let (start_pt, start_dir, end_pt, end_dir) = if link.source_node == node_a_name
                    {
                        (pt_a, dir_a, pt_b, dir_b)
                    } else {
                        (pt_b, dir_b, pt_a, dir_a)
                    };

                    // Compute parallel stub length to offset intermediate corner segments
                    let stub_length = if k_total <= 1 {
                        OrthogonalRouter::STUB_LENGTH
                    } else {
                        OrthogonalRouter::STUB_LENGTH + (rank as f64) * 1.5
                    };

                    // Filter obstacles to exclude source and target node
                    let link_obstacles: Vec<Rect> = self
                        .nodes
                        .iter()
                        .filter(|(name, _)| {
                            *name != &link.source_node && *name != &link.target_node
                        })
                        .map(|(_, n)| Rect::new(n.x, n.y, n.x + n.width, n.y + n.height))
                        .collect();

                    self.links[link_idx].waypoints = OrthogonalRouter::route_with_style_and_stub(
                        start_pt,
                        start_dir,
                        end_pt,
                        end_dir,
                        &link_obstacles,
                        link.routing_style,
                        stub_length,
                    );
                }
            }
        }
    }

    /// Set routing style for canvas and update all existing links
    pub fn set_routing_style(&mut self, style: RoutingStyle) {
        self.routing_style = style;
        for link in &mut self.links {
            link.routing_style = style;
        }
        self.recalculate_all_links();
    }

    /// Cycle routing style (Orthogonal -> Direct -> Octilinear)
    pub fn cycle_routing_style(&mut self) -> RoutingStyle {
        let next_style = self.routing_style.next();
        self.set_routing_style(next_style);
        next_style
    }

    /// Start interactive wiring from selected node or hovered node/port
    pub fn start_wiring(&mut self) -> bool {
        let (node_name, port_name) =
            if let (Some(n), Some(p)) = (&self.hover_node, &self.hover_port) {
                (n.clone(), p.clone())
            } else if let Some(n) = &self.hover_node {
                (n.clone(), String::new())
            } else if let Some(n) = &self.selected_node {
                (n.clone(), String::new())
            } else {
                return false;
            };

        if let Some(node) = self.nodes.get(&node_name) {
            let (cursor_x, cursor_y) = if let Some(port) = node.get_port(&port_name) {
                (port.x, port.y)
            } else {
                (node.x + node.width / 2.0, node.y + node.height / 2.0)
            };
            self.mode = CanvasMode::Wiring;
            self.wiring_source = Some((node_name, port_name));
            self.wiring_cursor = (cursor_x, cursor_y);
            return true;
        }
        false
    }

    /// Complete interactive wiring to target node, auto-assigning interfaces if not explicitly provided
    pub fn complete_wiring(&mut self, target_node: &str, target_port: &str) -> bool {
        if let Some((src_node, mut src_port)) = self.wiring_source.take() {
            if src_node == target_node {
                self.mode = CanvasMode::Normal;
                return false;
            }

            if src_port.is_empty() {
                src_port = self.allocate_next_interface(&src_node);
            }

            let tgt_port = if target_port.is_empty() {
                self.allocate_next_interface(target_node)
            } else {
                target_port.to_string()
            };

            let res = self.add_link(&src_node, &src_port, target_node, &tgt_port);
            self.mode = CanvasMode::Normal;
            return res;
        }
        self.mode = CanvasMode::Normal;
        false
    }

    /// Cancel wiring mode
    pub fn cancel_wiring(&mut self) {
        self.mode = CanvasMode::Normal;
        self.wiring_source = None;
    }

    /// Pan canvas
    pub fn pan(&mut self, dx: f64, dy: f64) {
        self.offset_x += dx;
        self.offset_y += dy;
    }

    /// Zoom canvas
    pub fn zoom_in(&mut self) {
        self.zoom = (self.zoom * 1.2).min(2.5);
    }

    pub fn zoom_out(&mut self) {
        self.zoom = (self.zoom / 1.2).max(0.5);
    }

    pub fn reset_view(&mut self) {
        self.offset_x = 0.0;
        self.offset_y = 0.0;
        self.zoom = 1.0;
    }

    /// Handle mouse click on canvas coordinates
    pub fn handle_click(&mut self, canvas_x: f64, canvas_y: f64) {
        if self.mode == CanvasMode::Wiring {
            // Find target node: check port near first, or check entire node bounding box
            for (name, node) in &self.nodes {
                if let Some(port) = node.find_port_near(canvas_x, canvas_y, 1.5) {
                    let port_name = port.name.clone();
                    let node_name = name.clone();
                    self.complete_wiring(&node_name, &port_name);
                    return;
                } else if node.contains_point(canvas_x, canvas_y) {
                    let node_name = name.clone();
                    self.complete_wiring(&node_name, "");
                    return;
                }
            }
            // If clicked on empty space, cancel wiring
            self.cancel_wiring();
            return;
        }

        // Check if port anchor glyph clicked to start wiring (precise threshold to avoid hijacking node drag)
        for (name, node) in &self.nodes {
            if let Some(port) = node.find_port_near(canvas_x, canvas_y, 1.0) {
                let node_name = name.clone();
                let port_name = port.name.clone();
                self.selected_node = Some(node_name.clone());
                self.selected_nodes.clear();
                self.selected_nodes.insert(node_name.clone());
                self.selected_link = None;
                self.mode = CanvasMode::Wiring;
                self.wiring_source = Some((node_name, port_name));
                self.wiring_cursor = (port.x, port.y);
                self.drag_start_mouse = (canvas_x, canvas_y);
                return;
            }
        }

        // Normal mode hit test
        for (name, node) in &self.nodes {
            if node.contains_point(canvas_x, canvas_y) {
                self.selected_node = Some(name.clone());
                self.selected_nodes.clear();
                self.selected_nodes.insert(name.clone());
                self.selected_link = None;
                self.mode = CanvasMode::DraggingNode;
                self.drag_start_mouse = (canvas_x, canvas_y);
                self.drag_start_node_pos = (node.x, node.y);
                return;
            }
        }

        // Check link badge hit test
        let (screen_x, screen_y) = self.canvas_to_screen(canvas_x, canvas_y);
        for i in 0..self.links.len() {
            if let Some((src_badge, tgt_badge)) = self.link_badge_positions(i) {
                if (screen_y == src_badge.y
                    && screen_x >= src_badge.x
                    && screen_x < src_badge.x + src_badge.len as i32)
                    || (screen_y == tgt_badge.y
                        && screen_x >= tgt_badge.x
                        && screen_x < tgt_badge.x + tgt_badge.len as i32)
                {
                    self.selected_link = Some(i);
                    self.selected_node = None;
                    self.selected_nodes.clear();
                    self.mode = CanvasMode::Normal;
                    return;
                }
            }
        }

        // Check link wire hit test (select closest link within 1.5 units)
        let mut best_link = None;
        let mut min_dist = 1.5;
        for (i, link) in self.links.iter().enumerate() {
            for win in link.waypoints.windows(2) {
                let dist = point_to_segment_distance(canvas_x, canvas_y, win[0], win[1]);
                if dist < min_dist {
                    min_dist = dist;
                    best_link = Some(i);
                }
            }
        }
        if let Some(i) = best_link {
            self.selected_link = Some(i);
            self.selected_node = None;
            self.selected_nodes.clear();
            self.mode = CanvasMode::Normal;
            return;
        }

        // Empty canvas clicked: prepare for drag panning
        self.selected_link = None;
        self.selected_node = None;
        self.selected_nodes.clear();
        self.mode = CanvasMode::PanningCanvas;
        self.drag_start_mouse = (canvas_x, canvas_y);
        self.drag_start_offset = (self.offset_x, self.offset_y);
    }

    /// Calculate shortest distance from a 2D point to a line segment
    pub fn point_to_segment_distance(px: f64, py: f64, p1: (f64, f64), p2: (f64, f64)) -> f64 {
        point_to_segment_distance(px, py, p1, p2)
    }

    /// Convert screen coordinate to canvas coordinate
    pub fn screen_to_canvas(&self, screen_x: u16, screen_y: u16) -> (f64, f64) {
        let cx = (screen_x as f64 - self.offset_x) / self.zoom;
        let cy = (screen_y as f64 - self.offset_y) / self.zoom;
        (cx, cy)
    }

    /// Convert canvas coordinate to screen coordinate
    pub fn canvas_to_screen(&self, canvas_x: f64, canvas_y: f64) -> (i32, i32) {
        let sx = (canvas_x * self.zoom + self.offset_x).round() as i32;
        let sy = (canvas_y * self.zoom + self.offset_y).round() as i32;
        (sx, sy)
    }

    /// Calculate screen positions for source and target interface pill badges for a link.
    /// Returns `Some(((source_badge_x, source_badge_y, badge_len), (target_badge_x, target_badge_y, badge_len)))`.
    pub fn link_badge_positions(&self, link_idx: usize) -> Option<(BadgePos, BadgePos)> {
        if link_idx >= self.links.len() {
            return None;
        }
        let link = &self.links[link_idx];
        let n = link.waypoints.len();
        if n < 2 {
            return None;
        }

        let pair = if link.source_node <= link.target_node {
            (&link.source_node, &link.target_node)
        } else {
            (&link.target_node, &link.source_node)
        };
        let parallel: Vec<usize> = self
            .links
            .iter()
            .enumerate()
            .filter(|(_, l)| {
                let p = if l.source_node <= l.target_node {
                    (&l.source_node, &l.target_node)
                } else {
                    (&l.target_node, &l.source_node)
                };
                p == pair
            })
            .map(|(idx, _)| idx)
            .collect();
        let total = parallel.len();
        let rank = parallel
            .iter()
            .position(|&idx| idx == link_idx)
            .unwrap_or(0);

        // Source interface pill badge
        let (sx0, sy0) = self.canvas_to_screen(link.waypoints[0].0, link.waypoints[0].1);
        let (sx1, sy1) = self.canvas_to_screen(link.waypoints[1].0, link.waypoints[1].1);
        let badge_src = format!("[{}]", link.source_port);
        let b_len = badge_src.len();
        let (bx, by) = if (sy0 - sy1).abs() == 0 {
            let bx_stagger = if total > 2 { (rank as i32) * 2 } else { 0 };
            if sx1 > sx0 {
                (sx0 + 1 + bx_stagger, sy0)
            } else {
                (sx0 - b_len as i32 - bx_stagger, sy0)
            }
        } else if (sx0 - sx1).abs() == 0 {
            let by_stagger = if total > 2 { (rank / 2) as i32 } else { 0 };
            let by = if sy1 > sy0 {
                sy0 + 1 + by_stagger
            } else {
                sy0 - 1 - by_stagger
            };
            let bx = if total > 1 && rank % 2 == 0 {
                sx0 - b_len as i32
            } else {
                sx0 + 1
            };
            (bx, by)
        } else {
            let stagger = if total > 2 { (rank as i32) * 2 } else { 0 };
            let bx = if sx1 > sx0 {
                sx0 + 1 + stagger
            } else {
                sx0 - b_len as i32 - stagger
            };
            let by = if sy1 > sy0 { sy0 + 1 } else { sy0 - 1 };
            (bx, by)
        };

        // Target interface pill badge
        let (tx0, ty0) = self.canvas_to_screen(link.waypoints[n - 1].0, link.waypoints[n - 1].1);
        let (tx1, ty1) = self.canvas_to_screen(link.waypoints[n - 2].0, link.waypoints[n - 2].1);
        let badge_tgt = format!("[{}]", link.target_port);
        let tb_len = badge_tgt.len();
        let (tbx, tby) = if (ty0 - ty1).abs() == 0 {
            let tbx_stagger = if total > 2 { (rank as i32) * 2 } else { 0 };
            if tx1 > tx0 {
                (tx0 + 1 + tbx_stagger, ty0)
            } else {
                (tx0 - tb_len as i32 - tbx_stagger, ty0)
            }
        } else if (tx0 - tx1).abs() == 0 {
            let tby_stagger = if total > 2 { (rank / 2) as i32 } else { 0 };
            let tby = if ty1 > ty0 {
                ty0 + 1 + tby_stagger
            } else {
                ty0 - 1 - tby_stagger
            };
            let tbx = if total > 1 && rank % 2 == 0 {
                tx0 - tb_len as i32
            } else {
                tx0 + 1
            };
            (tbx, tby)
        } else {
            let stagger = if total > 2 { (rank as i32) * 2 } else { 0 };
            let tbx = if tx1 > tx0 {
                tx0 + 1 + stagger
            } else {
                tx0 - tb_len as i32 - stagger
            };
            let tby = if ty1 > ty0 { ty0 + 1 } else { ty0 - 1 };
            (tbx, tby)
        };

        Some((
            BadgePos {
                x: bx,
                y: by,
                len: b_len,
            },
            BadgePos {
                x: tbx,
                y: tby,
                len: tb_len,
            },
        ))
    }

    /// Select next node
    pub fn select_next_node(&mut self) {
        let keys: Vec<String> = self.nodes.keys().cloned().collect();
        if keys.is_empty() {
            return;
        }

        if let Some(curr) = &self.selected_node {
            if let Some(pos) = keys.iter().position(|k| k == curr) {
                let next_pos = (pos + 1) % keys.len();
                let next_key = keys[next_pos].clone();
                self.selected_node = Some(next_key.clone());
                self.selected_nodes.clear();
                self.selected_nodes.insert(next_key);
                return;
            }
        }
        let first_key = keys[0].clone();
        self.selected_node = Some(first_key.clone());
        self.selected_nodes.clear();
        self.selected_nodes.insert(first_key);
    }

    /// Select previous node
    pub fn select_prev_node(&mut self) {
        let keys: Vec<String> = self.nodes.keys().cloned().collect();
        if keys.is_empty() {
            return;
        }

        if let Some(curr) = &self.selected_node {
            if let Some(pos) = keys.iter().position(|k| k == curr) {
                let prev_pos = if pos == 0 { keys.len() - 1 } else { pos - 1 };
                let prev_key = keys[prev_pos].clone();
                self.selected_node = Some(prev_key.clone());
                self.selected_nodes.clear();
                self.selected_nodes.insert(prev_key);
                return;
            }
        }
        let last_key = keys[keys.len() - 1].clone();
        self.selected_node = Some(last_key.clone());
        self.selected_nodes.clear();
        self.selected_nodes.insert(last_key);
    }

    /// Select next link
    pub fn select_next_link(&mut self) {
        if self.links.is_empty() {
            self.selected_link = None;
            return;
        }
        self.selected_node = None;
        let next_idx = match self.selected_link {
            Some(i) => (i + 1) % self.links.len(),
            None => 0,
        };
        self.selected_link = Some(next_idx);
    }

    /// Select previous link
    pub fn select_prev_link(&mut self) {
        if self.links.is_empty() {
            self.selected_link = None;
            return;
        }
        self.selected_node = None;
        let prev_idx = match self.selected_link {
            Some(i) => {
                if i == 0 {
                    self.links.len() - 1
                } else {
                    i - 1
                }
            }
            None => self.links.len() - 1,
        };
        self.selected_link = Some(prev_idx);
    }
}

/// Calculate shortest distance from a 2D point to a line segment
pub fn point_to_segment_distance(px: f64, py: f64, p1: (f64, f64), p2: (f64, f64)) -> f64 {
    let dx = p2.0 - p1.0;
    let dy = p2.1 - p1.1;
    let len_sq = dx * dx + dy * dy;
    if len_sq < 1e-6 {
        return ((px - p1.0).powi(2) + (py - p1.1).powi(2)).sqrt();
    }
    let t = (((px - p1.0) * dx + (py - p1.1) * dy) / len_sq).clamp(0.0, 1.0);
    let proj_x = p1.0 + t * dx;
    let proj_y = p1.1 + t * dy;
    ((px - proj_x).powi(2) + (py - proj_y).powi(2)).sqrt()
}
