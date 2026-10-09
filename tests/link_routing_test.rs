use clab_tui::canvas::link::{OrthogonalRouter, Rect};
use clab_tui::canvas::node::Direction;

fn assert_orthogonal(waypoints: &[(f64, f64)]) {
    assert!(waypoints.len() >= 2, "Path must have at least 2 waypoints");
    for win in waypoints.windows(2) {
        let (p1, p2) = (win[0], win[1]);
        let dx = (p1.0 - p2.0).abs();
        let dy = (p1.1 - p2.1).abs();
        assert!(
            dx < 1e-4 || dy < 1e-4,
            "Segment from {:?} to {:?} is oblique (dx={}, dy={})",
            p1,
            p2,
            dx,
            dy
        );
    }
}

#[test]
fn test_straight_horizontal_routing() {
    let start = (10.0, 20.0);
    let end = (40.0, 20.0);
    let obstacles = vec![];

    let path = OrthogonalRouter::route(start, Direction::East, end, Direction::West, &obstacles);

    assert_orthogonal(&path);
    assert_eq!(path.first(), Some(&start));
    assert_eq!(path.last(), Some(&end));
}

#[test]
fn test_straight_vertical_routing() {
    let start = (20.0, 10.0);
    let end = (20.0, 40.0);
    let obstacles = vec![];

    let path = OrthogonalRouter::route(start, Direction::South, end, Direction::North, &obstacles);

    assert_orthogonal(&path);
    assert_eq!(path.first(), Some(&start));
    assert_eq!(path.last(), Some(&end));
}

#[test]
fn test_l_shaped_manhattan_routing() {
    let start = (10.0, 10.0);
    let end = (30.0, 25.0);
    let obstacles = vec![];

    let path = OrthogonalRouter::route(start, Direction::East, end, Direction::North, &obstacles);

    assert_orthogonal(&path);
    assert_eq!(path.first(), Some(&start));
    assert_eq!(path.last(), Some(&end));
}

#[test]
fn test_obstacle_avoidance() {
    let start = (10.0, 20.0);
    let end = (50.0, 20.0);

    // Place an obstacle directly in between (x: 25 to 35, y: 15 to 25)
    let obstacle = Rect::new(25.0, 15.0, 35.0, 25.0);
    let obstacles = vec![obstacle];

    let path = OrthogonalRouter::route(start, Direction::East, end, Direction::West, &obstacles);

    assert_orthogonal(&path);
    assert_eq!(path.first(), Some(&start));
    assert_eq!(path.last(), Some(&end));

    // Ensure no segment intersects the interior of the obstacle
    for win in path.windows(2) {
        let (p1, p2) = (win[0], win[1]);
        assert!(
            !obstacle.intersects_segment(p1, p2, 0.1),
            "Path segment from {:?} to {:?} intersects obstacle",
            p1,
            p2
        );
    }
}

#[test]
fn test_simplify_collinear_waypoints() {
    let raw = vec![
        (0.0, 0.0),
        (5.0, 0.0),
        (10.0, 0.0),
        (10.0, 5.0),
        (10.0, 10.0),
    ];
    let simplified = OrthogonalRouter::simplify_waypoints(raw);
    assert_eq!(simplified, vec![(0.0, 0.0), (10.0, 0.0), (10.0, 10.0)]);
}

#[test]
fn test_glyph_generation() {
    // Horizontal
    assert_eq!(
        OrthogonalRouter::get_glyph_at(Some((0.0, 5.0)), (5.0, 5.0), Some((10.0, 5.0))),
        '─'
    );
    // Vertical
    assert_eq!(
        OrthogonalRouter::get_glyph_at(Some((5.0, 0.0)), (5.0, 5.0), Some((5.0, 10.0))),
        '│'
    );
    // Corner going East then South
    assert_eq!(
        OrthogonalRouter::get_glyph_at(Some((0.0, 5.0)), (5.0, 5.0), Some((5.0, 10.0))),
        '┐'
    );
    // Corner going South then East
    assert_eq!(
        OrthogonalRouter::get_glyph_at(Some((5.0, 0.0)), (5.0, 5.0), Some((10.0, 5.0))),
        '└'
    );
}

