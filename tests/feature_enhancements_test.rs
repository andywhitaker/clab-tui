use clab_tui::app::App;
use clab_tui::canvas::state::CanvasMode;
use clab_tui::clab::model::{LinkDefinition, NodeCategory, NodeProfile};
use clab_tui::clab::parser::TopologyParser;
use clab_tui::ui::layout::ActiveTab;
use clab_tui::ui::views::canvas_view::ProfileField;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::backend::TestBackend;
use ratatui::Terminal;
use tokio::sync::mpsc::unbounded_channel;

fn key_event(code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, KeyModifiers::NONE)
}

fn ctrl_key_event(code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, KeyModifiers::CONTROL)
}

fn buffer_contains(terminal: &Terminal<TestBackend>, needle: &str) -> bool {
    let buf = terminal.backend().buffer();
    let content = format!("{:?}", buf);
    content.contains(needle)
}

#[tokio::test]
async fn test_ascii_mode_straight_lines_render() {
    let backend = TestBackend::new(120, 40);
    let mut terminal = Terminal::new(backend).unwrap();

    let app = App::new(None, None, true);

    // Initial render in ASCII/Unicode canvas mode
    terminal.draw(|f| app.render(f)).unwrap();

    // Verify straight line characters and corners exist in render
    assert!(buffer_contains(&terminal, "─") || buffer_contains(&terminal, "│"));
    assert!(buffer_contains(&terminal, "srl1"));
    assert!(buffer_contains(&terminal, "srl2"));

    // Verify that broken dashed lines are not used for links
    assert!(!buffer_contains(&terminal, "╌"));
}

#[tokio::test]
async fn test_ascii_mode_wiring_straight_lines() {
    let backend = TestBackend::new(120, 40);
    let mut terminal = Terminal::new(backend).unwrap();
    let (tx, _rx) = unbounded_channel();

    let mut app = App::new(None, None, true);
    app.canvas.selected_node = Some("srl1".to_string());

    // Enter wiring mode
    app.handle_key(key_event(KeyCode::Char('w')), &tx);
    assert_eq!(app.canvas.mode, CanvasMode::Wiring);

    // Set wiring cursor target
    app.canvas.wiring_cursor = (50.0, 20.0);

    terminal.draw(|f| app.render(f)).unwrap();

    // Wiring rubber band renders solid endpoint marker ● and straight line segments
    assert!(buffer_contains(&terminal, "●"));
    assert!(buffer_contains(&terminal, "─") || buffer_contains(&terminal, "│"));
    // Dashed broken line ╌ is eliminated
    assert!(!buffer_contains(&terminal, "╌"));
}

#[tokio::test]
async fn test_yaml_viewer_edit_mode_toggle_and_navigation() {
    let (tx, _rx) = unbounded_channel();
    let mut app = App::new(None, None, true);

    // Switch to YAML tab
    app.handle_key(key_event(KeyCode::Char('4')), &tx);
    assert_eq!(app.active_tab, ActiveTab::Yaml);
    assert!(!app.yaml_state.is_editing);

    // Toggle into edit mode with 'e'
    app.handle_key(key_event(KeyCode::Char('e')), &tx);
    assert!(app.yaml_state.is_editing);

    // Initial cursor position
    assert_eq!(app.yaml_state.cursor_row, 0);
    assert_eq!(app.yaml_state.cursor_col, 0);

    // Navigate Down and Right
    app.handle_key(key_event(KeyCode::Down), &tx);
    assert_eq!(app.yaml_state.cursor_row, 1);

    app.handle_key(key_event(KeyCode::Right), &tx);
    assert_eq!(app.yaml_state.cursor_col, 1);

    // End and Home
    app.handle_key(key_event(KeyCode::End), &tx);
    let line_len = app.yaml_state.edit_lines[app.yaml_state.cursor_row]
        .chars()
        .count();
    assert_eq!(app.yaml_state.cursor_col, line_len);

    app.handle_key(key_event(KeyCode::Home), &tx);
    assert_eq!(app.yaml_state.cursor_col, 0);

    // Navigate Up
    app.handle_key(key_event(KeyCode::Up), &tx);
    assert_eq!(app.yaml_state.cursor_row, 0);

    // Exit edit mode with Esc
    app.handle_key(key_event(KeyCode::Esc), &tx);
    assert!(!app.yaml_state.is_editing);
}

