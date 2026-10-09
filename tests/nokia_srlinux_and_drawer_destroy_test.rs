use clab_tui::canvas::node::CanvasNode;
use clab_tui::canvas::state::CanvasState;
use clab_tui::clab::client::ClabClient;
use clab_tui::clab::mock::MockClabClient;
use clab_tui::clab::model::{NodeCategory, NodeDefinition, NodeProfile, KIND_TEMPLATES};
use clab_tui::clab::parser::TopologyParser;
use clab_tui::event::AppEvent;
use clab_tui::ui::layout::ActiveTab;
use clab_tui::ui::theme::Theme;
use clab_tui::ui::widgets::drawer::{DrawerField, InspectorDrawer};
use clab_tui::App;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use std::path::{Path, PathBuf};
use tokio::sync::mpsc::unbounded_channel;

fn key_event(code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, KeyModifiers::NONE)
}

// ===========================================================================
// 1. Nokia SR Linux Kind Tests ("nokia_srlinux" and backward compatible "srl")
// ===========================================================================

#[test]
fn test_nokia_srlinux_kind_templates_and_matching() {
    let srl_tmpl = KIND_TEMPLATES
        .iter()
        .find(|t| t.matches_kind("nokia_srlinux"))
        .expect("nokia_srlinux template must exist");

    assert_eq!(srl_tmpl.kind_name, "nokia_srlinux");
    assert_eq!(srl_tmpl.display_name, "Nokia SR Linux");
    assert_eq!(srl_tmpl.default_image, "ghcr.io/nokia/srlinux:latest");
    assert_eq!(srl_tmpl.node_category, NodeCategory::Router);
    assert_eq!(srl_tmpl.interface_pattern, "e1-{n:1}");
    assert_eq!(srl_tmpl.default_ports, &["e1-1", "e1-2", "e1-3", "e1-4"]);

    // Test matches_kind on KindTemplate
    assert!(srl_tmpl.matches_kind("nokia_srlinux"));
    assert!(srl_tmpl.matches_kind("NOKIA_SRLINUX"));
    assert!(srl_tmpl.matches_kind("srl"));
    assert!(srl_tmpl.matches_kind("SRL"));
    assert!(srl_tmpl.matches_kind("srlinux"));
    assert!(srl_tmpl.matches_kind("nokia-srlinux"));
    assert!(!srl_tmpl.matches_kind("ceos"));

    // Test default profiles
    let profiles = NodeProfile::default_profiles();
    let srl_prof = profiles
        .iter()
        .find(|p| p.matches_kind("nokia_srlinux"))
        .expect("Default profile for nokia_srlinux must exist");
    assert_eq!(srl_prof.kind_name, "nokia_srlinux");
    assert!(srl_prof.matches_kind("nokia_srlinux"));
    assert!(srl_prof.matches_kind("srl"));
    assert!(srl_prof.matches_kind("srlinux"));
    assert!(srl_prof.matches_kind("nokia-srlinux"));
    assert!(!srl_prof.matches_kind("linux"));

    // Test interface pattern derivation
    assert_eq!(
        NodeProfile::default_pattern_for_kind("nokia_srlinux"),
        "e1-{n:1}"
    );
    assert_eq!(NodeProfile::default_pattern_for_kind("srl"), "e1-{n:1}");
    assert_eq!(NodeProfile::default_pattern_for_kind("SRLinux"), "e1-{n:1}");
}

