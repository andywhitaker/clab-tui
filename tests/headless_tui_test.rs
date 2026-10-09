use clab_tui::app::App;
use clab_tui::canvas::state::CanvasMode;
use clab_tui::clab::mock::MockClabClient;
use clab_tui::clab::model::KIND_TEMPLATES;
use clab_tui::clab::parser::TopologyParser;
use clab_tui::ui::layout::ActiveTab;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use ratatui::backend::TestBackend;
use ratatui::Terminal;
use tokio::sync::mpsc::unbounded_channel;

fn key_event(code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, KeyModifiers::NONE)
}

fn buffer_contains(terminal: &Terminal<TestBackend>, needle: &str) -> bool {
    let buf = terminal.backend().buffer();
    let content = format!("{:?}", buf);
    content.contains(needle)
}

#[tokio::test]
async fn test_headless_tui_initial_render() {
    let backend = TestBackend::new(120, 40);
    let mut terminal = Terminal::new(backend).unwrap();

    let app = App::new(None, None, true);
    terminal.draw(|f| app.render(f)).unwrap();

    assert!(buffer_contains(&terminal, "clab-tui"));
    assert!(buffer_contains(&terminal, "leaf-spine-demo"));
    assert!(buffer_contains(&terminal, "Canvas Designer"));
    assert!(buffer_contains(&terminal, "srl1"));
}

#[tokio::test]
async fn test_headless_tui_tab_switching() {
    let backend = TestBackend::new(120, 40);
    let mut terminal = Terminal::new(backend).unwrap();
    let (tx, _rx) = unbounded_channel();

    let mut app = App::new(None, None, true);

    // Switch to Inspect Tab (key '2')
    app.handle_key(key_event(KeyCode::Char('2')), &tx);
    assert_eq!(app.active_tab, ActiveTab::Inspect);
    terminal.draw(|f| app.render(f)).unwrap();
    assert!(buffer_contains(&terminal, "Active Lab Containers"));

    // Switch to Logs Tab (key '3')
    app.handle_key(key_event(KeyCode::Char('3')), &tx);
    assert_eq!(app.active_tab, ActiveTab::Logs);
    terminal.draw(|f| app.render(f)).unwrap();
    assert!(buffer_contains(&terminal, "Live Operations Log"));

    // Switch to YAML Tab (key '4')
    app.handle_key(key_event(KeyCode::Char('4')), &tx);
    assert_eq!(app.active_tab, ActiveTab::Yaml);
    terminal.draw(|f| app.render(f)).unwrap();
    assert!(buffer_contains(&terminal, "Containerlab Topology YAML"));

    // Switch back to Canvas (key '1')
    app.handle_key(key_event(KeyCode::Char('1')), &tx);
    assert_eq!(app.active_tab, ActiveTab::Canvas);
}

#[tokio::test]
async fn test_headless_tui_node_movement_and_sync() {
    let (tx, _rx) = unbounded_channel();
    let mut app = App::new(None, None, true);

    app.canvas.selected_node = Some("srl1".to_string());
    let initial_x = app.canvas.nodes["srl1"].x;
    let initial_y = app.canvas.nodes["srl1"].y;

    // Move Right (key 'l')
    app.handle_key(key_event(KeyCode::Char('l')), &tx);
    let new_x = app.canvas.nodes["srl1"].x;
    assert!(new_x > initial_x);

    // Move Down (key 'j')
    app.handle_key(key_event(KeyCode::Char('j')), &tx);
    let new_y = app.canvas.nodes["srl1"].y;
    assert!(new_y > initial_y);

    // Verify synced to topology
    assert_eq!(app.topology.topology.nodes["srl1"].canvas_x(), Some(new_x));
    assert_eq!(app.topology.topology.nodes["srl1"].canvas_y(), Some(new_y));
}