#[tokio::test]
async fn test_yaml_viewer_text_editing_and_apply_success() {
    let (tx, _rx) = unbounded_channel();
    let mut app = App::new(None, None, true);

    // Switch to YAML tab
    app.handle_key(key_event(KeyCode::Char('4')), &tx);
    app.handle_key(key_event(KeyCode::Char('e')), &tx);
    assert!(app.yaml_state.is_editing);

    // Replace edit lines with a valid simple topology YAML
    let custom_yaml = r#"name: test-edited-lab
topology:
  nodes:
    r1:
      kind: srl
      image: ghcr.io/nokia/srlinux:latest
    r2:
      kind: srl
      image: ghcr.io/nokia/srlinux:latest
  links:
    - endpoints: ["r1:e1-1", "r2:e1-1"]
"#;

    app.yaml_state.edit_lines = custom_yaml.lines().map(|s| s.to_string()).collect();

    // Apply edits via Ctrl+S
    app.handle_key(ctrl_key_event(KeyCode::Char('s')), &tx);

    // Verify edit mode exited and topology updated
    assert!(!app.yaml_state.is_editing);
    assert_eq!(app.topology.name, "test-edited-lab");
    assert!(app.canvas.nodes.contains_key("r1"));
    assert!(app.canvas.nodes.contains_key("r2"));
    assert_eq!(app.canvas.links.len(), 1);
}

#[tokio::test]
async fn test_yaml_viewer_invalid_yaml_preserves_edits_and_shows_error() {
    let (tx, _rx) = unbounded_channel();
    let mut app = App::new(None, None, true);

    // Switch to YAML tab and start editing
    app.handle_key(key_event(KeyCode::Char('4')), &tx);
    app.handle_key(key_event(KeyCode::Char('e')), &tx);

    // Enter invalid YAML text
    let invalid_yaml = "name: broken_topology\ntopology: [invalid syntax ::";
    app.yaml_state.edit_lines = invalid_yaml.lines().map(|s| s.to_string()).collect();

    // Try applying edits with Ctrl+S
    app.handle_key(ctrl_key_event(KeyCode::Char('s')), &tx);

    // Verify edit mode is STILL active, error_msg is present, and user's edits are NOT lost
    assert!(app.yaml_state.is_editing);
    assert!(app.yaml_state.error_msg.is_some());
    assert_eq!(app.yaml_state.get_text(), invalid_yaml);
}

#[tokio::test]
async fn test_yaml_viewer_typing_backspace_and_newline() {
    let (tx, _rx) = unbounded_channel();
    let mut app = App::new(None, None, true);

    app.handle_key(key_event(KeyCode::Char('4')), &tx);
    app.handle_key(key_event(KeyCode::Char('e')), &tx);

    app.yaml_state.edit_lines = vec![String::new()];
    app.yaml_state.cursor_row = 0;
    app.yaml_state.cursor_col = 0;

    // Type "foo"
    app.handle_key(key_event(KeyCode::Char('f')), &tx);
    app.handle_key(key_event(KeyCode::Char('o')), &tx);
    app.handle_key(key_event(KeyCode::Char('o')), &tx);
    assert_eq!(app.yaml_state.edit_lines[0], "foo");
    assert_eq!(app.yaml_state.cursor_col, 3);

    // Backspace
    app.handle_key(key_event(KeyCode::Backspace), &tx);
    assert_eq!(app.yaml_state.edit_lines[0], "fo");
    assert_eq!(app.yaml_state.cursor_col, 2);

    // Enter (newline)
    app.handle_key(key_event(KeyCode::Enter), &tx);
    assert_eq!(app.yaml_state.edit_lines.len(), 2);
    assert_eq!(app.yaml_state.cursor_row, 1);
    assert_eq!(app.yaml_state.cursor_col, 0);

    // Tab (inserts 2 spaces)
    app.handle_key(key_event(KeyCode::Tab), &tx);
    assert_eq!(app.yaml_state.edit_lines[1], "  ");
    assert_eq!(app.yaml_state.cursor_col, 2);
}

