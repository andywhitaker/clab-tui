use clab_tui::app::App;
use clab_tui::canvas::node::CanvasNode;
use clab_tui::canvas::render::CanvasRenderer;
use clab_tui::canvas::state::{CanvasMode, CanvasState};
use clab_tui::clab::model::{LabTopology, NodeDefinition, TopologyData};
use clab_tui::clab::parser::TopologyParser;
use clab_tui::ui::layout::ActiveTab;
use clab_tui::ui::theme::Theme;
use clab_tui::ui::widgets::drawer::InspectorDrawer;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use ratatui::backend::TestBackend;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::Terminal;
use std::collections::BTreeMap;
use std::process::Command;
use tokio::sync::mpsc::unbounded_channel;

fn key_event(code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, KeyModifiers::NONE)
}

fn key_event_mod(code: KeyCode, modifiers: KeyModifiers) -> KeyEvent {
    KeyEvent::new(code, modifiers)
}

fn buffer_contains(terminal: &Terminal<TestBackend>, needle: &str) -> bool {
    let buf = terminal.backend().buffer();
    for y in buf.area.top()..buf.area.bottom() {
        let mut row = String::new();
        for x in buf.area.left()..buf.area.right() {
            row.push_str(buf[(x, y)].symbol());
        }
        if row.contains(needle) {
            return true;
        }
    }
    false
}

fn buffer_to_string(buf: &Buffer) -> String {
    let mut out = String::new();
    for y in buf.area.top()..buf.area.bottom() {
        for x in buf.area.left()..buf.area.right() {
            out.push_str(buf[(x, y)].symbol());
        }
        out.push('\n');
    }
    out
}

fn make_node(name: &str, x: f64, y: f64, ports: &[&str]) -> CanvasNode {
    let def = NodeDefinition {
        kind: Some("srl".to_string()),
        ..Default::default()
    };
    let mut node = CanvasNode::new(name, &def, x, y);
    node.rebuild_ports(&ports.iter().map(|s| s.to_string()).collect::<Vec<_>>());
    node
}

#[test]
fn test_parallel_links_different_waypoints_and_endpoints() {
    let mut canvas = CanvasState::new();

    let node_a = make_node("node_a", 10.0, 10.0, &["e1-1", "e1-2", "e1-3"]);
    canvas.nodes.insert(node_a.name.clone(), node_a);

    let node_b = make_node("node_b", 60.0, 10.0, &["e1-1", "e1-2", "e1-3"]);
    canvas.nodes.insert(node_b.name.clone(), node_b);

    canvas.add_link("node_a", "e1-1", "node_b", "e1-1");
    canvas.add_link("node_a", "e1-2", "node_b", "e1-2");
    canvas.recalculate_all_links();

    assert_eq!(canvas.links.len(), 2);
    let link1 = &canvas.links[0];
    let link2 = &canvas.links[1];

    // Waypoints must not be identical
    assert_ne!(
        link1.waypoints, link2.waypoints,
        "Parallel links must have distinct waypoints"
    );

    // Endpoints (perimeter attachments) must be separated
    let p1_start = link1.waypoints.first().unwrap();
    let p2_start = link2.waypoints.first().unwrap();
    assert_ne!(
        p1_start, p2_start,
        "Perimeter start points of parallel links must differ"
    );

    let p1_end = link1.waypoints.last().unwrap();
    let p2_end = link2.waypoints.last().unwrap();
    assert_ne!(
        p1_end, p2_end,
        "Perimeter end points of parallel links must differ"
    );

    // Add a 3rd parallel link
    canvas.add_link("node_a", "e1-3", "node_b", "e1-3");
    canvas.recalculate_all_links();
    assert_eq!(canvas.links.len(), 3);

    let starts: Vec<(f64, f64)> = canvas
        .links
        .iter()
        .map(|l| *l.waypoints.first().unwrap())
        .collect();
    // Verify all 3 start points are distinct
    assert_ne!(starts[0], starts[1]);
    assert_ne!(starts[1], starts[2]);
    assert_ne!(starts[0], starts[2]);
}