#[test]
fn test_nokia_srlinux_canvas_node_and_alias_srl() {
    // 1. Node created with "nokia_srlinux"
    let def_new = NodeDefinition {
        kind: Some("nokia_srlinux".to_string()),
        ..Default::default()
    };
    let node_new = CanvasNode::new("sr1", &def_new, 10.0, 10.0);
    assert_eq!(node_new.category, NodeCategory::Router);
    assert_eq!(node_new.image, "ghcr.io/nokia/srlinux:latest");
    assert_eq!(node_new.interface_pattern, "e1-{n:1}");
    assert_eq!(node_new.ports.len(), 4);
    assert_eq!(node_new.ports[0].name, "e1-1");
    assert_eq!(node_new.ports[3].name, "e1-4");

    // 2. Node created with legacy alias "srl"
    let def_legacy = NodeDefinition {
        kind: Some("srl".to_string()),
        ..Default::default()
    };
    let node_legacy = CanvasNode::new("sr2", &def_legacy, 20.0, 10.0);
    assert_eq!(node_legacy.category, NodeCategory::Router);
    assert_eq!(node_legacy.image, "ghcr.io/nokia/srlinux:latest");
    assert_eq!(node_legacy.interface_pattern, "e1-{n:1}");
    assert_eq!(node_legacy.ports.len(), 4);
    assert_eq!(node_legacy.ports[0].name, "e1-1");
    assert_eq!(node_legacy.ports[3].name, "e1-4");
}

#[test]
fn test_nokia_srlinux_demo_topologies_yaml_parse() {
    let demo_path = Path::new("examples/demo.clab.yml");
    let topo =
        TopologyParser::parse_file(demo_path).expect("Failed to parse examples/demo.clab.yml");
    assert_eq!(topo.name, "clab-demo");

    let spine1 = topo
        .topology
        .nodes
        .get("spine1")
        .expect("spine1 node present");
    assert_eq!(spine1.kind.as_deref(), Some("nokia_srlinux"));

    let spine2 = topo
        .topology
        .nodes
        .get("spine2")
        .expect("spine2 node present");
    assert_eq!(spine2.kind.as_deref(), Some("nokia_srlinux"));

    let leaf2 = topo
        .topology
        .nodes
        .get("leaf2")
        .expect("leaf2 node present");
    assert_eq!(leaf2.kind.as_deref(), Some("nokia_srlinux"));

    let temp_dir = tempfile::tempdir().expect("Failed to create tempdir");
    let leaf_spine_path = temp_dir.path().join("leaf-spine-demo.clab.yml");
    let sample = TopologyParser::create_sample_topology();
    TopologyParser::save_file(&sample, &leaf_spine_path)
        .expect("Failed to create leaf-spine-demo.clab.yml");
    let ls_topo = TopologyParser::parse_file(&leaf_spine_path)
        .expect("Failed to parse leaf-spine-demo.clab.yml");
    assert_eq!(ls_topo.name, "leaf-spine-demo");
    let srl1 = ls_topo
        .topology
        .nodes
        .get("srl1")
        .expect("srl1 node present");
    assert_eq!(srl1.kind.as_deref(), Some("nokia_srlinux"));
    let srl2 = ls_topo
        .topology
        .nodes
        .get("srl2")
        .expect("srl2 node present");
    assert_eq!(srl2.kind.as_deref(), Some("nokia_srlinux"));
}

#[test]
fn test_mock_inspect_with_nokia_srlinux() {
    let topo = TopologyParser::create_sample_topology();
    let inspect_items = MockClabClient::mock_inspect(&topo);

    assert!(!inspect_items.is_empty());
    let srl2_item = inspect_items
        .iter()
        .find(|c| c.name.contains("srl2"))
        .expect("srl2 container present");
    assert_eq!(srl2_item.kind, "nokia_srlinux");
    assert_eq!(srl2_item.image, "ghcr.io/nokia/srlinux:latest");
}

// ===========================================================================
// 2. Interactive Drawer Kind Selector & Direct Typing Tests
// ===========================================================================