#[tokio::test]
async fn test_headless_tui_add_node_modal() {
    let backend = TestBackend::new(120, 40);
    let mut terminal = Terminal::new(backend).unwrap();
    let (tx, _rx) = unbounded_channel();

    let mut app = App::new(None, None, true);
    let initial_count = app.canvas.nodes.len();

    // Open Add Node Modal (key 'a')
    app.handle_key(key_event(KeyCode::Char('a')), &tx);
    assert!(app.add_modal.is_open);
    terminal.draw(|f| app.render(f)).unwrap();
    assert!(buffer_contains(&terminal, "Add Network Node"));

    // Press Enter to place node
    app.handle_key(key_event(KeyCode::Enter), &tx);
    assert!(!app.add_modal.is_open);
    assert_eq!(app.canvas.nodes.len(), initial_count + 1);
}

#[tokio::test]
async fn test_headless_tui_clone_and_delete_node() {
    let (tx, _rx) = unbounded_channel();
    let mut app = App::new(None, None, true);
    let initial_count = app.canvas.nodes.len();

    app.canvas.selected_node = Some("host1".to_string());

    // Clone node (key 'c')
    app.handle_key(key_event(KeyCode::Char('c')), &tx);
    assert_eq!(app.canvas.nodes.len(), initial_count + 1);

    // Delete selected clone (key 'x')
    app.handle_key(key_event(KeyCode::Char('x')), &tx);
    assert_eq!(app.canvas.nodes.len(), initial_count);
}

#[tokio::test]
async fn test_headless_tui_help_popup() {
    let backend = TestBackend::new(120, 40);
    let mut terminal = Terminal::new(backend).unwrap();
    let (tx, _rx) = unbounded_channel();

    let mut app = App::new(None, None, true);

    // Open Help (key '?')
    app.handle_key(key_event(KeyCode::Char('?')), &tx);
    assert!(app.show_help);
    terminal.draw(|f| app.render(f)).unwrap();
    assert!(buffer_contains(
        &terminal,
        "clab-tui Keyboard & Mouse Reference"
    ));

    // Close Help (key 'Esc')
    app.handle_key(key_event(KeyCode::Esc), &tx);
    assert!(!app.show_help);
}

#[tokio::test]
async fn test_headless_tui_toast_notification() {
    let backend = TestBackend::new(120, 40);
    let mut terminal = Terminal::new(backend).unwrap();

    let mut app = App::new(None, None, true);
    app.toast_mgr.success("Custom Headless Test Toast Message");

    terminal.draw(|f| app.render(f)).unwrap();
    assert!(buffer_contains(
        &terminal,
        "Custom Headless Test Toast Message"
    ));
}

#[tokio::test]
async fn test_headless_tui_wiring_mode() {
    let (tx, _rx) = unbounded_channel();
    let mut app = App::new(None, None, true);

    app.canvas.selected_node = Some("srl1".to_string());

    // Enter Wiring Mode (key 'w')
    app.handle_key(key_event(KeyCode::Char('w')), &tx);
    assert_eq!(app.canvas.mode, clab_tui::canvas::state::CanvasMode::Wiring);

    // Cancel Wiring Mode (key 'Esc')
    app.handle_key(key_event(KeyCode::Esc), &tx);
    assert_eq!(app.canvas.mode, clab_tui::canvas::state::CanvasMode::Normal);
}