#[test]
fn test_parallel_links_badge_rendering_no_overlap() {
    let mut canvas = CanvasState::new();

    let node_a = make_node("node_a", 10.0, 10.0, &["e1-1", "e1-2"]);
    canvas.nodes.insert(node_a.name.clone(), node_a);

    let node_b = make_node("node_b", 60.0, 10.0, &["e1-1", "e1-2"]);
    canvas.nodes.insert(node_b.name.clone(), node_b);

    canvas.add_link("node_a", "e1-1", "node_b", "e1-1");
    canvas.add_link("node_a", "e1-2", "node_b", "e1-2");
    canvas.recalculate_all_links();

    let area = Rect::new(0, 0, 100, 30);
    let mut buf = Buffer::empty(area);
    let theme = Theme::tokyo_night();

    CanvasRenderer::render(&canvas, area, &mut buf, &theme);
    let rendered = buffer_to_string(&buf);

    // Both interface labels must be visible in the buffer
    assert!(
        rendered.contains("e1-1"),
        "Buffer should contain e1-1 interface label"
    );
    assert!(
        rendered.contains("e1-2"),
        "Buffer should contain e1-2 interface label"
    );
}

#[test]
fn test_parallel_links_click_hit_test() {
    let mut canvas = CanvasState::new();

    let node_a = make_node("node_a", 10.0, 10.0, &["e1-1", "e1-2"]);
    canvas.nodes.insert(node_a.name.clone(), node_a);

    let node_b = make_node("node_b", 60.0, 10.0, &["e1-1", "e1-2"]);
    canvas.nodes.insert(node_b.name.clone(), node_b);

    canvas.add_link("node_a", "e1-1", "node_b", "e1-1");
    canvas.add_link("node_a", "e1-2", "node_b", "e1-2");
    canvas.recalculate_all_links();

    // Link 0 and Link 1 have different waypoints and vertical separation.
    // Pick midpoint on Link 0 wire segment
    let p0_start = canvas.links[0].waypoints.first().unwrap();
    let p0_end = canvas.links[0].waypoints.last().unwrap();
    let mid0_x = (p0_start.0 + p0_end.0) / 2.0;
    let mid0_y = (p0_start.1 + p0_end.1) / 2.0;

    canvas.handle_click(mid0_x, mid0_y);
    assert_eq!(
        canvas.selected_link,
        Some(0),
        "Clicking link 0 midpoint should select link 0"
    );

    // Pick midpoint on Link 1 wire segment
    let p1_start = canvas.links[1].waypoints.first().unwrap();
    let p1_end = canvas.links[1].waypoints.last().unwrap();
    let mid1_x = (p1_start.0 + p1_end.0) / 2.0;
    let mid1_y = (p1_start.1 + p1_end.1) / 2.0;

    canvas.handle_click(mid1_x, mid1_y);
    assert_eq!(
        canvas.selected_link,
        Some(1),
        "Clicking link 1 midpoint should select link 1"
    );
}

#[test]
fn test_node_definition_interface_pattern_skipped_in_yaml() {
    let mut nodes = BTreeMap::new();
    let node_def = NodeDefinition {
        kind: Some("srl".to_string()),
        image: Some("ghcr.io/nokia/srlinux".to_string()),
        interface_pattern: Some("e1-[1-10]".to_string()),
        ..Default::default()
    };
    nodes.insert("srl1".to_string(), node_def);

    let topo = LabTopology {
        name: "test-topo".to_string(),
        topology: TopologyData {
            nodes,
            links: vec![],
            ..Default::default()
        },
        ..Default::default()
    };

    let yaml_str = serde_yaml::to_string(&topo).expect("Serialization failed");

    // interface_pattern must NOT be serialized
    assert!(
        !yaml_str.contains("interface_pattern"),
        "YAML output must not contain 'interface_pattern'"
    );
    assert!(
        !yaml_str.contains("interface-pattern"),
        "YAML output must not contain 'interface-pattern'"
    );
    assert!(
        !yaml_str.contains("e1-[1-10]"),
        "YAML output must not contain the interface pattern value"
    );

    // Deserialize back: must parse successfully and interface_pattern is None
    let parsed: LabTopology = serde_yaml::from_str(&yaml_str).expect("Deserialization failed");
    let deserialized_node = parsed.topology.nodes.get("srl1").unwrap();
    assert_eq!(deserialized_node.interface_pattern, None);
}