#[tokio::test]
async fn test_drawer_kind_typing_custom_value() {
    let (tx, _rx) = unbounded_channel();
    let mut app = App::new(None, None, true);

    // Open drawer for node srl1
    app.drawer.open_for_node("srl1");
    assert!(app.drawer.is_open);

    // Navigate to Kind field
    app.handle_key(key_event(KeyCode::Down), &tx);
    assert_eq!(app.drawer.focused_field, DrawerField::Kind);

    // Press Enter to start editing Kind
    app.handle_key(key_event(KeyCode::Enter), &tx);
    assert!(app.drawer.is_editing);
    assert!(app.drawer.kind_selector_open);

    // Clear buffer via Delete key event and type a custom kind name
    app.handle_key(key_event(KeyCode::Delete), &tx);
    for ch in "custom_router_os".chars() {
        app.handle_key(key_event(KeyCode::Char(ch)), &tx);
    }
    assert_eq!(app.drawer.edit_buffer, "custom_router_os");

    // Press Enter to commit the custom kind
    app.handle_key(key_event(KeyCode::Enter), &tx);
    assert!(!app.drawer.is_editing);
    assert!(!app.drawer.kind_selector_open);

    let node = app.canvas.nodes.get("srl1").expect("srl1 exists");
    assert_eq!(node.kind, "custom_router_os");
    assert_eq!(node.category, NodeCategory::Router); // Deduced from "router" keyword

    let topo_def = app
        .topology
        .topology
        .nodes
        .get("srl1")
        .expect("srl1 in topo");
    assert_eq!(topo_def.kind.as_deref(), Some("custom_router_os"));
}

#[tokio::test]
async fn test_drawer_kind_selection_from_list_with_arrows() {
    let (tx, _rx) = unbounded_channel();
    let mut app = App::new(None, None, true);

    app.drawer.open_for_node("srl1");
    app.handle_key(key_event(KeyCode::Down), &tx);
    assert_eq!(app.drawer.focused_field, DrawerField::Kind);

    // Enter editing mode
    app.handle_key(key_event(KeyCode::Enter), &tx);
    assert!(app.drawer.is_editing);
    assert!(app.drawer.kind_selector_open);

    let initial_idx = app.drawer.kind_selector_index;

    // Press Down arrow to pick next profile
    app.handle_key(key_event(KeyCode::Down), &tx);
    let next_idx = app.drawer.kind_selector_index;
    assert_ne!(initial_idx, next_idx);
    let selected_prof_kind = app.node_profiles[next_idx].kind_name.clone();
    let selected_prof_cat = app.node_profiles[next_idx].node_category;
    let selected_prof_img = app.node_profiles[next_idx].default_image.clone();
    assert_eq!(app.drawer.edit_buffer, selected_prof_kind);

    // Commit selection
    app.handle_key(key_event(KeyCode::Enter), &tx);
    assert!(!app.drawer.is_editing);

    let node = app.canvas.nodes.get("srl1").expect("srl1 exists");
    assert_eq!(node.kind, selected_prof_kind);
    assert_eq!(node.category, selected_prof_cat);
    assert_eq!(node.image, selected_prof_img);
}

#[tokio::test]
async fn test_drawer_kind_editing_esc_cancels() {
    let (tx, _rx) = unbounded_channel();
    let mut app = App::new(None, None, true);

    let orig_kind = app.canvas.nodes["srl1"].kind.clone();

    app.drawer.open_for_node("srl1");
    app.handle_key(key_event(KeyCode::Down), &tx);
    app.handle_key(key_event(KeyCode::Enter), &tx);
    assert!(app.drawer.is_editing);

    // Type something messy
    for ch in "discarded_edits".chars() {
        app.handle_key(key_event(KeyCode::Char(ch)), &tx);
    }

    // Press Esc to cancel
    app.handle_key(key_event(KeyCode::Esc), &tx);
    assert!(!app.drawer.is_editing);
    assert!(!app.drawer.kind_selector_open);

    // Verify kind unchanged
    assert_eq!(app.canvas.nodes["srl1"].kind, orig_kind);
}