#[tokio::test]
async fn test_node_profiles_default_and_add_custom_profile() {
    let (tx, _rx) = unbounded_channel();
    let mut app = App::new(None, None, true);
    let initial_count = app.node_profiles.len();
    assert!(initial_count >= 8);

    // Open Add Node Modal
    app.handle_key(key_event(KeyCode::Char('a')), &tx);
    assert!(app.add_modal.is_open);

    // Press 'n' to create a new profile
    app.handle_key(key_event(KeyCode::Char('n')), &tx);
    assert!(app.profile_modal.is_open);
    assert!(app.profile_modal.is_new);

    // Fill in profile modal fields
    app.profile_modal.kind_name = "vyos".to_string();
    app.profile_modal.display_name = "VyOS Router".to_string();
    app.profile_modal.image = "vyos/vyos:1.4".to_string();
    app.profile_modal.ports = "eth0, eth1, eth2, eth3".to_string();
    app.profile_modal.category = NodeCategory::Router;
    app.profile_modal.description = "VyOS Open Source Router".to_string();

    // Save profile via Enter on Save field
    app.profile_modal.focused_field = ProfileField::Save;
    app.handle_key(key_event(KeyCode::Enter), &tx);

    // Modal closed and profile added
    assert!(!app.profile_modal.is_open);
    assert_eq!(app.node_profiles.len(), initial_count + 1);

    let added = app.node_profiles.last().unwrap();
    assert_eq!(added.kind_name, "vyos");
    assert_eq!(added.display_name, "VyOS Router");
    assert_eq!(added.default_image, "vyos/vyos:1.4");
    assert_eq!(added.default_ports, vec!["eth0", "eth1", "eth2", "eth3"]);
    assert_eq!(added.node_category, NodeCategory::Router);
}

#[tokio::test]
async fn test_node_profiles_edit_existing_profile() {
    let (tx, _rx) = unbounded_channel();
    let mut app = App::new(None, None, true);

    // Open Add Node Modal
    app.handle_key(key_event(KeyCode::Char('a')), &tx);
    app.add_modal.selected_index = 0;

    // Press 'e' to edit selected profile
    app.handle_key(key_event(KeyCode::Char('e')), &tx);
    assert!(app.profile_modal.is_open);
    assert!(!app.profile_modal.is_new);
    assert_eq!(app.profile_modal.editing_index, Some(0));

    // Update display name and image
    app.profile_modal.display_name = "Custom SR Linux v2".to_string();
    app.profile_modal.image = "ghcr.io/nokia/srlinux:24.7.1".to_string();

    // Save with Ctrl+S
    app.handle_key(ctrl_key_event(KeyCode::Char('s')), &tx);

    assert!(!app.profile_modal.is_open);
    assert_eq!(app.node_profiles[0].display_name, "Custom SR Linux v2");
    assert_eq!(
        app.node_profiles[0].default_image,
        "ghcr.io/nokia/srlinux:24.7.1"
    );
}

#[tokio::test]
async fn test_node_profiles_delete_profile() {
    let (tx, _rx) = unbounded_channel();
    let mut app = App::new(None, None, true);
    let initial_count = app.node_profiles.len();

    // Open Add Node Modal
    app.handle_key(key_event(KeyCode::Char('a')), &tx);
    app.add_modal.selected_index = 0;

    // Delete profile with 'd'
    app.handle_key(key_event(KeyCode::Char('d')), &tx);
    assert_eq!(app.node_profiles.len(), initial_count - 1);
}

#[tokio::test]
async fn test_place_node_from_custom_profile() {
    let (tx, _rx) = unbounded_channel();
    let mut app = App::new(None, None, true);

    // Add a custom profile
    let custom_profile = NodeProfile::new(
        "custom-switch",
        "Aruba CX Virtual",
        "arubacx:latest",
        vec!["1/1/1".to_string(), "1/1/2".to_string()],
        "Enterprise access switch",
        NodeCategory::Switch,
    );
    app.node_profiles.push(custom_profile);

    // Open Add Node Modal and select the custom profile
    app.handle_key(key_event(KeyCode::Char('a')), &tx);
    app.add_modal.selected_index = app.node_profiles.len() - 1;

    // Place node with Enter
    app.handle_key(key_event(KeyCode::Enter), &tx);
    assert!(!app.add_modal.is_open);

    // Verify node is present on canvas with matching profile properties
    let added_node = app
        .canvas
        .nodes
        .values()
        .find(|n| n.kind == "custom-switch")
        .expect("Custom switch node must be present");

    assert_eq!(added_node.image, "arubacx:latest");
    assert_eq!(added_node.category, NodeCategory::Switch);
    assert_eq!(added_node.ports.len(), 2);
    assert_eq!(added_node.ports[0].name, "1/1/1");
    assert_eq!(added_node.ports[1].name, "1/1/2");
}