#[test]
fn test_drawer_removes_configured_ports_section() {
    let topo = TopologyParser::create_sample_topology();
    let mut canvas = CanvasState::new();
    canvas.load_from_topology(&topo);
    let theme = Theme::tokyo_night();

    let mut drawer = InspectorDrawer::new();
    drawer.open_for_node("srl1");

    let area = Rect::new(0, 0, 100, 40);
    let mut buf = Buffer::empty(area);
    drawer.render(&canvas, &topo, area, &mut buf, &theme);

    let rendered = buffer_to_string(&buf);
    assert!(
        !rendered.contains("Configured Ports"),
        "Drawer must not render 'Configured Ports' section"
    );
    assert!(
        !rendered.contains("Configured Ports:"),
        "Drawer must not render 'Configured Ports:' heading"
    );
}

#[tokio::test]
async fn test_inspect_shortcuts_and_hints_removed() {
    let backend = TestBackend::new(120, 40);
    let mut terminal = Terminal::new(backend).unwrap();
    let (tx, _rx) = unbounded_channel();

    let mut app = App::new(None, None, true);
    app.active_tab = ActiveTab::Inspect;

    terminal.draw(|f| app.render(f)).unwrap();

    // Verify footer and header omit shell and packet capture
    assert!(!buffer_contains(&terminal, "[e] Exec Shell"));
    assert!(!buffer_contains(&terminal, "[c] Packet Capture"));
    assert!(!buffer_contains(&terminal, "e Shell"));
    assert!(!buffer_contains(&terminal, "c Capture"));

    // Verify key 'e' does not open exec shell or crash
    app.handle_key(key_event(KeyCode::Char('e')), &tx);
    assert_eq!(app.active_tab, ActiveTab::Inspect);
    assert!(app.running);

    // Verify key 'c' does not open capture or crash
    app.handle_key(key_event(KeyCode::Char('c')), &tx);
    assert_eq!(app.active_tab, ActiveTab::Inspect);
    assert!(app.running);
}

#[tokio::test]
async fn test_kitty_graphics_mode_toggle_removed() {
    let backend = TestBackend::new(120, 40);
    let mut terminal = Terminal::new(backend).unwrap();
    let (tx, _rx) = unbounded_channel();

    let mut app = App::new(None, None, true);

    // Render Canvas tab
    terminal.draw(|f| app.render(f)).unwrap();
    assert!(!buffer_contains(&terminal, "[g] Graphics"));
    assert!(!buffer_contains(&terminal, "[Kitty: ON]"));
    assert!(!buffer_contains(&terminal, "[Braille Mode]"));
    assert!(!buffer_contains(&terminal, "[KITTY HI-RES ACTIVE]"));

    // Press 'g'
    app.handle_key(key_event(KeyCode::Char('g')), &tx);
    terminal.draw(|f| app.render(f)).unwrap();
    assert!(!buffer_contains(&terminal, "[Kitty: ON]"));
    assert!(!buffer_contains(&terminal, "[KITTY HI-RES ACTIVE]"));

    // Press 'G'
    app.handle_key(key_event_mod(KeyCode::Char('G'), KeyModifiers::SHIFT), &tx);
    terminal.draw(|f| app.render(f)).unwrap();
    assert!(!buffer_contains(&terminal, "[Kitty: ON]"));
    assert!(!buffer_contains(&terminal, "[KITTY HI-RES ACTIVE]"));

    // Help popup does not mention Graphics mode
    app.handle_key(key_event(KeyCode::Char('?')), &tx);
    assert!(app.show_help);
    terminal.draw(|f| app.render(f)).unwrap();
    assert!(!buffer_contains(&terminal, "Toggle high-res graphics"));
}

#[test]
fn test_cli_kitty_flag_rejected() {
    let bin_path = env!("CARGO_BIN_EXE_clab-tui");
    let output = Command::new(bin_path)
        .arg("--kitty")
        .output()
        .expect("Failed to execute binary");

    assert!(
        !output.status.success(),
        "--kitty flag must be rejected by CLI parser"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("unexpected argument '--kitty'"),
        "Expected error message about unknown --kitty argument, got: {}",
        stderr
    );
}