#[tokio::test]
async fn test_mouse_node_drag() {
    let backend = TestBackend::new(120, 40);
    let mut terminal = Terminal::new(backend).unwrap();

    let mut app = App::new(None, None, true);
    terminal.draw(|f| app.render(f)).unwrap();

    let initial_x = app.canvas.nodes["srl1"].x;
    let initial_y = app.canvas.nodes["srl1"].y;

    // Node is at canvas (initial_x, initial_y). Body area starts at y=3.
    // Screen coords of node body: column = initial_x as u16 + 5, row = initial_y as u16 + 3 + 1
    let click_col = initial_x as u16 + 5;
    let click_row = initial_y as u16 + 3 + 1;

    // MouseDown inside srl1 body
    app.handle_mouse(MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: click_col,
        row: click_row,
        modifiers: KeyModifiers::NONE,
    });
    assert_eq!(app.canvas.selected_node.as_deref(), Some("srl1"));
    assert_eq!(app.canvas.mode, CanvasMode::DraggingNode);

    // Drag 6 columns right, 4 rows down
    app.handle_mouse(MouseEvent {
        kind: MouseEventKind::Drag(MouseButton::Left),
        column: click_col + 6,
        row: click_row + 4,
        modifiers: KeyModifiers::NONE,
    });

    let dragged_x = app.canvas.nodes["srl1"].x;
    let dragged_y = app.canvas.nodes["srl1"].y;
    let expected_x = initial_x + 6.0;
    let expected_y = initial_y + 4.0;
    assert_eq!(dragged_x, expected_x);
    assert_eq!(dragged_y, expected_y);

    // MouseUp returns to Normal mode
    app.handle_mouse(MouseEvent {
        kind: MouseEventKind::Up(MouseButton::Left),
        column: click_col + 6,
        row: click_row + 4,
        modifiers: KeyModifiers::NONE,
    });
    assert_eq!(app.canvas.mode, CanvasMode::Normal);
}

#[tokio::test]
async fn test_mouse_node_drag_left_edge_does_not_hijack_wiring() {
    let backend = TestBackend::new(120, 40);
    let mut terminal = Terminal::new(backend).unwrap();

    let mut app = App::new(None, None, true);
    terminal.draw(|f| app.render(f)).unwrap();

    let initial_x = app.canvas.nodes["srl1"].x;
    let initial_y = app.canvas.nodes["srl1"].y;

    // Click 1 character inside the node's left border (column = initial_x + 1, row = initial_y + 3 + 2)
    // This is adjacent to the port anchor, but NOT the port anchor glyph itself.
    let click_col = initial_x as u16 + 1;
    let click_row = initial_y as u16 + 3 + 2;

    app.handle_mouse(MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: click_col,
        row: click_row,
        modifiers: KeyModifiers::NONE,
    });
    // Must select node and enter DraggingNode mode, NOT Wiring mode!
    assert_eq!(app.canvas.selected_node.as_deref(), Some("srl1"));
    assert_eq!(app.canvas.mode, CanvasMode::DraggingNode);

    // Drag by an odd offset: 5 columns right, 3 rows down
    app.handle_mouse(MouseEvent {
        kind: MouseEventKind::Drag(MouseButton::Left),
        column: click_col + 5,
        row: click_row + 3,
        modifiers: KeyModifiers::NONE,
    });

    let dragged_x = app.canvas.nodes["srl1"].x;
    let dragged_y = app.canvas.nodes["srl1"].y;
    assert_eq!(dragged_x, initial_x + 5.0);
    assert_eq!(dragged_y, initial_y + 3.0);

    // Release mouse outside canvas viewport (e.g. row = 1 in tabs area)
    app.handle_mouse(MouseEvent {
        kind: MouseEventKind::Up(MouseButton::Left),
        column: click_col + 5,
        row: 1, // tabs area
        modifiers: KeyModifiers::NONE,
    });
    // Mode must cleanly reset to Normal even when released outside canvas
    assert_eq!(app.canvas.mode, CanvasMode::Normal);
}