#[tokio::test]
async fn test_drawer_cycle_kind_includes_custom_profiles() {
    let mut app = App::new(None, None, true);

    // Add custom profile
    let custom = NodeProfile::new(
        "my-firewall",
        "Palo Alto VM",
        "paloalto/panos:latest",
        vec!["ethernet1/1".to_string()],
        "Custom NGFW",
        NodeCategory::Firewall,
    );
    app.node_profiles.push(custom);

    app.drawer.open_for_node("srl1");

    // Cycle kinds until "my-firewall" is reached
    let mut found = false;
    for _ in 0..app.node_profiles.len() + 2 {
        app.drawer
            .cycle_kind_with_profiles(&mut app.canvas, &mut app.topology, &app.node_profiles);
        if let Some(node) = app.canvas.nodes.get("srl1") {
            if node.kind == "my-firewall" {
                found = true;
                assert_eq!(node.category, NodeCategory::Firewall);
                assert_eq!(node.image, "paloalto/panos:latest");
                break;
            }
        }
    }
    assert!(
        found,
        "Drawer cycling should cycle through custom node profiles"
    );
}

#[tokio::test]
async fn test_profile_validation_rejects_invalid_inputs() {
    let mut modal = clab_tui::ui::views::ProfileEditModal::default();
    modal.open_for_new();

    // Empty kind name
    modal.kind_name = "".to_string();
    assert!(modal.to_profile().is_err());

    // Invalid characters in kind name
    modal.kind_name = "invalid/kind:name".to_string();
    assert!(modal.to_profile().is_err());

    // Valid kind name with default ports parsing
    modal.kind_name = "valid-node_1".to_string();
    modal.ports = "eth1, eth2; eth3 eth4".to_string();
    let prof = modal.to_profile().unwrap();
    assert_eq!(prof.kind_name, "valid-node_1");
    assert_eq!(prof.default_ports, vec!["eth1", "eth2", "eth3", "eth4"]);
}

#[tokio::test]
async fn test_cannot_delete_last_remaining_profile() {
    let (tx, _rx) = unbounded_channel();
    let mut app = App::new(None, None, true);

    // Drain down to 1 profile
    while app.node_profiles.len() > 1 {
        app.node_profiles.pop();
    }
    assert_eq!(app.node_profiles.len(), 1);

    // Try deleting the only remaining profile
    app.handle_key(key_event(KeyCode::Char('a')), &tx);
    app.handle_key(key_event(KeyCode::Char('d')), &tx);

    // Still 1 profile, not 0
    assert_eq!(app.node_profiles.len(), 1);
}

#[tokio::test]
async fn test_yaml_empty_buffer_and_backspace_at_start() {
    let (tx, _rx) = unbounded_channel();
    let mut app = App::new(None, None, true);

    app.handle_key(key_event(KeyCode::Char('4')), &tx);
    app.handle_key(key_event(KeyCode::Char('e')), &tx);

    app.yaml_state.edit_lines = vec![String::new()];
    app.yaml_state.cursor_row = 0;
    app.yaml_state.cursor_col = 0;

    // Backspace at 0, 0 should not panic or underflow
    app.handle_key(key_event(KeyCode::Backspace), &tx);
    assert_eq!(app.yaml_state.cursor_row, 0);
    assert_eq!(app.yaml_state.cursor_col, 0);

    // Delete on empty line should not panic
    app.handle_key(key_event(KeyCode::Delete), &tx);
    assert_eq!(app.yaml_state.edit_lines.len(), 1);
}