#[tokio::test]
async fn test_canvas_panning_keyboard_all_modes() {
    let (tx, _rx) = unbounded_channel();
    let mut app = App::new(None, None, true);
    app.active_tab = ActiveTab::Canvas;

    let init_x = app.canvas.offset_x;
    let init_y = app.canvas.offset_y;

    // 1. Shift + Arrows
    app.handle_key(key_event_mod(KeyCode::Left, KeyModifiers::SHIFT), &tx);
    assert_eq!(app.canvas.offset_x, init_x + 4.0);

    app.handle_key(key_event_mod(KeyCode::Right, KeyModifiers::SHIFT), &tx);
    assert_eq!(app.canvas.offset_x, init_x);

    app.handle_key(key_event_mod(KeyCode::Up, KeyModifiers::SHIFT), &tx);
    assert_eq!(app.canvas.offset_y, init_y + 4.0);

    app.handle_key(key_event_mod(KeyCode::Down, KeyModifiers::SHIFT), &tx);
    assert_eq!(app.canvas.offset_y, init_y);

    // 2. Ctrl + Arrows
    app.handle_key(key_event_mod(KeyCode::Left, KeyModifiers::CONTROL), &tx);
    assert_eq!(app.canvas.offset_x, init_x + 4.0);

    app.handle_key(key_event_mod(KeyCode::Right, KeyModifiers::CONTROL), &tx);
    assert_eq!(app.canvas.offset_x, init_x);

    app.handle_key(key_event_mod(KeyCode::Up, KeyModifiers::CONTROL), &tx);
    assert_eq!(app.canvas.offset_y, init_y + 4.0);

    app.handle_key(key_event_mod(KeyCode::Down, KeyModifiers::CONTROL), &tx);
    assert_eq!(app.canvas.offset_y, init_y);

    // 3. Uppercase vim keys H, J, K, L
    app.handle_key(key_event(KeyCode::Char('H')), &tx);
    assert_eq!(app.canvas.offset_x, init_x + 4.0);

    app.handle_key(key_event(KeyCode::Char('L')), &tx);
    assert_eq!(app.canvas.offset_x, init_x);

    app.handle_key(key_event(KeyCode::Char('K')), &tx);
    assert_eq!(app.canvas.offset_y, init_y + 4.0);

    app.handle_key(key_event(KeyCode::Char('J')), &tx);
    assert_eq!(app.canvas.offset_y, init_y);

    // 4. Plain arrows when no node is selected
    app.canvas.selected_node = None;
    app.handle_key(key_event(KeyCode::Left), &tx);
    assert_eq!(app.canvas.offset_x, init_x + 4.0);

    app.handle_key(key_event(KeyCode::Right), &tx);
    assert_eq!(app.canvas.offset_x, init_x);

    app.handle_key(key_event(KeyCode::Up), &tx);
    assert_eq!(app.canvas.offset_y, init_y + 4.0);

    app.handle_key(key_event(KeyCode::Down), &tx);
    assert_eq!(app.canvas.offset_y, init_y);

    // 5. Lowercase h/j/k/l when no node is selected
    app.handle_key(key_event(KeyCode::Char('h')), &tx);
    assert_eq!(app.canvas.offset_x, init_x + 4.0);

    app.handle_key(key_event(KeyCode::Char('l')), &tx);
    assert_eq!(app.canvas.offset_x, init_x);

    app.handle_key(key_event(KeyCode::Char('k')), &tx);
    assert_eq!(app.canvas.offset_y, init_y + 4.0);

    app.handle_key(key_event(KeyCode::Char('j')), &tx);
    assert_eq!(app.canvas.offset_y, init_y);

    // 6. When a node IS selected: plain arrow moves node, but Shift+Arrow pans canvas
    app.canvas.selected_node = Some("srl1".to_string());
    let (node_orig_x, _) = {
        let n = app.canvas.nodes.get("srl1").unwrap();
        (n.x, n.y)
    };

    // Plain Left moves node
    app.handle_key(key_event(KeyCode::Left), &tx);
    let (node_new_x, _) = {
        let n = app.canvas.nodes.get("srl1").unwrap();
        (n.x, n.y)
    };
    assert_ne!(
        node_new_x, node_orig_x,
        "Plain Left must move selected node position"
    );
    assert_eq!(
        app.canvas.offset_x, init_x,
        "Canvas offset must remain unchanged"
    );

    // Shift + Left pans canvas, does NOT move node
    app.handle_key(key_event_mod(KeyCode::Left, KeyModifiers::SHIFT), &tx);
    assert_eq!(
        app.canvas.offset_x,
        init_x + 4.0,
        "Shift + Left must pan canvas offset"
    );
    let (node_pos_after_pan_x, _) = {
        let n = app.canvas.nodes.get("srl1").unwrap();
        (n.x, n.y)
    };
    assert_eq!(
        node_pos_after_pan_x, node_new_x,
        "Node position must not change during canvas pan"
    );
}

