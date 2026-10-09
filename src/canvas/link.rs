use crate::canvas::node::Direction;
use std::cmp::Ordering;
use std::collections::{BinaryHeap, HashMap};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rect {
    pub min_x: f64,
    pub min_y: f64,
    pub max_x: f64,
    pub max_y: f64,
}

impl Rect {
    pub fn new(min_x: f64, min_y: f64, max_x: f64, max_y: f64) -> Self {
        Self {
            min_x: min_x.min(max_x),
            min_y: min_y.min(max_y),
            max_x: min_x.max(max_x),
            max_y: min_y.max(max_y),
        }
    }

    /// Check if point is strictly inside (interior)
    pub fn contains_interior(&self, x: f64, y: f64, margin: f64) -> bool {
        x > (self.min_x - margin)
            && x < (self.max_x + margin)
            && y > (self.min_y - margin)
            && y < (self.max_y + margin)
    }

    /// Check if an axis-aligned segment intersects the interior of the rect
    pub fn intersects_segment(&self, p1: (f64, f64), p2: (f64, f64), margin: f64) -> bool {
        let (x1, y1) = p1;
        let (x2, y2) = p2;

        if !x1.is_finite() || !y1.is_finite() || !x2.is_finite() || !y2.is_finite() {
            return false;
        }

        let seg_min_x = x1.min(x2);
        let seg_max_x = x1.max(x2);
        let seg_min_y = y1.min(y2);
        let seg_max_y = y1.max(y2);

        let box_min_x = self.min_x - margin;
        let box_max_x = self.max_x + margin;
        let box_min_y = self.min_y - margin;
        let box_max_y = self.max_y + margin;

        if seg_max_x <= box_min_x
            || seg_min_x >= box_max_x
            || seg_max_y <= box_min_y
            || seg_min_y >= box_max_y
        {
            return false;
        }

        // Horizontal segment
        if (y1 - y2).abs() < 1e-4 && y1 > box_min_y && y1 < box_max_y {
            let overlap_min = seg_min_x.max(box_min_x);
            let overlap_max = seg_max_x.min(box_max_x);
            return overlap_max > overlap_min;
        }

        // Vertical segment
        if (x1 - x2).abs() < 1e-4 && x1 > box_min_x && x1 < box_max_x {
            let overlap_min = seg_min_y.max(box_min_y);
            let overlap_max = seg_max_y.min(box_max_y);
            return overlap_max > overlap_min;
        }

        // General / diagonal segment: Liang-Barsky line clipping against box interior
        let dx = x2 - x1;
        let dy = y2 - y1;

        let mut t0 = 0.0f64;
        let mut t1 = 1.0f64;

        let p = [-dx, dx, -dy, dy];
        let q = [
            x1 - box_min_x,
            box_max_x - x1,
            y1 - box_min_y,
            box_max_y - y1,
        ];

        for i in 0..4 {
            if p[i].abs() < 1e-9 {
                if q[i] <= 0.0 {
                    return false;
                }
            } else {
                let r = q[i] / p[i];
                if p[i] < 0.0 {
                    if r > t0 {
                        t0 = r;
                    }
                } else if r < t1 {
                    t1 = r;
                }
                if t0 >= t1 {
                    return false;
                }
            }
        }

        t0 < t1
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RoutingStyle {
    #[default]
    Orthogonal,
    Direct,
    Octilinear,
}

impl RoutingStyle {
    pub fn next(&self) -> Self {
        match self {
            RoutingStyle::Orthogonal => RoutingStyle::Direct,
            RoutingStyle::Direct => RoutingStyle::Octilinear,
            RoutingStyle::Octilinear => RoutingStyle::Orthogonal,
        }
    }

    pub fn name(&self) -> &'static str {
        match self {
            RoutingStyle::Orthogonal => "Orthogonal (Manhattan)",
            RoutingStyle::Direct => "Direct (Diagonal)",
            RoutingStyle::Octilinear => "Octilinear (45° Chamfers)",
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct CanvasLink {
    pub source_node: String,
    pub source_port: String,
    pub target_node: String,
    pub target_port: String,
    pub waypoints: Vec<(f64, f64)>,
    pub routing_style: RoutingStyle,
}

impl CanvasLink {
    pub fn new(
        source_node: impl Into<String>,
        source_port: impl Into<String>,
        target_node: impl Into<String>,
        target_port: impl Into<String>,
    ) -> Self {
        Self {
            source_node: source_node.into(),
            source_port: source_port.into(),
            target_node: target_node.into(),
            target_port: target_port.into(),
            waypoints: Vec::new(),
            routing_style: RoutingStyle::Orthogonal,
        }
    }

    pub fn with_routing_style(mut self, style: RoutingStyle) -> Self {
        self.routing_style = style;
        self
    }
}

#[derive(Copy, Clone, PartialEq, Eq)]
struct GridPoint {
    xi: usize,
    yi: usize,
}

#[derive(Copy, Clone, PartialEq)]
struct PathNode {
    cost: f64,
    bends: usize,
    point: GridPoint,
    incoming_dir: Option<Direction>,
}

impl Eq for PathNode {}

impl Ord for PathNode {
    fn cmp(&self, other: &Self) -> Ordering {
        // Lower total score has higher priority in BinaryHeap
        let score_self = self.cost + (self.bends as f64) * 5.0;
        let score_other = other.cost + (other.bends as f64) * 5.0;
        score_other
            .partial_cmp(&score_self)
            .unwrap_or(Ordering::Equal)
    }
}

impl PartialOrd for PathNode {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

pub struct OrthogonalRouter;

impl OrthogonalRouter {
    pub const STUB_LENGTH: f64 = 2.0;
    pub const OBSTACLE_MARGIN: f64 = 1.0;

    /// Calculate step vector for a direction
    pub fn direction_vector(dir: Direction, length: f64) -> (f64, f64) {
        match dir {
            Direction::North => (0.0, -length),
            Direction::South => (0.0, length),
            Direction::East => (length, 0.0),
            Direction::West => (-length, 0.0),
        }
    }

    /// Route orthogonal Manhattan path between two port anchors around obstacles
    pub fn route(
        start: (f64, f64),
        start_dir: Direction,
        end: (f64, f64),
        end_dir: Direction,
        obstacles: &[Rect],
    ) -> Vec<(f64, f64)> {
        Self::route_ext(start, start_dir, end, end_dir, obstacles, Self::STUB_LENGTH)
    }

    /// Route orthogonal Manhattan path with specified stub length
    pub fn route_ext(
        start: (f64, f64),
        start_dir: Direction,
        end: (f64, f64),
        end_dir: Direction,
        obstacles: &[Rect],
        stub_length: f64,
    ) -> Vec<(f64, f64)> {
        // Fast path for identical points
        if (start.0 - end.0).abs() < 1e-4 && (start.1 - end.1).abs() < 1e-4 {
            return vec![start, end];
        }

        let (sdx, sdy) = Self::direction_vector(start_dir, stub_length);
        let stub1 = (start.0 + sdx, start.1 + sdy);

        let (edx, edy) = Self::direction_vector(end_dir, stub_length);
        let stub2 = (end.0 + edx, end.1 + edy);

        // Try direct simple routing first (Horizontal then Vertical or Vertical then Horizontal)
        if let Some(direct_path) = Self::try_simple_route(stub1, stub2, obstacles) {
            let mut full = Vec::with_capacity(direct_path.len() + 2);
            full.push(start);
            full.extend(direct_path);
            full.push(end);
            return Self::simplify_waypoints(full);
        }

        // Hanan Grid A* router
        let mut xs: Vec<f64> = Vec::new();
        let mut ys: Vec<f64> = Vec::new();

        xs.push(stub1.0);
        xs.push(stub2.0);
        ys.push(stub1.1);
        ys.push(stub2.1);

        // Add midpoint coordinates to give extra routing channels
        xs.push((stub1.0 + stub2.0) / 2.0);
        ys.push((stub1.1 + stub2.1) / 2.0);

        for obs in obstacles {
            xs.push(obs.min_x - Self::OBSTACLE_MARGIN);
            xs.push(obs.max_x + Self::OBSTACLE_MARGIN);
            ys.push(obs.min_y - Self::OBSTACLE_MARGIN);
            ys.push(obs.max_y + Self::OBSTACLE_MARGIN);
        }

        // Sort and deduplicate with tolerance
        xs.sort_by(|a, b| a.partial_cmp(b).unwrap());
        ys.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let x_grid = Self::dedup_coords(xs);
        let y_grid = Self::dedup_coords(ys);

        let start_xi = Self::find_closest_coord(&x_grid, stub1.0);
        let start_yi = Self::find_closest_coord(&y_grid, stub1.1);
        let end_xi = Self::find_closest_coord(&x_grid, stub2.0);
        let end_yi = Self::find_closest_coord(&y_grid, stub2.1);

        let start_pt = GridPoint {
            xi: start_xi,
            yi: start_yi,
        };
        let end_pt = GridPoint {
            xi: end_xi,
            yi: end_yi,
        };

        if let Some(grid_path) = Self::astar_search(start_pt, end_pt, &x_grid, &y_grid, obstacles) {
            let mut waypoints = Vec::new();
            waypoints.push(start);
            for gp in grid_path {
                waypoints.push((x_grid[gp.xi], y_grid[gp.yi]));
            }
            waypoints.push(end);
            return Self::simplify_waypoints(waypoints);
        }

        // Fallback: simple 2-bend Manhattan line if search fails
        let mid_x = (start.0 + end.0) / 2.0;
        let fallback = vec![start, (mid_x, start.1), (mid_x, end.1), end];
        Self::simplify_waypoints(fallback)
    }

    fn try_simple_route(
        p1: (f64, f64),
        p2: (f64, f64),
        obstacles: &[Rect],
    ) -> Option<Vec<(f64, f64)>> {
        // Direct straight line?
        if (p1.0 - p2.0).abs() < 1e-4 && !Self::segment_blocked(p1, p2, obstacles) {
            return Some(vec![p1, p2]);
        }
        if (p1.1 - p2.1).abs() < 1e-4 && !Self::segment_blocked(p1, p2, obstacles) {
            return Some(vec![p1, p2]);
        }

        // H then V: p1 -> (p2.0, p1.1) -> p2
        let corner_hv = (p2.0, p1.1);
        if !Self::segment_blocked(p1, corner_hv, obstacles)
            && !Self::segment_blocked(corner_hv, p2, obstacles)
        {
            return Some(vec![p1, corner_hv, p2]);
        }

        // V then H: p1 -> (p1.0, p2.1) -> p2
        let corner_vh = (p1.0, p2.1);
        if !Self::segment_blocked(p1, corner_vh, obstacles)
            && !Self::segment_blocked(corner_vh, p2, obstacles)
        {
            return Some(vec![p1, corner_vh, p2]);
        }

        None
    }

    fn segment_blocked(p1: (f64, f64), p2: (f64, f64), obstacles: &[Rect]) -> bool {
        for obs in obstacles {
            if obs.intersects_segment(p1, p2, 0.2) {
                return true;
            }
        }
        false
    }

    fn dedup_coords(coords: Vec<f64>) -> Vec<f64> {
        let mut res: Vec<f64> = Vec::new();
        for c in coords {
            if res.is_empty() || (res.last().copied().unwrap_or(0.0) - c).abs() > 0.4 {
                res.push(c);
            }
        }
        res
    }

    fn find_closest_coord(grid: &[f64], val: f64) -> usize {
        let mut best_i = 0;
        let mut min_diff = f64::MAX;
        for (i, &g) in grid.iter().enumerate() {
            let diff = (g - val).abs();
            if diff < min_diff {
                min_diff = diff;
                best_i = i;
            }
        }
        best_i
    }

    fn astar_search(
        start: GridPoint,
        goal: GridPoint,
        x_grid: &[f64],
        y_grid: &[f64],
        obstacles: &[Rect],
    ) -> Option<Vec<GridPoint>> {
        let mut heap = BinaryHeap::new();
        let mut came_from: HashMap<(usize, usize), (GridPoint, Option<Direction>)> = HashMap::new();
        let mut cost_so_far: HashMap<(usize, usize), f64> = HashMap::new();

        heap.push(PathNode {
            cost: 0.0,
            bends: 0,
            point: start,
            incoming_dir: None,
        });
        cost_so_far.insert((start.xi, start.yi), 0.0);

        let mut iterations = 0;
        const MAX_ITERATIONS: usize = 3000;

        while let Some(current) = heap.pop() {
            iterations += 1;
            if iterations > MAX_ITERATIONS {
                break;
            }

            if current.point.xi == goal.xi && current.point.yi == goal.yi {
                // Reconstruct path
                let mut path = Vec::new();
                let mut curr_pt = goal;
                path.push(curr_pt);
                while let Some(&(prev, _)) = came_from.get(&(curr_pt.xi, curr_pt.yi)) {
                    path.push(prev);
                    if prev == start {
                        break;
                    }
                    curr_pt = prev;
                }
                path.reverse();
                return Some(path);
            }

            let curr_x = x_grid[current.point.xi];
            let curr_y = y_grid[current.point.yi];

            // 4 neighbors
            let neighbors = [
                (
                    if current.point.xi > 0 {
                        Some(current.point.xi - 1)
                    } else {
                        None
                    },
                    Some(current.point.yi),
                    Direction::West,
                ),
                (
                    if current.point.xi + 1 < x_grid.len() {
                        Some(current.point.xi + 1)
                    } else {
                        None
                    },
                    Some(current.point.yi),
                    Direction::East,
                ),
                (
                    Some(current.point.xi),
                    if current.point.yi > 0 {
                        Some(current.point.yi - 1)
                    } else {
                        None
                    },
                    Direction::North,
                ),
                (
                    Some(current.point.xi),
                    if current.point.yi + 1 < y_grid.len() {
                        Some(current.point.yi + 1)
                    } else {
                        None
                    },
                    Direction::South,
                ),
            ];

            for (nxt_xi_opt, nxt_yi_opt, move_dir) in neighbors {
                let (nxt_xi, nxt_yi) = match (nxt_xi_opt, nxt_yi_opt) {
                    (Some(xi), Some(yi)) => (xi, yi),
                    _ => continue,
                };

                let nxt_x = x_grid[nxt_xi];
                let nxt_y = y_grid[nxt_yi];

                // Check segment collision with obstacle interiors
                if Self::segment_blocked((curr_x, curr_y), (nxt_x, nxt_y), obstacles) {
                    continue;
                }

                let dist = (curr_x - nxt_x).abs() + (curr_y - nxt_y).abs();
                let bend_penalty = if let Some(prev_dir) = current.incoming_dir {
                    if prev_dir != move_dir {
                        8.0
                    } else {
                        0.0
                    }
                } else {
                    0.0
                };

                let new_cost = cost_so_far
                    .get(&(current.point.xi, current.point.yi))
                    .copied()
                    .unwrap_or(0.0)
                    + dist
                    + bend_penalty;

                let next_pt = GridPoint {
                    xi: nxt_xi,
                    yi: nxt_yi,
                };
                let current_recorded_cost = cost_so_far
                    .get(&(next_pt.xi, next_pt.yi))
                    .copied()
                    .unwrap_or(f64::MAX);

                if new_cost < current_recorded_cost {
                    cost_so_far.insert((next_pt.xi, next_pt.yi), new_cost);
                    came_from.insert((next_pt.xi, next_pt.yi), (current.point, Some(move_dir)));

                    let h = (nxt_x - x_grid[goal.xi]).abs() + (nxt_y - y_grid[goal.yi]).abs();
                    let bends = current.bends + if bend_penalty > 0.0 { 1 } else { 0 };

                    heap.push(PathNode {
                        cost: new_cost + h,
                        bends,
                        point: next_pt,
                        incoming_dir: Some(move_dir),
                    });
                }
            }
        }

        None
    }

    /// Route direct line straight between two endpoints
    pub fn route_direct(start: (f64, f64), end: (f64, f64)) -> Vec<(f64, f64)> {
        vec![start, end]
    }

    /// Route octilinear path with 45-degree diagonal segments around obstacles or direct 45-degree chamfers
    pub fn route_octilinear(
        start: (f64, f64),
        start_dir: Direction,
        end: (f64, f64),
        end_dir: Direction,
        obstacles: &[Rect],
    ) -> Vec<(f64, f64)> {
        Self::route_octilinear_ext(start, start_dir, end, end_dir, obstacles, Self::STUB_LENGTH)
    }

    /// Route octilinear path with custom stub length
    pub fn route_octilinear_ext(
        start: (f64, f64),
        start_dir: Direction,
        end: (f64, f64),
        end_dir: Direction,
        obstacles: &[Rect],
        stub_length: f64,
    ) -> Vec<(f64, f64)> {
        if (start.0 - end.0).abs() < 1e-4 && (start.1 - end.1).abs() < 1e-4 {
            return vec![start, end];
        }

        let (sdx, sdy) = Self::direction_vector(start_dir, stub_length);
        let stub1 = (start.0 + sdx, start.1 + sdy);

        let (edx, edy) = Self::direction_vector(end_dir, stub_length);
        let stub2 = (end.0 + edx, end.1 + edy);

        // Check simple octilinear route first if stubs are clear
        if !Self::segment_blocked(start, stub1, obstacles)
            && !Self::segment_blocked(stub2, end, obstacles)
        {
            if let Some(path) = Self::try_simple_octilinear_route(stub1, stub2, obstacles) {
                let mut full = Vec::with_capacity(path.len() + 2);
                full.push(start);
                full.extend(path);
                full.push(end);
                return Self::simplify_waypoints(full);
            }
        }

        // Route orthogonal path around obstacles and chamfer corners with 45° diagonal segments
        let ortho = Self::route_ext(start, start_dir, end, end_dir, obstacles, stub_length);
        let chamfered = Self::chamfer_corners(ortho, obstacles, 2.0);
        Self::simplify_waypoints(chamfered)
    }

    fn try_simple_octilinear_route(
        p1: (f64, f64),
        p2: (f64, f64),
        obstacles: &[Rect],
    ) -> Option<Vec<(f64, f64)>> {
        // 1. Direct segment (straight H, V, or pure 45° diagonal)
        let dx = p2.0 - p1.0;
        let dy = p2.1 - p1.1;
        let is_diag_45 = (dx.abs() - dy.abs()).abs() < 1e-4;
        let is_straight = dx.abs() < 1e-4 || dy.abs() < 1e-4 || is_diag_45;

        if is_straight && !Self::segment_blocked(p1, p2, obstacles) {
            return Some(vec![p1, p2]);
        }

        // 2. Octilinear 2-segment path: one 45° diagonal and one orthogonal
        if dx.abs() > dy.abs() {
            // A: Diagonal then Horizontal
            let corner_dh = (p1.0 + dy.abs() * dx.signum(), p2.1);
            if !Self::segment_blocked(p1, corner_dh, obstacles)
                && !Self::segment_blocked(corner_dh, p2, obstacles)
            {
                return Some(vec![p1, corner_dh, p2]);
            }

            // B: Horizontal then Diagonal
            let corner_hd = (p2.0 - dy.abs() * dx.signum(), p1.1);
            if !Self::segment_blocked(p1, corner_hd, obstacles)
                && !Self::segment_blocked(corner_hd, p2, obstacles)
            {
                return Some(vec![p1, corner_hd, p2]);
            }
        } else {
            // A: Diagonal then Vertical
            let corner_dv = (p2.0, p1.1 + dx.abs() * dy.signum());
            if !Self::segment_blocked(p1, corner_dv, obstacles)
                && !Self::segment_blocked(corner_dv, p2, obstacles)
            {
                return Some(vec![p1, corner_dv, p2]);
            }

            // B: Vertical then Diagonal
            let corner_vd = (p1.0, p2.1 - dx.abs() * dy.signum());
            if !Self::segment_blocked(p1, corner_vd, obstacles)
                && !Self::segment_blocked(corner_vd, p2, obstacles)
            {
                return Some(vec![p1, corner_vd, p2]);
            }
        }

        None
    }

    fn chamfer_corners(
        points: Vec<(f64, f64)>,
        obstacles: &[Rect],
        max_chamfer: f64,
    ) -> Vec<(f64, f64)> {
        if points.len() < 3 {
            return points;
        }

        let mut result = Vec::with_capacity(points.len() * 2);
        result.push(points[0]);

        for i in 1..points.len() - 1 {
            let prev = points[i - 1];
            let curr = points[i];
            let next = points[i + 1];

            let dx1 = curr.0 - prev.0;
            let dy1 = curr.1 - prev.1;
            let dx2 = next.0 - curr.0;
            let dy2 = next.1 - curr.1;

            let len1 = (dx1 * dx1 + dy1 * dy1).sqrt();
            let len2 = (dx2 * dx2 + dy2 * dy2).sqrt();

            // Only chamfer 90-degree orthogonal corners (H-to-V or V-to-H)
            let is_corner_90 = ((dy1.abs() < 1e-4 && dx2.abs() < 1e-4)
                || (dx1.abs() < 1e-4 && dy2.abs() < 1e-4))
                && len1 > 1e-4
                && len2 > 1e-4;

            if is_corner_90 {
                let c = max_chamfer.min(len1 / 2.0).min(len2 / 2.0);
                if c >= 0.5 {
                    let u1 = (dx1 / len1, dy1 / len1);
                    let u2 = (dx2 / len2, dy2 / len2);
                    let p_in = (curr.0 - c * u1.0, curr.1 - c * u1.1);
                    let p_out = (curr.0 + c * u2.0, curr.1 + c * u2.1);

                    if !Self::segment_blocked(p_in, p_out, obstacles) {
                        result.push(p_in);
                        result.push(p_out);
                        continue;
                    }
                }
            }

            result.push(curr);
        }

        result.push(*points.last().unwrap());
        result
    }

    /// Route according to specified RoutingStyle
    pub fn route_with_style(
        start: (f64, f64),
        start_dir: Direction,
        end: (f64, f64),
        end_dir: Direction,
        obstacles: &[Rect],
        style: RoutingStyle,
    ) -> Vec<(f64, f64)> {
        Self::route_with_style_and_stub(
            start,
            start_dir,
            end,
            end_dir,
            obstacles,
            style,
            Self::STUB_LENGTH,
        )
    }

    /// Route according to specified RoutingStyle with custom stub length
    pub fn route_with_style_and_stub(
        start: (f64, f64),
        start_dir: Direction,
        end: (f64, f64),
        end_dir: Direction,
        obstacles: &[Rect],
        style: RoutingStyle,
        stub_length: f64,
    ) -> Vec<(f64, f64)> {
        match style {
            RoutingStyle::Orthogonal => {
                Self::route_ext(start, start_dir, end, end_dir, obstacles, stub_length)
            }
            RoutingStyle::Direct => Self::route_direct(start, end),
            RoutingStyle::Octilinear => {
                Self::route_octilinear_ext(start, start_dir, end, end_dir, obstacles, stub_length)
            }
        }
    }

    /// Simplify waypoints removing unnecessary intermediate collinear points
    pub fn simplify_waypoints(points: Vec<(f64, f64)>) -> Vec<(f64, f64)> {
        if points.len() <= 2 {
            return points;
        }

        let mut simplified = Vec::with_capacity(points.len());
        simplified.push(points[0]);

        for i in 1..points.len() - 1 {
            let prev = simplified.last().unwrap();
            let curr = points[i];
            let next = points[i + 1];

            let dx1 = curr.0 - prev.0;
            let dy1 = curr.1 - prev.1;
            let dx2 = next.0 - curr.0;
            let dy2 = next.1 - curr.1;

            // Discard duplicate points
            if (dx1.abs() < 1e-4 && dy1.abs() < 1e-4) || (dx2.abs() < 1e-4 && dy2.abs() < 1e-4) {
                continue;
            }

            // Check if both segments are collinear and in the same direction
            let cross = dx1 * dy2 - dy1 * dx2;
            let dot = dx1 * dx2 + dy1 * dy2;
            let is_collinear = cross.abs() < 1e-4 && dot > 0.0;

            if !is_collinear {
                simplified.push(curr);
            }
        }

        let last = *points.last().unwrap();
        if let Some(prev) = simplified.last() {
            if (prev.0 - last.0).abs() < 1e-4 && (prev.1 - last.1).abs() < 1e-4 {
                return simplified;
            }
        }
        simplified.push(last);
        simplified
    }

    /// Convert a path of orthogonal/diagonal points into character glyphs for rendering
    pub fn get_glyph_at(
        prev: Option<(f64, f64)>,
        curr: (f64, f64),
        next: Option<(f64, f64)>,
    ) -> char {
        match (prev, next) {
            (None, None) => '·',
            (None, Some(n)) => {
                let dx = (n.0 - curr.0).round() as i32;
                let dy = (n.1 - curr.1).round() as i32;
                if dx == 0 && dy == 0 {
                    '·'
                } else if dy == 0 {
                    '─'
                } else if dx == 0 {
                    '│'
                } else if dx.abs() == dy.abs() {
                    if (dx > 0 && dy > 0) || (dx < 0 && dy < 0) {
                        '╲'
                    } else {
                        '╱'
                    }
                } else if dx.abs() > dy.abs() {
                    '─'
                } else {
                    '│'
                }
            }
            (Some(p), None) => {
                let dx = (curr.0 - p.0).round() as i32;
                let dy = (curr.1 - p.1).round() as i32;
                if dx == 0 && dy == 0 {
                    '·'
                } else if dy == 0 {
                    '─'
                } else if dx == 0 {
                    '│'
                } else if dx.abs() == dy.abs() {
                    if (dx > 0 && dy > 0) || (dx < 0 && dy < 0) {
                        '╲'
                    } else {
                        '╱'
                    }
                } else if dx.abs() > dy.abs() {
                    '─'
                } else {
                    '│'
                }
            }
            (Some(p), Some(n)) => {
                let dx1 = (curr.0 - p.0).round() as i32;
                let dy1 = (curr.1 - p.1).round() as i32;
                let dx2 = (n.0 - curr.0).round() as i32;
                let dy2 = (n.1 - curr.1).round() as i32;

                if dx1 == 0 && dy1 == 0 {
                    return Self::get_glyph_at(None, curr, Some(n));
                }
                if dx2 == 0 && dy2 == 0 {
                    return Self::get_glyph_at(Some(p), curr, None);
                }

                // Straight horizontal
                if dy1 == 0 && dy2 == 0 {
                    '─'
                }
                // Straight vertical
                else if dx1 == 0 && dx2 == 0 {
                    '│'
                }
                // Straight diagonal down-right / up-left
                else if ((dx1 > 0 && dy1 > 0) || (dx1 < 0 && dy1 < 0))
                    && ((dx2 > 0 && dy2 > 0) || (dx2 < 0 && dy2 < 0))
                {
                    '╲'
                }
                // Straight diagonal up-right / down-left
                else if ((dx1 > 0 && dy1 < 0) || (dx1 < 0 && dy1 > 0))
                    && ((dx2 > 0 && dy2 < 0) || (dx2 < 0 && dy2 > 0))
                {
                    '╱'
                }
                // Crossing diagonals
                else if (((dx1 > 0 && dy1 > 0) || (dx1 < 0 && dy1 < 0))
                    && ((dx2 > 0 && dy2 < 0) || (dx2 < 0 && dy2 > 0)))
                    || (((dx1 > 0 && dy1 < 0) || (dx1 < 0 && dy1 > 0))
                        && ((dx2 > 0 && dy2 > 0) || (dx2 < 0 && dy2 < 0)))
                {
                    '╳'
                }
                // Orthogonal 90° corners
                else if (dy1 == 0 && dx2 == 0) || (dx1 == 0 && dy2 == 0) {
                    if (dx1 > 0 && dy2 > 0) || (dy1 < 0 && dx2 < 0) {
                        '┐'
                    } else if (dx1 > 0 && dy2 < 0) || (dy1 > 0 && dx2 < 0) {
                        '┘'
                    } else if (dx1 < 0 && dy2 > 0) || (dy1 < 0 && dx2 > 0) {
                        '┌'
                    } else if (dx1 < 0 && dy2 < 0) || (dy1 > 0 && dx2 > 0) {
                        '└'
                    } else {
                        '┼'
                    }
                }
                // Diagonal bends connecting orthogonal to diagonal or vice versa
                else if (dx1 * dy1 != 0) || (dx2 * dy2 != 0) {
                    let net_diag = if dx1 * dy1 != 0 { dx1 * dy1 } else { dx2 * dy2 };
                    if net_diag > 0 {
                        '╲'
                    } else if net_diag < 0 {
                        '╱'
                    } else {
                        '┼'
                    }
                } else {
                    '┼'
                }
            }
        }
    }
}
