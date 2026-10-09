use clab_tui::canvas::node::CanvasNode;
use clab_tui::canvas::state::CanvasState;
use clab_tui::canvas::Direction;
use clab_tui::clab::mock::MockClabClient;
use clab_tui::clab::model::{LinkDefinition, NodeDefinition, KIND_TEMPLATES};
use clab_tui::clab::pattern::InterfacePattern;
use clab_tui::graphics::kitty::{GraphicsProtocol, KittyGraphRenderer};
use clab_tui::ui::theme::Theme;
use clab_tui::ui::views::LinkEditField;
use clab_tui::App;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use ratatui::backend::TestBackend;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::Terminal;
use tokio::sync::mpsc::unbounded_channel;

fn key(code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, KeyModifiers::NONE)
}

fn create_test_node(name: &str, kind: &str, x: f64, y: f64) -> CanvasNode {
    let def = NodeDefinition {
        kind: Some(kind.to_string()),
        ..Default::default()
    };
    CanvasNode::new(name, &def, x, y)
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

// ---------------------------------------------------------------------------
// 1. Interface Pattern Parsing and Validation Tests
// ---------------------------------------------------------------------------

#[test]
fn test_interface_pattern_parsing_unbounded() {
    let pat = InterfacePattern::parse("eth{n}");
    assert_eq!(pat.sets.len(), 1);
    assert_eq!(pat.sets[0].prefix, "eth");
    assert_eq!(pat.sets[0].suffix, "");
    assert_eq!(pat.sets[0].start_index, 1);
    assert_eq!(pat.sets[0].end_index, None);

    let pat = InterfacePattern::parse("e1-{n:1}");
    assert_eq!(pat.sets.len(), 1);
    assert_eq!(pat.sets[0].prefix, "e1-");
    assert_eq!(pat.sets[0].suffix, "");
    assert_eq!(pat.sets[0].start_index, 1);
    assert_eq!(pat.sets[0].end_index, None);

    let pat = InterfacePattern::parse("GigabitEthernet{n:2}");
    assert_eq!(pat.sets.len(), 1);
    assert_eq!(pat.sets[0].prefix, "GigabitEthernet");
    assert_eq!(pat.sets[0].suffix, "");
    assert_eq!(pat.sets[0].start_index, 2);
    assert_eq!(pat.sets[0].end_index, None);
}

#[test]
fn test_interface_pattern_parsing_bounded_and_multi_set() {
    let pat = InterfacePattern::parse("ethernet-1/{n:1-24}");
    assert_eq!(pat.sets.len(), 1);
    assert_eq!(pat.sets[0].prefix, "ethernet-1/");
    assert_eq!(pat.sets[0].suffix, "");
    assert_eq!(pat.sets[0].start_index, 1);
    assert_eq!(pat.sets[0].end_index, Some(24));

    let pat = InterfacePattern::parse("Gi0/{n:1-4}/0");
    assert_eq!(pat.sets.len(), 1);
    assert_eq!(pat.sets[0].prefix, "Gi0/");
    assert_eq!(pat.sets[0].suffix, "/0");
    assert_eq!(pat.sets[0].start_index, 1);
    assert_eq!(pat.sets[0].end_index, Some(4));

    let pat = InterfacePattern::parse("ethernet-1/{n:1-2},ethernet-2/{n:1-2}");
    assert_eq!(pat.sets.len(), 2);
    assert_eq!(pat.sets[0].prefix, "ethernet-1/");
    assert_eq!(pat.sets[0].start_index, 1);
    assert_eq!(pat.sets[0].end_index, Some(2));
    assert_eq!(pat.sets[1].prefix, "ethernet-2/");
    assert_eq!(pat.sets[1].start_index, 1);
    assert_eq!(pat.sets[1].end_index, Some(2));
}

#[test]
fn test_interface_pattern_allocation_sequence_and_spillover() {
    let pat = InterfacePattern::parse("ethernet-1/{n:1-2},ethernet-2/{n:1-2}");
    let mut used: Vec<String> = Vec::new();

    // 1st allocation
    let used_refs: Vec<&str> = used.iter().map(|s| s.as_str()).collect();
    let iface1 = pat.allocate_next(&used_refs);
    assert_eq!(iface1, "ethernet-1/1");
    used.push(iface1);

    // 2nd allocation
    let used_refs: Vec<&str> = used.iter().map(|s| s.as_str()).collect();
    let iface2 = pat.allocate_next(&used_refs);
    assert_eq!(iface2, "ethernet-1/2");
    used.push(iface2);

    // 3rd allocation spills over to ethernet-2
    let used_refs: Vec<&str> = used.iter().map(|s| s.as_str()).collect();
    let iface3 = pat.allocate_next(&used_refs);
    assert_eq!(iface3, "ethernet-2/1");
    used.push(iface3);

    // 4th allocation
    let used_refs: Vec<&str> = used.iter().map(|s| s.as_str()).collect();
    let iface4 = pat.allocate_next(&used_refs);
    assert_eq!(iface4, "ethernet-2/2");
    used.push(iface4);

    // 5th allocation spills past final set
    let used_refs: Vec<&str> = used.iter().map(|s| s.as_str()).collect();
    let iface5 = pat.allocate_next(&used_refs);
    assert_eq!(iface5, "ethernet-2/3");
    used.push(iface5);

    let used_refs: Vec<&str> = used.iter().map(|s| s.as_str()).collect();
    let iface6 = pat.allocate_next(&used_refs);
    assert_eq!(iface6, "ethernet-2/4");
}

#[test]
fn test_interface_pattern_validation() {
    // Valid patterns
    assert!(InterfacePattern::validate("eth{n}").is_ok());
    assert!(InterfacePattern::validate("e1-{n:1}").is_ok());
    assert!(InterfacePattern::validate("ethernet-1/{n:1-48},ethernet-2/{n:1-48}").is_ok());
    assert!(InterfacePattern::validate("Gi0/{n}/1").is_ok());
    assert!(InterfacePattern::validate("port_{n:0-10}").is_ok());

    // Invalid patterns
    assert!(InterfacePattern::validate("").is_err());
    assert!(InterfacePattern::validate("eth{n").is_err()); // missing }
    assert!(InterfacePattern::validate("eth{n:10-5}").is_err()); // start > end
    assert!(InterfacePattern::validate("eth{n:foo}").is_err()); // non-numeric
    assert!(InterfacePattern::validate("eth{n}extra{n}").is_err()); // multiple braces in set
    assert!(InterfacePattern::validate("eth@{n}").is_err()); // invalid character '@'
    assert!(InterfacePattern::validate("eth {n:1}").is_err()); // whitespace in name
    assert!(InterfacePattern::validate("eth#{n:1}").is_err()); // invalid symbol '#'
}

#[test]
fn test_default_interface_patterns_in_kind_templates() {
    for tmpl in KIND_TEMPLATES.iter() {
        assert!(
            !tmpl.interface_pattern.is_empty(),
            "Kind template for {} has empty interface pattern",
            tmpl.kind_name
        );
        assert!(
            InterfacePattern::validate(tmpl.interface_pattern).is_ok(),
            "Kind template for {} has invalid interface pattern: {}",
            tmpl.kind_name,
            tmpl.interface_pattern
        );
    }
}

// ---------------------------------------------------------------------------
// 2. CanvasState Interface Allocation and Link Auto-Assignment
// ---------------------------------------------------------------------------

#[test]
fn test_canvas_state_interface_allocation() {
    let mut state = CanvasState::new();
    state.nodes.clear();
    state.links.clear();

    let mut node1 = create_test_node("leaf1", "srl", 10.0, 10.0);
    node1.interface_pattern = "e1-{n:1}".to_string();
    state.nodes.insert(node1.name.clone(), node1);

    let mut node2 = create_test_node("spine1", "ceos", 50.0, 10.0);
    node2.interface_pattern = "eth{n:1}".to_string();
    state.nodes.insert(node2.name.clone(), node2);

    // Initial allocations
    assert_eq!(state.allocate_next_interface("leaf1"), "e1-1");
    assert_eq!(state.allocate_next_interface("spine1"), "eth1");

    // Add a link leaf1:e1-1 <-> spine1:eth1
    state.add_link("leaf1", "e1-1", "spine1", "eth1");

    // Subsequent allocations should advance to the next index
    assert_eq!(state.allocate_next_interface("leaf1"), "e1-2");
    assert_eq!(state.allocate_next_interface("spine1"), "eth2");
}

#[test]
fn test_canvas_state_spillover_interface_allocation() {
    let mut state = CanvasState::new();
    state.nodes.clear();
    state.links.clear();

    let mut node = create_test_node("router1", "srl", 10.0, 10.0);
    node.interface_pattern = "eth1/{n:1-2},eth2/{n:1-2}".to_string();
    state.nodes.insert(node.name.clone(), node);

    let other = create_test_node("other", "linux", 50.0, 10.0);
    state.nodes.insert(other.name.clone(), other);

    assert_eq!(state.allocate_next_interface("router1"), "eth1/1");
    assert!(state.add_link("router1", "eth1/1", "other", "p1"));

    assert_eq!(state.allocate_next_interface("router1"), "eth1/2");
    assert!(state.add_link("router1", "eth1/2", "other", "p2"));

    // Spillover to second set
    assert_eq!(state.allocate_next_interface("router1"), "eth2/1");
    assert!(state.add_link("router1", "eth2/1", "other", "p3"));

    assert_eq!(state.allocate_next_interface("router1"), "eth2/2");
    assert!(state.add_link("router1", "eth2/2", "other", "p4"));

    // Past both sets
    assert_eq!(state.allocate_next_interface("router1"), "eth2/3");
}

// ---------------------------------------------------------------------------
// 3. Dynamic Perimeter Connection Tests
// ---------------------------------------------------------------------------

#[test]
fn test_canvas_node_perimeter_connection_towards_cardinal_directions() {
    // Node centered at (59, 52.5): top-left (50, 50), width 18, height 5
    let node = create_test_node("n1", "srl", 50.0, 50.0);
    let cx = 50.0 + node.width / 2.0; // 59.0
    let cy = 50.0 + node.height / 2.0; // 52.5

    // Target directly to the East (x=100, y=cy)
    let (pt_e, dir_e) = node.perimeter_connection_towards(100.0, cy);
    assert_eq!(dir_e, Direction::East);
    assert!((pt_e.0 - 68.0).abs() < 1e-4);
    assert!((pt_e.1 - cy).abs() < 1e-4);

    // Target directly to the West (x=0, y=cy)
    let (pt_w, dir_w) = node.perimeter_connection_towards(0.0, cy);
    assert_eq!(dir_w, Direction::West);
    assert!((pt_w.0 - 50.0).abs() < 1e-4);
    assert!((pt_w.1 - cy).abs() < 1e-4);

    // Target directly to the South (x=cx, y=100)
    let (pt_s, dir_s) = node.perimeter_connection_towards(cx, 100.0);
    assert_eq!(dir_s, Direction::South);
    assert!((pt_s.0 - cx).abs() < 1e-4);
    assert!((pt_s.1 - 55.0).abs() < 1e-4);

    // Target directly to the North (x=cx, y=0)
    let (pt_n, dir_n) = node.perimeter_connection_towards(cx, 0.0);
    assert_eq!(dir_n, Direction::North);
    assert!((pt_n.0 - cx).abs() < 1e-4);
    assert!((pt_n.1 - 50.0).abs() < 1e-4);
}

#[test]
fn test_canvas_node_perimeter_connection_towards_diagonal() {
    let node = create_test_node("n1", "srl", 50.0, 50.0);

    // Target far Southeast at (100, 100)
    let (pt, dir) = node.perimeter_connection_towards(100.0, 100.0);
    assert_eq!(dir, Direction::South);
    assert!((pt.1 - (node.y + node.height)).abs() < 1e-4);
    assert!(pt.0 >= node.x && pt.0 <= node.x + node.width);
}

// ---------------------------------------------------------------------------
// 4. Link Selection and Interactive Editing
// ---------------------------------------------------------------------------

#[test]
fn test_link_selection_and_cycling() {
    let mut state = CanvasState::new();
    state.nodes.clear();
    state.links.clear();

    let n1 = create_test_node("n1", "srl", 10.0, 10.0);
    let n2 = create_test_node("n2", "srl", 30.0, 10.0);
    let n3 = create_test_node("n3", "srl", 50.0, 10.0);
    state.nodes.insert(n1.name.clone(), n1);
    state.nodes.insert(n2.name.clone(), n2);
    state.nodes.insert(n3.name.clone(), n3);

    state.add_link("n1", "e1-1", "n2", "e1-1");
    state.add_link("n2", "e1-2", "n3", "e1-1");
    state.add_link("n1", "e1-2", "n3", "e1-2");

    assert_eq!(state.selected_link, None);

    // select next wraps: None -> 0 -> 1 -> 2 -> 0
    state.select_next_link();
    assert_eq!(state.selected_link, Some(0));

    state.select_next_link();
    assert_eq!(state.selected_link, Some(1));

    state.select_next_link();
    assert_eq!(state.selected_link, Some(2));

    state.select_next_link();
    assert_eq!(state.selected_link, Some(0));

    // select prev wraps: 0 -> 2 -> 1 -> 0
    state.select_prev_link();
    assert_eq!(state.selected_link, Some(2));

    state.select_prev_link();
    assert_eq!(state.selected_link, Some(1));

    state.select_prev_link();
    assert_eq!(state.selected_link, Some(0));
}

#[test]
fn test_update_link_ports_validation_and_uniqueness() {
    let mut state = CanvasState::new();
    state.nodes.clear();
    state.links.clear();

    let n1 = create_test_node("n1", "srl", 10.0, 10.0);
    let n2 = create_test_node("n2", "srl", 30.0, 10.0);
    state.nodes.insert(n1.name.clone(), n1);
    state.nodes.insert(n2.name.clone(), n2);

    state.add_link("n1", "e1-1", "n2", "e1-1");
    state.add_link("n1", "e1-2", "n2", "e1-2");

    // Valid update
    let res = state.update_link_ports(0, "e1-10", "e1-10");
    assert!(res.is_ok());
    assert_eq!(state.links[0].source_port, "e1-10");
    assert_eq!(state.links[0].target_port, "e1-10");

    // Invalid port name: empty
    let res_empty = state.update_link_ports(0, "", "e1-10");
    assert!(res_empty.is_err());
    assert_eq!(state.links[0].source_port, "e1-10"); // unchanged

    // Invalid port name: illegal character
    let res_char = state.update_link_ports(0, "e1-10!", "e1-10");
    assert!(res_char.is_err());

    // Invalid port name: leading hyphen
    let res_hyphen = state.update_link_ports(0, "-e1-10", "e1-10");
    assert!(res_hyphen.is_err());

    // Duplicate link collision: updating link 1 to match link 0's endpoints
    let res_dup = state.update_link_ports(1, "e1-10", "e1-10");
    assert!(res_dup.is_err());
    assert!(res_dup.unwrap_err().contains("Duplicate link"));
}

#[test]
fn test_link_edit_modal_app_workflow() {
    let (tx, _rx) = unbounded_channel();
    let mut app = App::new(None, None, true);

    // Create a topology with 2 nodes and 1 link
    app.topology.topology.nodes.clear();
    app.topology.topology.links.clear();
    app.topology
        .topology
        .nodes
        .insert("leaf1".to_string(), NodeDefinition::default());
    app.topology
        .topology
        .nodes
        .insert("spine1".to_string(), NodeDefinition::default());
    app.topology.topology.links.push(LinkDefinition {
        endpoints: ["leaf1:e1-1".to_string(), "spine1:e1-1".to_string()],
        labels: None,
        vars: None,
    });
    let topo = app.topology.clone();
    app.canvas.load_from_topology(&topo);

    assert_eq!(app.canvas.links.len(), 1);
    app.canvas.selected_link = Some(0);

    // Press 'e' on selected link to open LinkEditModal
    app.handle_key(key(KeyCode::Char('e')), &tx);
    assert!(app.link_edit_modal.is_open);
    assert_eq!(app.link_edit_modal.source_port, "e1-1");
    assert_eq!(app.link_edit_modal.target_port, "e1-1");
    assert_eq!(app.link_edit_modal.focused_field, LinkEditField::SourcePort);

    // Type "0" at end of source port -> "e1-10"
    app.handle_key(key(KeyCode::Char('0')), &tx);
    assert_eq!(app.link_edit_modal.source_port, "e1-10");

    // Tab to target port
    app.handle_key(key(KeyCode::Tab), &tx);
    assert_eq!(app.link_edit_modal.focused_field, LinkEditField::TargetPort);

    // Type "0" at end of target port -> "e1-10"
    app.handle_key(key(KeyCode::Char('0')), &tx);
    assert_eq!(app.link_edit_modal.target_port, "e1-10");

    // Press Enter to save
    app.handle_key(key(KeyCode::Enter), &tx);
    assert!(!app.link_edit_modal.is_open);

    // Check canvas link and topology endpoints updated
    assert_eq!(app.canvas.links[0].source_port, "e1-10");
    assert_eq!(app.canvas.links[0].target_port, "e1-10");
    assert_eq!(
        app.topology.topology.links[0].endpoints,
        ["leaf1:e1-10".to_string(), "spine1:e1-10".to_string()]
    );

    // Press 'e' again, change, then cancel with Esc
    app.handle_key(key(KeyCode::Char('e')), &tx);
    assert!(app.link_edit_modal.is_open);
    app.handle_key(key(KeyCode::Char('9')), &tx);
    app.handle_key(key(KeyCode::Esc), &tx);
    assert!(!app.link_edit_modal.is_open);
    assert_eq!(app.canvas.links[0].source_port, "e1-10"); // Unchanged
}

// ---------------------------------------------------------------------------
// 5. Deploy Flag and Mock Client Consistency Tests
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_deploy_modal_cleanup_flag_label() {
    let (tx, _rx) = unbounded_channel();
    let mut app = App::new(None, None, true);

    // Press 'd' to open deploy prompt modal
    app.handle_key(key(KeyCode::Char('d')), &tx);
    assert!(app.confirm_modal.is_open);
    assert_eq!(app.confirm_modal.action_name, "deploy");
    assert!(app.confirm_modal.show_cleanup_toggle);

    // Render to backend to verify the label contains "Cleanup (-c)"
    let backend = TestBackend::new(80, 24);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal.draw(|f| app.render(f)).unwrap();

    assert!(buffer_contains(&terminal, "Cleanup (-c)"));
}

#[tokio::test]
async fn test_mock_deploy_cleanup_and_reconfigure_dispatch() {
    let mock = MockClabClient::new();
    let topo = clab_tui::clab::parser::TopologyParser::create_sample_topology();
    let (log_tx, mut log_rx) = unbounded_channel();

    let res = mock.simulate_deploy(&topo, false, true, log_tx).await;
    assert!(res.is_ok());

    let mut logs = Vec::new();
    while let Ok(line) = log_rx.try_recv() {
        logs.push(line);
    }
    assert!(logs
        .iter()
        .any(|l| l.contains("Removing previous lab artifacts")));
}

// ---------------------------------------------------------------------------
// 6. High-Res Graphics Buffer Rasterization and Stability
// ---------------------------------------------------------------------------

#[test]
fn test_kitty_render_to_buffer_non_blocking_and_safe() {
    let mut state = CanvasState::new();
    state.nodes.clear();
    state.links.clear();

    let node1 = create_test_node("leaf1", "srl", 20.0, 15.0);
    let node2 = create_test_node("spine1", "ceos", 60.0, 15.0);
    state.nodes.insert(node1.name.clone(), node1);
    state.nodes.insert(node2.name.clone(), node2);
    state.add_link("leaf1", "e1-1", "spine1", "eth1");

    let area = Rect::new(0, 0, 80, 30);
    let mut buf = Buffer::empty(area);
    let theme = Theme::default();

    // Test render_with_protocol does not hang or panic
    KittyGraphRenderer::render_with_protocol(
        &state,
        area,
        &mut buf,
        &theme,
        GraphicsProtocol::Kitty,
    );

    // Extreme zoom and pan coordinates safety test
    state.zoom = 50.0;
    state.offset_x = -1000.0;
    state.offset_y = 500.0;
    KittyGraphRenderer::render_with_protocol(
        &state,
        area,
        &mut buf,
        &theme,
        GraphicsProtocol::Kitty,
    );

    state.zoom = 0.05;
    state.offset_x = 10000.0;
    state.offset_y = -10000.0;
    KittyGraphRenderer::render_with_protocol(
        &state,
        area,
        &mut buf,
        &theme,
        GraphicsProtocol::Kitty,
    );
}

#[test]
fn test_update_link_ports_single_node_port_collision_and_self_loop() {
    let mut state = CanvasState::new();
    state.nodes.clear();
    state.links.clear();

    let n1 = create_test_node("n1", "srl", 10.0, 10.0);
    let n2 = create_test_node("n2", "srl", 30.0, 10.0);
    let n3 = create_test_node("n3", "srl", 50.0, 10.0);
    state.nodes.insert(n1.name.clone(), n1);
    state.nodes.insert(n2.name.clone(), n2);
    state.nodes.insert(n3.name.clone(), n3);

    // Link 0: n1:e1-1 <-> n2:e1-1
    // Link 1: n1:e1-2 <-> n3:e1-1
    state.add_link("n1", "e1-1", "n2", "e1-1");
    state.add_link("n1", "e1-2", "n3", "e1-1");

    // Attempting to update link 1 so n1 uses "e1-1" (which is already bound to link 0)
    let res = state.update_link_ports(1, "e1-1", "e1-2");
    assert!(res.is_err());
    assert!(res.unwrap_err().contains("already in use on node 'n1'"));

    // Attempting to update link 1 so n3 uses "e1-1" (already used on n3 by link 1 itself) -> ok since it's the same link
    let res_ok = state.update_link_ports(1, "e1-5", "e1-1");
    assert!(res_ok.is_ok());

    // Attempting same-node self-loop
    state.links[1].source_node = "n1".to_string();
    state.links[1].target_node = "n1".to_string();
    let res_self = state.update_link_ports(1, "e1-9", "e1-9");
    assert!(res_self.is_err());
    assert!(res_self
        .unwrap_err()
        .contains("Cannot connect an interface to itself"));
}

#[test]
fn test_accurate_diagonal_link_hit_testing() {
    let mut state = CanvasState::new();
    state.nodes.clear();
    state.links.clear();

    // Link with a diagonal segment from (10.0, 10.0) to (50.0, 50.0)
    state.links.push(clab_tui::canvas::link::CanvasLink {
        source_node: "n1".to_string(),
        source_port: "p1".to_string(),
        target_node: "n2".to_string(),
        target_port: "p2".to_string(),
        waypoints: vec![(10.0, 10.0), (50.0, 50.0)],
        routing_style: clab_tui::canvas::link::RoutingStyle::Direct,
    });

    // Click near the diagonal line at (30.0, 30.5) -> should select the link
    state.handle_click(30.0, 30.5);
    assert_eq!(state.selected_link, Some(0));

    // Deselect
    state.selected_link = None;

    // Click in the bounding box corner far from the diagonal at (12.0, 48.0) -> should NOT select
    state.handle_click(12.0, 48.0);
    assert_eq!(state.selected_link, None);
}

#[test]
fn test_link_edit_modal_mouse_interaction() {
    let mut app = App::new(None, None, true);
    let area = Rect::new(0, 0, 100, 30);
    app.last_area.set(area);

    // Open modal for link 0
    let link = app.canvas.links[0].clone();
    app.link_edit_modal.open_for_link(
        0,
        &link.source_node,
        &link.source_port,
        &link.target_node,
        &link.target_port,
    );
    assert!(app.link_edit_modal.is_open);
    assert_eq!(app.link_edit_modal.focused_field, LinkEditField::SourcePort);

    let modal_width = 64.min(area.width.saturating_sub(4));
    let modal_height = 14.min(area.height.saturating_sub(2));
    let modal_x = area.x + (area.width.saturating_sub(modal_width)) / 2;
    let modal_y = area.y + (area.height.saturating_sub(modal_height)) / 2;

    // Click on target port row (modal_y + 6)
    app.handle_mouse(MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: modal_x + 10,
        row: modal_y + 6,
        modifiers: KeyModifiers::NONE,
    });
    assert_eq!(app.link_edit_modal.focused_field, LinkEditField::TargetPort);

    // Click on source port row (modal_y + 4)
    app.handle_mouse(MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: modal_x + 10,
        row: modal_y + 4,
        modifiers: KeyModifiers::NONE,
    });
    assert_eq!(app.link_edit_modal.focused_field, LinkEditField::SourcePort);

    // Click on Cancel button (modal_y + modal_height - 3, col modal_x + 32)
    app.handle_mouse(MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: modal_x + 32,
        row: modal_y + modal_height - 3,
        modifiers: KeyModifiers::NONE,
    });
    assert!(!app.link_edit_modal.is_open);
}