#[tokio::test]
async fn test_yaml_save_rejects_path_traversal() {
    let (tx, _rx) = unbounded_channel();
    let mut app = App::new(None, None, true);

    app.handle_key(key_event(KeyCode::Char('4')), &tx);
    app.handle_key(key_event(KeyCode::Char('e')), &tx);

    let traversal_yaml = "name: ../../etc/passwd\ntopology:\n  nodes:\n    r1:\n      kind: srl\n";
    app.yaml_state.edit_lines = traversal_yaml.lines().map(|s| s.to_string()).collect();

    // Apply edits
    app.handle_key(ctrl_key_event(KeyCode::Char('s')), &tx);

    // Must be rejected by validation
    assert!(app.yaml_state.is_editing);
    assert!(app.yaml_state.error_msg.is_some());
}

#[tokio::test]
async fn test_ascii_mode_link_corners_rendered() {
    let backend = TestBackend::new(120, 40);
    let mut terminal = Terminal::new(backend).unwrap();

    let mut topo = TopologyParser::create_sample_topology();
    // Add a link that requires an orthogonal L-bend or multi-segment routing
    topo.topology
        .links
        .push(LinkDefinition::new("srl1", "e1-3", "host2", "eth2"));

    let app = App::new(Some(topo), None, true);

    terminal.draw(|f| app.render(f)).unwrap();

    // Verify corners exist in rendered buffer
    let has_corners = buffer_contains(&terminal, "┐")
        || buffer_contains(&terminal, "┘")
        || buffer_contains(&terminal, "┌")
        || buffer_contains(&terminal, "└");
    assert!(
        has_corners,
        "Link with bends must render corner box-drawing characters"
    );
}

#[tokio::test]
async fn test_ascii_mode_wiring_corner_glyph() {
    let backend = TestBackend::new(120, 40);
    let mut terminal = Terminal::new(backend).unwrap();
    let (tx, _rx) = unbounded_channel();

    let mut app = App::new(None, None, true);
    app.canvas.selected_node = Some("srl1".to_string());

    // Enter wiring mode
    app.handle_key(key_event(KeyCode::Char('w')), &tx);
    assert_eq!(app.canvas.mode, CanvasMode::Wiring);

    // Target position that creates an orthogonal bend
    app.canvas.wiring_cursor = (50.0, 20.0);

    terminal.draw(|f| app.render(f)).unwrap();

    // Corner glyph should be rendered and not overwritten by vertical line
    assert!(
        buffer_contains(&terminal, "┐")
            || buffer_contains(&terminal, "┘")
            || buffer_contains(&terminal, "┌")
            || buffer_contains(&terminal, "└")
    );
    assert!(buffer_contains(&terminal, "●"));
}

#[tokio::test]
async fn test_node_profile_duplicate_kind_rejected() {
    let (tx, _rx) = unbounded_channel();
    let mut app = App::new(None, None, true);

    // Try adding a profile with existing kind "srl"
    app.handle_key(key_event(KeyCode::Char('a')), &tx);
    app.handle_key(key_event(KeyCode::Char('n')), &tx);
    assert!(app.profile_modal.is_open);

    app.profile_modal.kind_name = "srl".to_string();
    app.profile_modal.focused_field = ProfileField::Save;
    app.handle_key(key_event(KeyCode::Enter), &tx);

    // Duplicate must be rejected
    assert!(app.profile_modal.is_open);
    assert!(app.profile_modal.error_msg.is_some());
    assert!(app
        .profile_modal
        .error_msg
        .as_ref()
        .unwrap()
        .contains("already exists"));
}

#[tokio::test]
async fn test_drawer_cycle_unknown_kind_resets_to_profile_zero() {
    let mut app = App::new(None, None, true);

    // Set node to an unknown kind not present in profiles
    if let Some(node) = app.canvas.nodes.get_mut("srl1") {
        node.kind = "nonexistent_kind_xyz".to_string();
    }

    app.drawer.open_for_node("srl1");
    app.drawer
        .cycle_kind_with_profiles(&mut app.canvas, &mut app.topology, &app.node_profiles);

    // First cycle from unknown kind should reset to profile 0
    let node = app.canvas.nodes.get("srl1").unwrap();
    assert_eq!(node.kind, app.node_profiles[0].kind_name);
}