#[tokio::test]
async fn test_mouse_wiring_abort_on_empty_drag_and_release() {
    let backend = TestBackend::new(120, 40);
    let mut terminal = Terminal::new(backend).unwrap();

    let mut app = App::new(None, None, true);
    terminal.draw(|f| app.render(f)).unwrap();

    let srl1_node = &app.canvas.nodes["srl1"];
    let srl1_port = &srl1_node.ports[0];
    let (s_sx, s_sy) = app.canvas.canvas_to_screen(srl1_port.x, srl1_port.y);
    let port1_col = s_sx as u16;
    let port1_row = s_sy as u16 + 3;

    // Click on port anchor to start wiring
    app.handle_mouse(MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: port1_col,
        row: port1_row,
        modifiers: KeyModifiers::NONE,
    });
    assert_eq!(app.canvas.mode, CanvasMode::Wiring);

    // Drag away into empty canvas space
    app.handle_mouse(MouseEvent {
        kind: MouseEventKind::Drag(MouseButton::Left),
        column: port1_col + 15,
        row: port1_row + 10,
        modifiers: KeyModifiers::NONE,
    });
    assert_eq!(app.canvas.mode, CanvasMode::Wiring);

    // Release on empty canvas space -> should cancel wiring mode
    app.handle_mouse(MouseEvent {
        kind: MouseEventKind::Up(MouseButton::Left),
        column: port1_col + 15,
        row: port1_row + 10,
        modifiers: KeyModifiers::NONE,
    });
    assert_eq!(app.canvas.mode, CanvasMode::Normal);
    assert!(app.canvas.wiring_source.is_none());
}

#[tokio::test]
async fn test_mouse_wiring_click_to_connect() {
    let backend = TestBackend::new(120, 40);
    let mut terminal = Terminal::new(backend).unwrap();

    let mut app = App::new(None, None, true);
    terminal.draw(|f| app.render(f)).unwrap();

    let initial_links = app.canvas.links.len();

    let srl1_node = &app.canvas.nodes["srl1"];
    let srl1_port = &srl1_node.ports[0];
    let (s_sx, s_sy) = app.canvas.canvas_to_screen(srl1_port.x, srl1_port.y);
    let port1_col = s_sx as u16;
    let port1_row = s_sy as u16 + 3;

    // 1. Click down on port 1
    app.handle_mouse(MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: port1_col,
        row: port1_row,
        modifiers: KeyModifiers::NONE,
    });
    assert_eq!(app.canvas.mode, CanvasMode::Wiring);

    // 2. Release without moving (click-to-connect initiates)
    app.handle_mouse(MouseEvent {
        kind: MouseEventKind::Up(MouseButton::Left),
        column: port1_col,
        row: port1_row,
        modifiers: KeyModifiers::NONE,
    });
    assert_eq!(app.canvas.mode, CanvasMode::Wiring);

    // 3. Move mouse across canvas to target port on host2
    let host2_node = &app.canvas.nodes["host2"];
    let host2_port = &host2_node.ports[1]; // eth2
    let (h_sx, h_sy) = app.canvas.canvas_to_screen(host2_port.x, host2_port.y);
    let port2_col = h_sx as u16;
    let port2_row = h_sy as u16 + 3;

    app.handle_mouse(MouseEvent {
        kind: MouseEventKind::Moved,
        column: port2_col,
        row: port2_row,
        modifiers: KeyModifiers::NONE,
    });
    assert_eq!(app.canvas.mode, CanvasMode::Wiring);

    // 4. Click down on target port to connect
    app.handle_mouse(MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: port2_col,
        row: port2_row,
        modifiers: KeyModifiers::NONE,
    });
    assert_eq!(app.canvas.links.len(), initial_links + 1);
    assert_eq!(app.canvas.mode, CanvasMode::Normal);
}