#[test]
fn test_canvas_panning_mouse_dragging() {
    let mut app = App::new(None, None, true);
    app.active_tab = ActiveTab::Canvas;

    // Set last_area so canvas_area() knows the bounds
    app.last_area.set(Rect::new(0, 0, 100, 40));

    // MouseDown on empty canvas location (e.g. column 80, row 30)
    let down_event = MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: 80,
        row: 30,
        modifiers: KeyModifiers::NONE,
    };
    app.handle_mouse(down_event);

    assert_eq!(
        app.canvas.mode,
        CanvasMode::PanningCanvas,
        "Click on empty canvas must enter PanningCanvas mode"
    );

    let start_offset_x = app.canvas.offset_x;
    let start_offset_y = app.canvas.offset_y;

    // MouseDrag by +10 in x and +5 in y
    let drag_event = MouseEvent {
        kind: MouseEventKind::Drag(MouseButton::Left),
        column: 90,
        row: 35,
        modifiers: KeyModifiers::NONE,
    };
    app.handle_mouse(drag_event);

    assert_eq!(app.canvas.offset_x, start_offset_x + 10.0);
    assert_eq!(app.canvas.offset_y, start_offset_y + 5.0);

    // MouseUp resets mode to Normal
    let up_event = MouseEvent {
        kind: MouseEventKind::Up(MouseButton::Left),
        column: 90,
        row: 35,
        modifiers: KeyModifiers::NONE,
    };
    app.handle_mouse(up_event);

    assert_eq!(
        app.canvas.mode,
        CanvasMode::Normal,
        "MouseUp must restore CanvasMode::Normal"
    );

    // Shift + Click over a node position must pan instead of dragging the node
    let (node_col, node_row) = {
        let n = app.canvas.nodes.get("srl1").unwrap();
        let (sx, sy) = app.canvas.canvas_to_screen(n.x + 4.0, n.y + 1.0);
        (sx as u16, sy as u16 + 3) // +3 for header + tabs
    };

    let shift_down = MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: node_col,
        row: node_row,
        modifiers: KeyModifiers::SHIFT,
    };
    app.handle_mouse(shift_down);

    assert_eq!(
        app.canvas.mode,
        CanvasMode::PanningCanvas,
        "Shift+Click on node must initiate PanningCanvas instead of DraggingNode"
    );
}