#[tokio::test]
async fn test_yaml_save_key_applies_pending_edits() {
    let (tx, _rx) = unbounded_channel();
    let tmp = std::env::temp_dir().join(format!("test-save-{}.clab.yml", std::process::id()));
    let mut app = App::new(None, Some(tmp.clone()), true);

    // Start editing YAML
    app.handle_key(key_event(KeyCode::Char('4')), &tx);
    app.handle_key(key_event(KeyCode::Char('e')), &tx);

    let valid_yaml =
        "name: saved-from-edits\ntopology:\n  nodes:\n    spine1:\n      kind: nokia_srlinux\n";
    app.yaml_state.edit_lines = valid_yaml.lines().map(|s| s.to_string()).collect();

    // Exit edit mode with Esc
    app.handle_key(key_event(KeyCode::Esc), &tx);
    assert!(!app.yaml_state.is_editing);

    // Now press 's' in read-only view
    app.handle_key(key_event(KeyCode::Char('s')), &tx);

    // Edits must have been applied to topology and saved
    assert_eq!(app.topology.name, "saved-from-edits");
    assert!(app.canvas.nodes.contains_key("spine1"));
    assert!(tmp.exists());
    let _ = std::fs::remove_file(tmp);
}

#[tokio::test]
async fn test_add_node_modal_scrolling_window() {
    let backend = TestBackend::new(120, 20);
    let mut terminal = Terminal::new(backend).unwrap();
    let (tx, _rx) = unbounded_channel();

    let mut app = App::new(None, None, true);

    // Add extra profiles so count exceeds small terminal height
    for i in 1..=10 {
        app.node_profiles.push(NodeProfile::new(
            format!("router{}", i),
            format!("Router {}", i),
            "test:latest",
            vec!["eth1".to_string()],
            "Test description",
            NodeCategory::Router,
        ));
    }

    // Open Add Node Modal and select the last item
    app.handle_key(key_event(KeyCode::Char('a')), &tx);
    app.add_modal.selected_index = app.node_profiles.len() - 1;

    terminal.draw(|f| app.render(f)).unwrap();

    // The selected last item must be rendered within the visible buffer window
    assert!(buffer_contains(&terminal, "router10"));
}

#[tokio::test]
async fn test_tab_switch_updates_yaml_viewer() {
    let (tx, _rx) = unbounded_channel();
    let mut app = App::new(None, None, true);

    // Canvas is active tab. Add a node to canvas
    let profile = app.node_profiles[0].clone();
    app.canvas.add_node_from_profile(&profile, 20.0, 20.0);
    app.sync_canvas_to_model();

    // Switch to YAML tab using Tab key (cycling through tabs)
    app.handle_key(key_event(KeyCode::Tab), &tx); // Inspect
    app.handle_key(key_event(KeyCode::Tab), &tx); // Logs
    app.handle_key(key_event(KeyCode::Tab), &tx); // Yaml
    assert_eq!(app.active_tab, ActiveTab::Yaml);

    // YAML state must have updated with the new node
    assert!(app.yaml_state.get_text().contains(&profile.kind_name));
}

#[tokio::test]
async fn test_ascii_mode_diagonal_lines_render() {
    let backend = TestBackend::new(120, 40);
    let mut terminal = Terminal::new(backend).unwrap();

    let mut app = App::new(None, None, true);

    // Add a cross-diagonal link between diagonally positioned nodes srl1 (15, 10) and host2 (55, 25)
    app.canvas.add_link("srl1", "e1-3", "host2", "eth2");

    // Switch routing style to Direct (Diagonal)
    app.canvas
        .set_routing_style(clab_tui::canvas::link::RoutingStyle::Direct);

    terminal.draw(|f| app.render(f)).unwrap();

    // Verify diagonal line characters exist in render buffer
    assert!(
        buffer_contains(&terminal, "╲") || buffer_contains(&terminal, "╱"),
        "Diagonal line characters should be present in buffer when direct routing is active"
    );
    // Verify status overlay shows Direct route
    assert!(buffer_contains(&terminal, "Route: Direct (Diag)"));
}

