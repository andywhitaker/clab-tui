use crate::clab::model::{NodeCategory, NodeDefinition, NodeProfile, KIND_TEMPLATES};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    North,
    South,
    East,
    West,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PortAnchor {
    pub name: String,
    pub x: f64,
    pub y: f64,
    pub direction: Direction,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CanvasNode {
    pub name: String,
    pub kind: String,
    pub image: String,
    pub category: NodeCategory,
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
    pub ports: Vec<PortAnchor>,
    pub interface_pattern: String,
}

impl CanvasNode {
    pub const DEFAULT_WIDTH: f64 = 18.0;
    pub const DEFAULT_HEIGHT: f64 = 5.0;

    pub fn new(name: &str, def: &NodeDefinition, x: f64, y: f64) -> Self {
        let kind = def.kind.clone().unwrap_or_else(|| "linux".to_string());
        let template = KIND_TEMPLATES.iter().find(|t| t.matches_kind(&kind));

        let category = template
            .map(|t| t.node_category)
            .unwrap_or_else(|| crate::clab::model::deduce_category(&kind));
        let image = def
            .image
            .clone()
            .or_else(|| template.map(|t| t.default_image.to_string()))
            .unwrap_or_else(|| "alpine:latest".to_string());

        let interface_pattern = def
            .interface_pattern
            .clone()
            .or_else(|| template.map(|t| t.interface_pattern.to_string()))
            .unwrap_or_else(|| NodeProfile::default_pattern_for_kind(&kind));

        let mut node = Self {
            name: name.to_string(),
            kind,
            image,
            category,
            x,
            y,
            width: Self::DEFAULT_WIDTH,
            height: Self::DEFAULT_HEIGHT,
            ports: Vec::new(),
            interface_pattern,
        };

        // Determine configured or default ports
        let port_names: Vec<String> = if let Some(p) = &def.ports {
            p.clone()
        } else if let Some(tmpl) = template {
            tmpl.default_ports.iter().map(|s| s.to_string()).collect()
        } else {
            vec![
                "eth1".to_string(),
                "eth2".to_string(),
                "eth3".to_string(),
                "eth4".to_string(),
            ]
        };

        node.rebuild_ports(&port_names);
        node
    }

    pub fn from_profile(name: &str, profile: &NodeProfile, x: f64, y: f64) -> Self {
        let mut node = Self {
            name: name.to_string(),
            kind: profile.kind_name.clone(),
            image: profile.default_image.clone(),
            category: profile.node_category,
            x,
            y,
            width: Self::DEFAULT_WIDTH,
            height: Self::DEFAULT_HEIGHT,
            ports: Vec::new(),
            interface_pattern: profile.interface_pattern.clone(),
        };
        node.rebuild_ports(&profile.default_ports);
        node
    }

    /// Calculate connection point on node perimeter and direction facing a target point
    pub fn perimeter_connection_towards(
        &self,
        target_x: f64,
        target_y: f64,
    ) -> ((f64, f64), Direction) {
        self.perimeter_connection_towards_with_offset(target_x, target_y, 0.0)
    }

    /// Calculate connection point on node perimeter and direction facing a target point,
    /// with an optional lateral offset along the perimeter edge to separate parallel links.
    pub fn perimeter_connection_towards_with_offset(
        &self,
        target_x: f64,
        target_y: f64,
        offset: f64,
    ) -> ((f64, f64), Direction) {
        let cx = self.x + self.width / 2.0;
        let cy = self.y + self.height / 2.0;
        let dx = target_x - cx;
        let dy = target_y - cy;

        if dx.abs() < 1e-4 && dy.abs() < 1e-4 {
            let y_clamped = (cy + offset).clamp(self.y + 0.8, self.y + self.height - 0.8);
            return ((self.x + self.width, y_clamped), Direction::East);
        }

        let hw = self.width / 2.0;
        let hh = self.height / 2.0;

        let tx = if dx.abs() > 1e-4 {
            hw / dx.abs()
        } else {
            f64::INFINITY
        };
        let ty = if dy.abs() > 1e-4 {
            hh / dy.abs()
        } else {
            f64::INFINITY
        };

        if tx < ty {
            // Intersects East or West vertical edge
            let y_hit = cy + dy * tx;
            let y_clamped = (y_hit + offset).clamp(self.y + 0.8, self.y + self.height - 0.8);
            if dx > 0.0 {
                ((self.x + self.width, y_clamped), Direction::East)
            } else {
                ((self.x, y_clamped), Direction::West)
            }
        } else {
            // Intersects North or South horizontal edge
            let x_hit = cx + dx * ty;
            let x_clamped = (x_hit + offset).clamp(self.x + 1.0, self.x + self.width - 1.0);
            if dy > 0.0 {
                ((x_clamped, self.y + self.height), Direction::South)
            } else {
                ((x_clamped, self.y), Direction::North)
            }
        }
    }

    /// Rebuild port anchor locations around the perimeter
    pub fn rebuild_ports(&mut self, port_names: &[String]) {
        self.ports.clear();
        let total = port_names.len();
        if total == 0 {
            return;
        }

        // Distribute ports around East and West first (or South/North if many)
        for (i, name) in port_names.iter().enumerate() {
            let (px, py, dir) = if i % 2 == 0 {
                // Left / West side
                let rank = (i / 2) as f64;
                let count = total.div_ceil(2) as f64;
                let spacing = self.height / (count + 1.0);
                (self.x, self.y + (rank + 1.0) * spacing, Direction::West)
            } else {
                // Right / East side
                let rank = (i / 2) as f64;
                let count = (total / 2) as f64;
                let spacing = self.height / (count + 1.0);
                (
                    self.x + self.width,
                    self.y + (rank + 1.0) * spacing,
                    Direction::East,
                )
            };

            self.ports.push(PortAnchor {
                name: name.clone(),
                x: px,
                y: py,
                direction: dir,
            });
        }
    }

    /// Update node position and port anchors
    pub fn set_pos(&mut self, x: f64, y: f64) {
        let dx = x - self.x;
        let dy = y - self.y;
        self.x = x;
        self.y = y;
        for port in &mut self.ports {
            port.x += dx;
            port.y += dy;
        }
    }

    /// Check if point is inside the node's bounding box
    pub fn contains_point(&self, px: f64, py: f64) -> bool {
        px >= self.x && px <= self.x + self.width && py >= self.y && py <= self.y + self.height
    }

    /// Check if node bounding box intersects with given rectangle [rx1..rx2, ry1..ry2]
    pub fn intersects_rect(&self, rx1: f64, ry1: f64, rx2: f64, ry2: f64) -> bool {
        let left = rx1.min(rx2);
        let right = rx1.max(rx2);
        let top = ry1.min(ry2);
        let bottom = ry1.max(ry2);

        let node_right = self.x + self.width;
        let node_bottom = self.y + self.height;

        !(node_right < left || self.x > right || node_bottom < top || self.y > bottom)
    }

    /// Find port closest to point within threshold
    pub fn find_port_near(&self, px: f64, py: f64, threshold: f64) -> Option<&PortAnchor> {
        self.ports
            .iter()
            .filter(|p| {
                let dist = ((p.x - px).powi(2) + (p.y - py).powi(2)).sqrt();
                dist <= threshold
            })
            .min_by(|a, b| {
                let da = ((a.x - px).powi(2) + (a.y - py).powi(2)).sqrt();
                let db = ((b.x - px).powi(2) + (b.y - py).powi(2)).sqrt();
                da.partial_cmp(&db).unwrap_or(std::cmp::Ordering::Equal)
            })
    }

    /// Get port anchor by name
    pub fn get_port(&self, name: &str) -> Option<&PortAnchor> {
        self.ports.iter().find(|p| p.name == name)
    }

    /// Category symbol
    pub fn icon(&self) -> &'static str {
        match self.category {
            NodeCategory::Router => "⨂",
            NodeCategory::Switch => "⇋",
            NodeCategory::Host => "💻",
            NodeCategory::Firewall => "🛡",
        }
    }
}