#[tokio::test]
async fn test_mouse_wiring_drop_on_node_card_auto_allocates() {
    let mut app = App::new(None, None, true);
    let area = Rect::new(0, 0, 120, 40);
    app.last_area.set(area);
    let initial_links = app.canvas.links.len();

    // Start wiring from srl1
    app.canvas.selected_node = Some("srl1".to_string());
    assert!(app.canvas.start_wiring());
    assert_eq!(app.canvas.mode, clab_tui::canvas::state::CanvasMode::Wiring);

    // Target node is host2 at (55.0, 25.0)
    let (tgt_sx, tgt_sy) = app.canvas.canvas_to_screen(60.0, 27.0);
    let body = app.canvas_area();
    let col = body.x + tgt_sx as u16;
    let row = body.y + tgt_sy as u16;

    // Release mouse over host2 card
    app.handle_mouse(MouseEvent {
        kind: MouseEventKind::Up(MouseButton::Left),
        column: col,
        row,
        modifiers: KeyModifiers::NONE,
    });

    assert_eq!(app.canvas.links.len(), initial_links + 1);
    let new_link = app.canvas.links.last().unwrap();
    assert_eq!(new_link.source_node, "srl1");
    assert_eq!(new_link.target_node, "host2");
    // Verify auto-allocated interface patterns
    assert!(!new_link.source_port.is_empty());
    assert!(!new_link.target_port.is_empty());
}