#[test]
fn test_drawer_kind_selector_popup_render() {
    let mut canvas = CanvasState::new();
    let topo = TopologyParser::create_sample_topology();
    canvas.load_from_topology(&topo);
    let profiles = NodeProfile::default_profiles();
    let theme = Theme::tokyo_night();

    let mut drawer = InspectorDrawer::new();
    drawer.open_for_node("srl1");
    drawer.focused_field = DrawerField::Kind;
    drawer.start_editing_kind("nokia_srlinux", &profiles);
    assert!(drawer.kind_selector_open);

    let area = Rect::new(0, 0, 120, 40);
    let mut buf = Buffer::empty(area);

    drawer.render_with_profiles(&canvas, &topo, &profiles, area, &mut buf, &theme);

    let mut content = String::new();
    for y in 0..area.height {
        for x in 0..area.width {
            content.push_str(buf[(x, y)].symbol());
        }
        content.push('\n');
    }

    assert!(
        content.contains("Select or Type Node Kind"),
        "Popup title should be rendered"
    );
    assert!(
        content.contains("nokia_srlinux"),
        "SR Linux kind profile should be listed"
    );
    assert!(
        content.contains("ceos"),
        "Arista cEOS profile should be listed"
    );
}

#[test]
fn test_drawer_kind_selector_mouse_selection() {
    let mut app = App::new(None, None, true);
    app.last_area.set(Rect::new(0, 0, 120, 40));

    app.drawer.open_for_node("srl1");
    app.drawer.focused_field = DrawerField::Kind;
    app.drawer
        .start_editing_kind("nokia_srlinux", &app.node_profiles);
    assert!(app.drawer.kind_selector_open);

    // Position of popup in 120x40 area:
    // drawer_w = 38, drawer_x = 120 - 38 = 82
    // popup_w = 46, popup_x = 82 - 47 = 35
    // popup_y = 3, list_top = 6
    // Row 0 is at y = 6, Row 1 is at y = 7
    let mouse_click_row1 = MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: 40,
        row: 7, // click on second item in list
        modifiers: KeyModifiers::NONE,
    };

    app.handle_mouse(mouse_click_row1);
    assert_eq!(app.drawer.kind_selector_index, 1);
    assert_eq!(app.drawer.edit_buffer, app.node_profiles[1].kind_name);
}

// ===========================================================================
// 3. Teardown and Cleanup Hardening Tests
// ===========================================================================

#[tokio::test]
async fn test_destroy_unpersisted_temp_lab_persists_file_and_destroys() {
    let (tx, mut rx) = unbounded_channel();
    // App with no path on disk yet (None)
    let mut app = App::new(None, None, true);
    assert!(app.topology_path.is_none());

    // Trigger destroy with cleanup
    app.trigger_destroy_with_opts(&tx, true);
    assert!(app.is_operating);
    assert_eq!(app.active_tab, ActiveTab::Logs);

    // Await finished event
    let mut received_finished = false;
    let mut logs = Vec::new();
    while let Some(evt) = rx.recv().await {
        match evt {
            AppEvent::Log(l) => logs.push(l),
            AppEvent::DestroyFinished(success) => {
                assert!(success, "Mock destroy must succeed");
                received_finished = true;
                break;
            }
            _ => {}
        }
    }
    assert!(received_finished);
    assert!(
        logs.iter().any(|l| l.contains("Destroying lab")),
        "Destroy step log present"
    );
    assert!(
        logs.iter().any(|l| l.contains("Removing lab directory")),
        "Cleanup step log present"
    );
}

#[tokio::test]
async fn test_clab_client_destroy_missing_file_reports_error() {
    let client = ClabClient::new();
    let (log_tx, mut log_rx) = unbounded_channel();
    let non_existent = PathBuf::from("/tmp/definitely_does_not_exist_clab_test_123.clab.yml");

    let res = client.destroy(&non_existent, true, log_tx).await;
    assert!(res.is_err(), "Destroy on missing file must return Err");

    let mut log_lines = Vec::new();
    while let Ok(line) = log_rx.try_recv() {
        log_lines.push(line);
    }

    assert!(
        log_lines.iter().any(|l| l.contains("does not exist")),
        "Log channel must contain clear missing file error message"
    );
}