#[test]
fn test_parallel_links_badge_click_hit_test() {
    let mut canvas = CanvasState::new();

    let node_a = make_node("node_a", 10.0, 10.0, &["e1-1", "e1-2"]);
    canvas.nodes.insert(node_a.name.clone(), node_a);

    let node_b = make_node("node_b", 60.0, 10.0, &["e1-1", "e1-2"]);
    canvas.nodes.insert(node_b.name.clone(), node_b);

    canvas.add_link("node_a", "e1-1", "node_b", "e1-1");
    canvas.add_link("node_a", "e1-2", "node_b", "e1-2");
    canvas.recalculate_all_links();

    // Verify badge positions for both links
    let (b0_src, b0_tgt) = canvas
        .link_badge_positions(0)
        .expect("Link 0 must have badge positions");
    let (b1_src, b1_tgt) = canvas
        .link_badge_positions(1)
        .expect("Link 1 must have badge positions");

    // Click on Link 0's source badge
    let (c_src0_x, c_src0_y) = canvas.screen_to_canvas(b0_src.x as u16 + 1, b0_src.y as u16);
    canvas.handle_click(c_src0_x, c_src0_y);
    assert_eq!(
        canvas.selected_link,
        Some(0),
        "Clicking on Link 0 source badge must select Link 0"
    );

    // Click on Link 0's target badge
    let (c_tgt0_x, c_tgt0_y) = canvas.screen_to_canvas(b0_tgt.x as u16 + 1, b0_tgt.y as u16);
    canvas.handle_click(c_tgt0_x, c_tgt0_y);
    assert_eq!(
        canvas.selected_link,
        Some(0),
        "Clicking on Link 0 target badge must select Link 0"
    );

    // Click on Link 1's source badge
    let (c_src1_x, c_src1_y) = canvas.screen_to_canvas(b1_src.x as u16 + 1, b1_src.y as u16);
    canvas.handle_click(c_src1_x, c_src1_y);
    assert_eq!(
        canvas.selected_link,
        Some(1),
        "Clicking on Link 1 source badge must select Link 1"
    );

    // Click on Link 1's target badge
    let (c_tgt1_x, c_tgt1_y) = canvas.screen_to_canvas(b1_tgt.x as u16 + 1, b1_tgt.y as u16);
    canvas.handle_click(c_tgt1_x, c_tgt1_y);
    assert_eq!(
        canvas.selected_link,
        Some(1),
        "Clicking on Link 1 target badge must select Link 1"
    );
}

#[test]
fn test_parallel_links_closest_wire_hit_test() {
    let mut canvas = CanvasState::new();

    let node_a = make_node("node_a", 10.0, 10.0, &["e1-1", "e1-2"]);
    canvas.nodes.insert(node_a.name.clone(), node_a);

    let node_b = make_node("node_b", 60.0, 10.0, &["e1-1", "e1-2"]);
    canvas.nodes.insert(node_b.name.clone(), node_b);

    canvas.add_link("node_a", "e1-1", "node_b", "e1-1");
    canvas.add_link("node_a", "e1-2", "node_b", "e1-2");
    canvas.recalculate_all_links();

    let p0_y = canvas.links[0].waypoints[0].1;
    let p1_y = canvas.links[1].waypoints[0].1;
    let mid_x = 35.0; // Between node_a (right=28) and node_b (left=60)

    // Click closer to Link 0 (40% distance towards Link 1)
    let click_y_closer_to_0 = p0_y + (p1_y - p0_y) * 0.3;
    canvas.handle_click(mid_x, click_y_closer_to_0);
    assert_eq!(
        canvas.selected_link,
        Some(0),
        "Clicking closer to Link 0 wire must select Link 0"
    );

    // Click closer to Link 1 (70% distance towards Link 1)
    let click_y_closer_to_1 = p0_y + (p1_y - p0_y) * 0.7;
    canvas.handle_click(mid_x, click_y_closer_to_1);
    assert_eq!(
        canvas.selected_link,
        Some(1),
        "Clicking closer to Link 1 wire must select Link 1"
    );
}