#[tokio::test]
async fn test_ascii_mode_wiring_diagonal_rubber_band() {
    let backend = TestBackend::new(120, 40);
    let mut terminal = Terminal::new(backend).unwrap();
    let (tx, _rx) = unbounded_channel();

    let mut app = App::new(None, None, true);
    app.canvas.selected_node = Some("srl1".to_string());
    app.canvas
        .set_routing_style(clab_tui::canvas::link::RoutingStyle::Direct);

    // Enter wiring mode
    app.handle_key(key_event(KeyCode::Char('w')), &tx);
    assert_eq!(app.canvas.mode, CanvasMode::Wiring);

    // Set cursor at diagonal offset from srl1 port
    app.canvas.wiring_cursor = (50.0, 30.0);

    terminal.draw(|f| app.render(f)).unwrap();

    // Direct wiring rubber band renders solid endpoint marker ● and diagonal line segments
    assert!(buffer_contains(&terminal, "●"));
    assert!(
        buffer_contains(&terminal, "╲") || buffer_contains(&terminal, "╱"),
        "Direct wiring preview rubber band should render diagonal characters"
    );
}

#[tokio::test]
async fn test_ascii_mode_keybinding_cycle_routing_style() {
    let backend = TestBackend::new(120, 40);
    let mut terminal = Terminal::new(backend).unwrap();
    let (tx, _rx) = unbounded_channel();

    let mut app = App::new(None, None, true);

    assert_eq!(
        app.canvas.routing_style,
        clab_tui::canvas::link::RoutingStyle::Orthogonal
    );

    // Press 'r' to cycle to Direct
    app.handle_key(key_event(KeyCode::Char('r')), &tx);
    assert_eq!(
        app.canvas.routing_style,
        clab_tui::canvas::link::RoutingStyle::Direct
    );

    terminal.draw(|f| app.render(f)).unwrap();
    assert!(buffer_contains(&terminal, "Route: Direct (Diag)"));

    // Press 'r' again to cycle to Octilinear
    app.handle_key(key_event(KeyCode::Char('r')), &tx);
    assert_eq!(
        app.canvas.routing_style,
        clab_tui::canvas::link::RoutingStyle::Octilinear
    );

    terminal.draw(|f| app.render(f)).unwrap();
    assert!(buffer_contains(&terminal, "Route: Octilinear (45°)"));

    // Press 'r' again to cycle back to Orthogonal
    app.handle_key(key_event(KeyCode::Char('r')), &tx);
    assert_eq!(
        app.canvas.routing_style,
        clab_tui::canvas::link::RoutingStyle::Orthogonal
    );
}

#[test]
fn test_ascii_mode_line_segment_blending_and_crossings() {
    use clab_tui::canvas::render::CanvasRenderer;
    use ratatui::buffer::Buffer;
    use ratatui::layout::Rect;
    use ratatui::style::Style;

    let area = Rect::new(0, 0, 20, 20);
    let mut buf = Buffer::empty(area);
    let style = Style::default();

    // 1. Draw horizontal line from (2, 5) to (15, 5)
    CanvasRenderer::draw_line_segment(&mut buf, area, 2, 5, 15, 5, style);
    assert_eq!(buf[(5, 5)].symbol(), "─");

    // 2. Draw vertical line from (5, 2) to (5, 10), crossing horizontal line at (5, 5)
    CanvasRenderer::draw_line_segment(&mut buf, area, 5, 2, 5, 10, style);
    assert_eq!(
        buf[(5, 5)].symbol(),
        "┼",
        "Orthogonal intersection must form ┼"
    );

    // 3. Draw diagonal down-right from (2, 2) to (10, 10)
    CanvasRenderer::draw_line_segment(&mut buf, area, 2, 2, 10, 10, style);
    assert_eq!(buf[(3, 3)].symbol(), "╲");

    // 4. Draw diagonal up-right crossing at (6, 6) from (2, 10) to (10, 2)
    CanvasRenderer::draw_line_segment(&mut buf, area, 2, 10, 10, 2, style);
    assert_eq!(buf[(6, 6)].symbol(), "╳", "Crossing diagonals must form ╳");
}

#[tokio::test]
async fn test_ascii_mode_wiring_octilinear_corner_not_plus() {
    let backend = TestBackend::new(120, 40);
    let mut terminal = Terminal::new(backend).unwrap();
    let (tx, _rx) = unbounded_channel();

    let mut app = App::new(None, None, true);
    app.canvas.selected_node = Some("srl1".to_string());
    app.canvas
        .set_routing_style(clab_tui::canvas::link::RoutingStyle::Octilinear);

    // Enter wiring mode
    app.handle_key(key_event(KeyCode::Char('w')), &tx);
    assert_eq!(app.canvas.mode, CanvasMode::Wiring);

    // Set cursor at offset creating an octilinear diagonal bend
    app.canvas.wiring_cursor = (50.0, 30.0);

    terminal.draw(|f| app.render(f)).unwrap();

    // The rubber-band corner must be a diagonal character '╲' or '╱', NOT '+'
    assert!(
        buffer_contains(&terminal, "╲") || buffer_contains(&terminal, "╱"),
        "Octilinear wiring preview must render diagonal glyphs"
    );
    assert!(buffer_contains(&terminal, "●"));
}