#[tokio::test]
async fn test_mock_simulate_destroy_cleanup_difference() {
    let mock = MockClabClient::new();
    let (log_tx1, mut log_rx1) = unbounded_channel();
    let (log_tx2, mut log_rx2) = unbounded_channel();

    // 1. With cleanup = false
    let _ = mock
        .simulate_destroy(Path::new("test.clab.yml"), false, log_tx1)
        .await;
    let mut logs_no_cleanup = Vec::new();
    while let Ok(line) = log_rx1.try_recv() {
        logs_no_cleanup.push(line);
    }
    assert!(
        !logs_no_cleanup
            .iter()
            .any(|l| l.contains("Removing lab directory")),
        "Without cleanup, directory removal log should not be emitted"
    );

    // 2. With cleanup = true
    let _ = mock
        .simulate_destroy(Path::new("test.clab.yml"), true, log_tx2)
        .await;
    let mut logs_cleanup = Vec::new();
    while let Ok(line) = log_rx2.try_recv() {
        logs_cleanup.push(line);
    }
    assert!(
        logs_cleanup
            .iter()
            .any(|l| l.contains("Removing lab directory")),
        "With cleanup, directory removal log must be emitted"
    );
}

#[tokio::test]
async fn test_drawer_kind_rejects_empty_value() {
    let (tx, _rx) = unbounded_channel();
    let mut app = App::new(None, None, true);

    app.drawer.open_for_node("srl1");
    app.handle_key(key_event(KeyCode::Down), &tx);
    app.handle_key(key_event(KeyCode::Enter), &tx);
    assert!(app.drawer.is_editing);

    let orig_kind = app.canvas.nodes["srl1"].kind.clone();

    // Clear buffer completely
    app.handle_key(key_event(KeyCode::Delete), &tx);
    assert_eq!(app.drawer.edit_buffer, "");

    // Press Enter with empty buffer
    app.handle_key(key_event(KeyCode::Enter), &tx);

    // Kind should NOT be set to empty string
    assert_eq!(app.canvas.nodes["srl1"].kind, orig_kind);
    assert_ne!(app.canvas.nodes["srl1"].kind, "");
}

#[tokio::test]
async fn test_drawer_kind_ctrl_u_clears_and_canonicalizes_srl() {
    let (tx, _rx) = unbounded_channel();
    let mut app = App::new(None, None, true);

    app.drawer.open_for_node("srl1");
    app.handle_key(key_event(KeyCode::Down), &tx);
    app.handle_key(key_event(KeyCode::Enter), &tx);

    // Use Ctrl+u to clear buffer
    let ctrl_u = KeyEvent::new(KeyCode::Char('u'), KeyModifiers::CONTROL);
    app.handle_key(ctrl_u, &tx);
    assert_eq!(app.drawer.edit_buffer, "");

    // Type legacy alias "srl"
    for ch in "srl".chars() {
        app.handle_key(key_event(KeyCode::Char(ch)), &tx);
    }
    assert_eq!(app.drawer.edit_buffer, "srl");

    // Commit
    app.handle_key(key_event(KeyCode::Enter), &tx);
    assert!(!app.drawer.is_editing);

    // Should canonicalize to "nokia_srlinux"
    let node = app.canvas.nodes.get("srl1").expect("srl1 exists");
    assert_eq!(node.kind, "nokia_srlinux");
    assert_eq!(node.category, NodeCategory::Router);
}

#[test]
fn test_drawer_kind_selector_narrow_terminal_no_panic() {
    let mut canvas = CanvasState::new();
    let topo = TopologyParser::create_sample_topology();
    canvas.load_from_topology(&topo);
    let profiles = NodeProfile::default_profiles();
    let theme = Theme::tokyo_night();

    let mut drawer = InspectorDrawer::new();
    drawer.open_for_node("srl1");
    drawer.focused_field = DrawerField::Kind;
    drawer.start_editing_kind("nokia_srlinux", &profiles);

    // Test terminal heights from 4 to 14 (ensuring clamp(min, max) does not panic when height < 12)
    for h in 4..=14 {
        for w in 15..=50 {
            let area = Rect::new(0, 0, w, h);
            let mut buf = Buffer::empty(area);
            drawer.render_with_profiles(&canvas, &topo, &profiles, area, &mut buf, &theme);
        }
    }
}