#[test]
fn test_direct_diagonal_routing() {
    let start = (10.0, 10.0);
    let end = (35.0, 25.0);
    let path = OrthogonalRouter::route_direct(start, end);
    assert_eq!(path, vec![start, end]);
}

#[test]
fn test_octilinear_routing_and_chamfers() {
    let start = (10.0, 10.0);
    let end = (40.0, 30.0);
    let obstacles = vec![];

    let path = OrthogonalRouter::route_octilinear(
        start,
        Direction::East,
        end,
        Direction::West,
        &obstacles,
    );

    assert!(path.len() >= 2);
    assert_eq!(path.first(), Some(&start));
    assert_eq!(path.last(), Some(&end));

    // Ensure all segments are octilinear: dx == 0, dy == 0, or |dx| == |dy| (45 degrees)
    for win in path.windows(2) {
        let (p1, p2) = (win[0], win[1]);
        let dx = (p1.0 - p2.0).abs();
        let dy = (p1.1 - p2.1).abs();
        let is_h = dy < 1e-4;
        let is_v = dx < 1e-4;
        let is_diag_45 = (dx - dy).abs() < 1e-4;
        assert!(
            is_h || is_v || is_diag_45,
            "Segment from {:?} to {:?} is not octilinear (dx={}, dy={})",
            p1,
            p2,
            dx,
            dy
        );
    }
}

#[test]
fn test_simplify_collinear_diagonal_waypoints() {
    let raw = vec![
        (0.0, 0.0),
        (5.0, 5.0),
        (10.0, 10.0),
        (15.0, 15.0),
        (20.0, 15.0),
        (25.0, 15.0),
    ];
    let simplified = OrthogonalRouter::simplify_waypoints(raw);
    assert_eq!(simplified, vec![(0.0, 0.0), (15.0, 15.0), (25.0, 15.0)]);
}

#[test]
fn test_diagonal_glyph_generation() {
    // Straight diagonal down-right / up-left
    assert_eq!(
        OrthogonalRouter::get_glyph_at(Some((0.0, 0.0)), (5.0, 5.0), Some((10.0, 10.0))),
        '╲'
    );
    // Straight diagonal up-right / down-left
    assert_eq!(
        OrthogonalRouter::get_glyph_at(Some((0.0, 10.0)), (5.0, 5.0), Some((10.0, 0.0))),
        '╱'
    );
    // Crossing diagonals
    assert_eq!(
        OrthogonalRouter::get_glyph_at(Some((0.0, 0.0)), (5.0, 5.0), Some((10.0, 0.0))),
        '╳'
    );
    // Endpoint single-direction diagonal
    assert_eq!(
        OrthogonalRouter::get_glyph_at(None, (5.0, 5.0), Some((10.0, 10.0))),
        '╲'
    );
    assert_eq!(
        OrthogonalRouter::get_glyph_at(Some((0.0, 10.0)), (5.0, 5.0), None),
        '╱'
    );
}

#[test]
fn test_rect_diagonal_intersection() {
    let rect = Rect::new(10.0, 10.0, 20.0, 20.0);

    // Diagonal passing right through the center
    assert!(rect.intersects_segment((0.0, 0.0), (30.0, 30.0), 0.0));

    // Diagonal passing through opposite corners
    assert!(rect.intersects_segment((0.0, 30.0), (30.0, 0.0), 0.0));

    // Diagonal missing the box
    assert!(!rect.intersects_segment((0.0, 0.0), (5.0, 5.0), 0.0));
    assert!(!rect.intersects_segment((0.0, 25.0), (25.0, 25.0), 0.0));
}