#[test]
fn test_vertically_stacked_parallel_links_dense_bundle() {
    let mut canvas = CanvasState::new();

    // Vertically stacked nodes: node_a above node_b
    let node_a = make_node(
        "node_a",
        20.0,
        5.0,
        &["eth1", "eth2", "eth3", "eth4", "eth5"],
    );
    canvas.nodes.insert(node_a.name.clone(), node_a);

    let node_b = make_node(
        "node_b",
        20.0,
        25.0,
        &["eth1", "eth2", "eth3", "eth4", "eth5"],
    );
    canvas.nodes.insert(node_b.name.clone(), node_b);

    // Add 5 parallel links between them
    for i in 1..=5 {
        canvas.add_link(
            "node_a",
            &format!("eth{}", i),
            "node_b",
            &format!("eth{}", i),
        );
    }
    canvas.recalculate_all_links();

    assert_eq!(canvas.links.len(), 5);

    // 1. All 5 start points and end points on perimeter must be unique
    let mut start_pts: Vec<(f64, f64)> = Vec::new();
    let mut end_pts: Vec<(f64, f64)> = Vec::new();
    for link in &canvas.links {
        let s = link.waypoints[0];
        let e = link.waypoints[link.waypoints.len() - 1];
        start_pts.push(s);
        end_pts.push(e);
    }

    for i in 0..5 {
        for j in (i + 1)..5 {
            assert_ne!(
                start_pts[i], start_pts[j],
                "Start points {} and {} must differ",
                i, j
            );
            assert_ne!(
                end_pts[i], end_pts[j],
                "End points {} and {} must differ",
                i, j
            );
        }
    }

    // 2. All 5 badge positions must be valid
    let mut badge_src_positions = Vec::new();
    for i in 0..5 {
        let (src_b, _) = canvas.link_badge_positions(i).unwrap();
        badge_src_positions.push(src_b);
    }
    // Verify each link has a distinct badge position
    for i in 0..5 {
        for j in (i + 1)..5 {
            assert_ne!(
                badge_src_positions[i], badge_src_positions[j],
                "Badge positions {} and {} must differ",
                i, j
            );
        }
    }

    // 3. Hit testing selects each of the 5 links when clicking near its wire
    for i in 0..5 {
        let mid_x = (start_pts[i].0 + end_pts[i].0) / 2.0;
        let mid_y = (start_pts[i].1 + end_pts[i].1) / 2.0;
        canvas.handle_click(mid_x, mid_y);
        assert_eq!(
            canvas.selected_link,
            Some(i),
            "Clicking link {} at ({}, {}) should select link {}",
            i,
            mid_x,
            mid_y,
            i
        );
    }
}

#[test]
fn test_canvas_panning_middle_mouse_button() {
    let mut app = App::new(None, None, true);
    app.active_tab = ActiveTab::Canvas;
    app.last_area.set(Rect::new(0, 0, 100, 40));

    let init_offset_x = app.canvas.offset_x;
    let init_offset_y = app.canvas.offset_y;

    // Middle MouseDown
    let down_event = MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Middle),
        column: 50,
        row: 20,
        modifiers: KeyModifiers::NONE,
    };
    app.handle_mouse(down_event);
    assert_eq!(
        app.canvas.mode,
        CanvasMode::PanningCanvas,
        "Middle click must enter PanningCanvas mode"
    );

    // Middle Drag
    let drag_event = MouseEvent {
        kind: MouseEventKind::Drag(MouseButton::Middle),
        column: 65,
        row: 28,
        modifiers: KeyModifiers::NONE,
    };
    app.handle_mouse(drag_event);
    assert_eq!(app.canvas.offset_x, init_offset_x + 15.0);
    assert_eq!(app.canvas.offset_y, init_offset_y + 8.0);

    // Middle MouseUp
    let up_event = MouseEvent {
        kind: MouseEventKind::Up(MouseButton::Middle),
        column: 65,
        row: 28,
        modifiers: KeyModifiers::NONE,
    };
    app.handle_mouse(up_event);
    assert_eq!(
        app.canvas.mode,
        CanvasMode::Normal,
        "Middle button release must restore Normal mode"
    );
}

#[tokio::test]
async fn test_canvas_ctrl_s_saves_topology() {
    let temp_dir = tempfile::tempdir().expect("Failed to create tempdir");
    let save_path = temp_dir.path().join("saved_topo.clab.yml");
    let sample = TopologyParser::create_sample_topology();
    TopologyParser::save_file(&sample, &save_path).unwrap();

    let mut app = App::new(Some(sample), Some(save_path.clone()), true);
    app.active_tab = ActiveTab::Canvas;
    let (tx, _rx) = unbounded_channel();

    // Modify a node coordinate
    if let Some(node) = app.canvas.nodes.get_mut("srl1") {
        node.x += 10.0;
    }
    app.sync_canvas_to_model();

    // Trigger Ctrl+S on canvas
    let ctrl_s = key_event_mod(KeyCode::Char('s'), KeyModifiers::CONTROL);
    app.handle_key(ctrl_s, &tx);

    // Verify file on disk was updated
    let reloaded = TopologyParser::parse_file(&save_path).expect("Failed to re-parse");
    let srl1_node = reloaded.topology.nodes.get("srl1").expect("srl1 exists");
    let x_label = srl1_node
        .labels
        .as_ref()
        .expect("labels exist")
        .get("clab-tui-x")
        .expect("x label exists");
    assert_eq!(x_label, "25.0");
}