#[test]
fn test_draw_line_segment_oblique_endpoint_char() {
    use clab_tui::canvas::render::CanvasRenderer;
    use ratatui::buffer::Buffer;
    use ratatui::layout::Rect;
    use ratatui::style::Style;

    let area = Rect::new(0, 0, 20, 20);
    let mut buf = Buffer::empty(area);
    let style = Style::default();

    // Shallow oblique segment from (0, 0) to (10, 1)
    // The arrival at (10, 1) from (9, 1) is horizontal
    CanvasRenderer::draw_line_segment(&mut buf, area, 0, 0, 10, 1, style);
    assert_eq!(
        buf[(10, 1)].symbol(),
        "─",
        "Shallow segment ending horizontally must end with ─"
    );

    // Steep oblique segment from (0, 0) to (1, 10)
    // The arrival at (1, 10) from (1, 9) is vertical
    let mut buf2 = Buffer::empty(area);
    CanvasRenderer::draw_line_segment(&mut buf2, area, 0, 0, 1, 10, style);
    assert_eq!(
        buf2[(1, 10)].symbol(),
        "│",
        "Steep segment ending vertically must end with │"
    );
}

#[test]
fn test_blend_chars_half_blocks_and_crossings() {
    use clab_tui::canvas::render::CanvasRenderer;

    // Overwriting half-block raster cell
    assert_eq!(CanvasRenderer::blend_chars('▀', '─'), '─');
    assert_eq!(CanvasRenderer::blend_chars('▀', '╲'), '╲');

    // Orthogonal and diagonal intersections
    assert_eq!(CanvasRenderer::blend_chars('─', '╲'), '┼');
    assert_eq!(CanvasRenderer::blend_chars('│', '╱'), '┼');
    assert_eq!(CanvasRenderer::blend_chars('╱', '╲'), '╳');
}

#[test]
fn test_ascii_mode_wiring_orthogonal_corner_not_plus() {
    use clab_tui::canvas::link::RoutingStyle;
    use clab_tui::canvas::node::{CanvasNode, Direction, PortAnchor};
    use clab_tui::canvas::render::CanvasRenderer;
    use clab_tui::canvas::state::{CanvasMode, CanvasState};
    use clab_tui::clab::model::NodeDefinition;
    use clab_tui::ui::theme::Theme;
    use ratatui::buffer::Buffer;
    use ratatui::layout::Rect;

    let mut canvas = CanvasState::new();
    let theme = Theme::tokyo_night();
    let area = Rect::new(0, 0, 80, 40);
    let mut buf = Buffer::empty(area);

    let mut def = NodeDefinition::default();
    def.set_canvas_pos(10.0, 10.0);
    let mut node = CanvasNode::new("n1", &def, 10.0, 10.0);
    node.ports.push(PortAnchor {
        name: "e1-1".to_string(),
        x: 20.0,
        y: 12.0,
        direction: Direction::East,
    });
    canvas.nodes.insert("n1".to_string(), node);
    canvas.wiring_source = Some(("n1".to_string(), "e1-1".to_string()));
    canvas.mode = CanvasMode::Wiring;
    canvas.routing_style = RoutingStyle::Orthogonal;
    canvas.wiring_cursor = (35.0, 20.0);

    CanvasRenderer::render_wiring_preview(&canvas, area, &mut buf, &theme);

    // Corner is at (sx2=35, sy1=12)
    let corner_cell = buf[(35, 12)].symbol();
    assert_ne!(
        corner_cell, "┼",
        "Orthogonal wiring corner must not be '+' or '┼'"
    );
    assert_ne!(corner_cell, "+");
    assert_eq!(
        corner_cell, "┐",
        "Orthogonal corner from (20,12) East to (35,12) then South to (35,20) must be '┐'"
    );
}