#[test]
fn test_canvas_routing_style_cycle() {
    use clab_tui::canvas::link::RoutingStyle;
    use clab_tui::canvas::state::CanvasState;
    use clab_tui::clab::parser::TopologyParser;

    let topo = TopologyParser::create_sample_topology();
    let mut canvas = CanvasState::new();
    canvas.load_from_topology(&topo);

    assert_eq!(canvas.routing_style, RoutingStyle::Orthogonal);

    // Cycle to Direct
    let s1 = canvas.cycle_routing_style();
    assert_eq!(s1, RoutingStyle::Direct);
    assert_eq!(canvas.routing_style, RoutingStyle::Direct);
    assert_eq!(canvas.links[0].waypoints.len(), 2); // Direct line has 2 waypoints

    // Cycle to Octilinear
    let s2 = canvas.cycle_routing_style();
    assert_eq!(s2, RoutingStyle::Octilinear);
    assert_eq!(canvas.routing_style, RoutingStyle::Octilinear);

    // Cycle back to Orthogonal
    let s3 = canvas.cycle_routing_style();
    assert_eq!(s3, RoutingStyle::Orthogonal);
    assert_eq!(canvas.routing_style, RoutingStyle::Orthogonal);
}

#[test]
fn test_simplify_duplicate_waypoints() {
    let raw = vec![
        (0.0, 0.0),
        (0.0, 0.0), // duplicate
        (5.0, 5.0),
        (5.0, 5.0), // duplicate
        (10.0, 10.0),
        (10.0, 10.0), // duplicate at end
    ];
    let simplified = OrthogonalRouter::simplify_waypoints(raw);
    assert_eq!(simplified, vec![(0.0, 0.0), (10.0, 10.0)]);
}

#[test]
fn test_diagonal_to_orthogonal_bend_glyphs() {
    // Horizontal to Diagonal down-right (dx2=3, dy2=3): should produce '╲'
    assert_eq!(
        OrthogonalRouter::get_glyph_at(Some((0.0, 5.0)), (5.0, 5.0), Some((8.0, 8.0))),
        '╲'
    );
    // Horizontal to Diagonal up-right (dx2=3, dy2=-3): should produce '╱'
    assert_eq!(
        OrthogonalRouter::get_glyph_at(Some((0.0, 5.0)), (5.0, 5.0), Some((8.0, 2.0))),
        '╱'
    );
    // Vertical to Diagonal down-right (dx2=3, dy2=3): should produce '╲'
    assert_eq!(
        OrthogonalRouter::get_glyph_at(Some((5.0, 0.0)), (5.0, 5.0), Some((8.0, 8.0))),
        '╲'
    );
    // Diagonal down-right to Horizontal (dx1=3, dy1=3): should produce '╲'
    assert_eq!(
        OrthogonalRouter::get_glyph_at(Some((2.0, 2.0)), (5.0, 5.0), Some((10.0, 5.0))),
        '╲'
    );
    // Diagonal up-right to Vertical (dx1=3, dy1=-3): should produce '╱'
    assert_eq!(
        OrthogonalRouter::get_glyph_at(Some((2.0, 8.0)), (5.0, 5.0), Some((5.0, 0.0))),
        '╱'
    );
    // Zero-delta fallback delegation
    assert_eq!(
        OrthogonalRouter::get_glyph_at(Some((5.0, 5.0)), (5.0, 5.0), Some((10.0, 10.0))),
        '╲'
    );
}

#[test]
fn test_rect_intersects_segment_non_finite_safety() {
    let rect = Rect::new(10.0, 10.0, 20.0, 20.0);
    assert!(!rect.intersects_segment((f64::NAN, 15.0), (30.0, 15.0), 0.0));
    assert!(!rect.intersects_segment((15.0, 15.0), (f64::INFINITY, 15.0), 0.0));
}

#[test]
fn test_octilinear_routing_blocked_stub_fallback() {
    let start = (10.0, 10.0);
    let end = (40.0, 10.0);
    // Obstacle blocking direct path between stubs at (20.0, 5.0) to (30.0, 15.0)
    let obstacle = Rect::new(20.0, 5.0, 30.0, 15.0);
    let obstacles = vec![obstacle];

    let path = OrthogonalRouter::route_octilinear(
        start,
        Direction::East,
        end,
        Direction::West,
        &obstacles,
    );
    assert!(path.len() >= 2);
    // Obstacle must not be intersected by any path segment
    for win in path.windows(2) {
        let (p1, p2) = (win[0], win[1]);
        assert!(
            !obstacle.intersects_segment(p1, p2, 0.1),
            "Octilinear segment {:?} to {:?} must not penetrate obstacle",
            p1,
            p2
        );
    }
}