#[tokio::test]
async fn test_mouse_wiring_interaction() {
    let backend = TestBackend::new(120, 40);
    let mut terminal = Terminal::new(backend).unwrap();

    let mut app = App::new(None, None, true);
    terminal.draw(|f| app.render(f)).unwrap();

    let initial_link_count = app.canvas.links.len();

    // Find srl1 port and host1 port
    let srl1_node = &app.canvas.nodes["srl1"];
    let srl1_port = &srl1_node.ports[0];
    let (s_sx, s_sy) = app.canvas.canvas_to_screen(srl1_port.x, srl1_port.y);
    let port1_col = s_sx as u16;
    let port1_row = s_sy as u16 + 3; // body offset

    // Click on srl1 port anchor to start wiring
    app.handle_mouse(MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: port1_col,
        row: port1_row,
        modifiers: KeyModifiers::NONE,
    });
    assert_eq!(app.canvas.mode, CanvasMode::Wiring);
    assert!(app.canvas.wiring_source.is_some());

    // Drag cursor
    app.handle_mouse(MouseEvent {
        kind: MouseEventKind::Drag(MouseButton::Left),
        column: port1_col + 10,
        row: port1_row + 5,
        modifiers: KeyModifiers::NONE,
    });
    assert_eq!(app.canvas.mode, CanvasMode::Wiring);

    // Find an unconnected target port on host2
    let host2_node = &app.canvas.nodes["host2"];
    let host2_port = &host2_node.ports[1]; // eth2
    let (h_sx, h_sy) = app.canvas.canvas_to_screen(host2_port.x, host2_port.y);
    let port2_col = h_sx as u16;
    let port2_row = h_sy as u16 + 3;

    // Release mouse over target port
    app.handle_mouse(MouseEvent {
        kind: MouseEventKind::Up(MouseButton::Left),
        column: port2_col,
        row: port2_row,
        modifiers: KeyModifiers::NONE,
    });

    assert_eq!(app.canvas.links.len(), initial_link_count + 1);
}

#[tokio::test]
async fn test_kitty_graphics_mode_toggle_and_render() {
    let backend = TestBackend::new(120, 40);
    let mut terminal = Terminal::new(backend).unwrap();
    let (tx, _rx) = unbounded_channel();

    let mut app = App::new(None, None, true);

    // Default canvas render: ensure no legacy Kitty / Braille mode header badges exist
    terminal.draw(|f| app.render(f)).unwrap();
    assert!(!buffer_contains(&terminal, "[Braille Mode]"));
    assert!(!buffer_contains(&terminal, "[Kitty: ON]"));
    assert!(!buffer_contains(&terminal, "[KITTY HI-RES ACTIVE]"));
    assert!(!buffer_contains(&terminal, "[g] Graphics"));
    assert!(buffer_contains(&terminal, "srl1"));

    // Pressing 'g' should no longer toggle Kitty mode or change render behavior
    app.handle_key(key_event(KeyCode::Char('g')), &tx);

    terminal.draw(|f| app.render(f)).unwrap();
    assert!(!buffer_contains(&terminal, "[Kitty: ON]"));
    assert!(!buffer_contains(&terminal, "[KITTY HI-RES ACTIVE]"));
    assert!(!buffer_contains(&terminal, "[Braille Mode]"));
    assert!(buffer_contains(&terminal, "srl1"));
}

#[test]
fn test_multitool_image_location() {
    // 1. KindTemplate check
    let linux_tmpl = KIND_TEMPLATES
        .iter()
        .find(|t| t.kind_name == "linux")
        .expect("linux kind template must exist");
    assert_eq!(
        linux_tmpl.default_image,
        "ghcr.io/srl-labs/network-multitool:latest"
    );

    // 2. Sample topology check
    let topo = TopologyParser::create_sample_topology();
    let host1 = &topo.topology.nodes["host1"];
    assert_eq!(
        host1.image.as_deref(),
        Some("ghcr.io/srl-labs/network-multitool:latest")
    );
    let host2 = &topo.topology.nodes["host2"];
    assert_eq!(
        host2.image.as_deref(),
        Some("ghcr.io/srl-labs/network-multitool:latest")
    );

    // 3. Mock inspect check
    let inspect = MockClabClient::mock_inspect(&topo);
    let host_inspect = inspect
        .iter()
        .find(|c| c.name.contains("host1"))
        .expect("host1 inspect info must exist");
    assert_eq!(
        host_inspect.image,
        "ghcr.io/srl-labs/network-multitool:latest"
    );
}