#[test]
fn test_drawer_kind_selector_mouse_outside_click_closes() {
    let mut app = App::new(None, None, true);
    app.last_area.set(Rect::new(0, 0, 120, 40));

    app.drawer.open_for_node("srl1");
    app.drawer.focused_field = DrawerField::Kind;
    app.drawer
        .start_editing_kind("nokia_srlinux", &app.node_profiles);
    assert!(app.drawer.kind_selector_open);
    assert!(app.drawer.is_editing);

    // Click far outside on canvas (x=5, y=5)
    let outside_click = MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: 5,
        row: 5,
        modifiers: KeyModifiers::NONE,
    };
    app.handle_mouse(outside_click);

    assert!(!app.drawer.kind_selector_open);
    assert!(!app.drawer.is_editing);
}

#[test]
fn test_drawer_kind_selector_double_click_applies() {
    let mut app = App::new(None, None, true);
    app.last_area.set(Rect::new(0, 0, 120, 40));

    app.drawer.open_for_node("srl1");
    app.drawer.focused_field = DrawerField::Kind;
    app.drawer
        .start_editing_kind("nokia_srlinux", &app.node_profiles);
    assert!(app.drawer.kind_selector_open);

    // Click on row 7 (index 1 = ceos)
    let click1 = MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: 40,
        row: 7,
        modifiers: KeyModifiers::NONE,
    };
    app.handle_mouse(click1);
    assert_eq!(app.drawer.kind_selector_index, 1);
    assert!(app.drawer.is_editing);

    // Click again on same row 7 -> applies edit!
    app.handle_mouse(click1);
    assert!(!app.drawer.is_editing);
    assert!(!app.drawer.kind_selector_open);
    assert_eq!(
        app.canvas.nodes["srl1"].kind,
        app.node_profiles[1].kind_name
    );
}

#[test]
fn test_drawer_field_mouse_click_opens_kind_selector() {
    let mut app = App::new(None, None, true);
    app.last_area.set(Rect::new(0, 0, 120, 40));

    // Open drawer, not editing
    app.drawer.open_for_node("srl1");
    assert!(!app.drawer.is_editing);

    // Drawer is at x = 120 - 38 = 82
    // Field 1 (Kind) is at y = 4 + 1 * 3 = 7
    let click_kind_field = MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: 90,
        row: 7,
        modifiers: KeyModifiers::NONE,
    };
    app.handle_mouse(click_kind_field);

    assert_eq!(app.drawer.focused_field, DrawerField::Kind);
    assert!(app.drawer.is_editing);
    assert!(app.drawer.kind_selector_open);
}

#[tokio::test]
async fn test_destroy_missing_file_toast_message() {
    let (tx, _rx) = unbounded_channel();
    let mut app = App::new(None, None, false); // non-mock mode

    // Simulate missing file error in log viewer
    app.log_viewer
        .push_line("ERROR: Topology file '/nonexistent/lab.clab.yml' does not exist".to_string());

    // Dispatch DestroyFinished(false)
    app.handle_event(AppEvent::DestroyFinished(false), &tx);

    let toasts = &app.toast_mgr.toasts;
    assert!(!toasts.is_empty());
    let last_toast = toasts.last().unwrap();
    assert!(
        last_toast.message.contains("Topology file does not exist"),
        "Toast message must be actionable: got '{}'",
        last_toast.message
    );
}

#[tokio::test]
async fn test_destroy_permission_denied_toast_message() {
    let (tx, _rx) = unbounded_channel();
    let mut app = App::new(None, None, false);

    // Simulate permission denied error in log viewer
    app.log_viewer.push_line("ERROR: Permission denied executing containerlab. Check executable permissions and elevated privileges.".to_string());

    app.handle_event(AppEvent::DestroyFinished(false), &tx);

    let toasts = &app.toast_mgr.toasts;
    assert!(!toasts.is_empty());
    let last_toast = toasts.last().unwrap();
    assert!(
        last_toast.message.contains("Permission denied"),
        "Toast message must be actionable: got '{}'",
        last_toast.message
    );
}
