use clab_tui::canvas::render::CanvasRenderer;
use clab_tui::canvas::state::{CanvasMode, CanvasState};
use clab_tui::clab::model::{canonical_kind, kinds_match, NodeProfile, KIND_TEMPLATES};
use clab_tui::clab::parser::TopologyParser;
use clab_tui::ui::layout::ActiveTab;
use clab_tui::ui::theme::Theme;
use clab_tui::ui::views::canvas_view::{AddNodeModal, CanvasView, CanvasViewParams};
use clab_tui::ui::widgets::drawer::InspectorDrawer;
use clab_tui::App;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use tokio::sync::mpsc::unbounded_channel;

fn key_event(code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, KeyModifiers::NONE)
}

#[test]
fn test_schema_kinds_canonical_resolution_and_aliases() {
    let cases = [
        ("nokia_srlinux", "srl"),
        ("nokia_srlinux", "srlinux"),
        ("nokia_srlinux", "nokia-srlinux"),
        ("arista_ceos", "ceos"),
        ("arista_ceos", "arista-ceos"),
        ("cisco_c8000v", "c8000v"),
        ("cisco_c8000v", "cisco-c8000v"),
        ("cisco_xrv9k", "xrv9k"),
        ("cisco_xrv9k", "cisco-xrv9k"),
        ("cisco_xrv", "xrv"),
        ("cisco_xrv", "vr-xrv"),
        ("juniper_crpd", "crpd"),
        ("juniper_crpd", "juniper-crpd"),
        ("juniper_vrr", "vrr"),
        ("juniper_vrr", "vr-vrr"),
        ("juniper_vrr", "juniper-vrr"),
        ("sonic-vs", "sonic"),
        ("sonic-vs", "sonic_vs"),
        ("sonic-vm", "sonic_vm"),
        ("checkpoint_cloudguard", "checkpoint"),
        ("checkpoint_cloudguard", "cloudguard"),
        ("nokia_sros", "sros"),
        ("nokia_sros", "vr-sros"),
        ("arista_veos", "veos"),
        ("arista_veos", "vr-veos"),
        ("juniper_vmx", "vmx"),
        ("juniper_vmx", "vr-vmx"),
        ("nvidia_cumulusvx", "cumulus_vx"),
        ("nvidia_cumulusvx", "cumulus-vx"),
        ("nvidia_cumulusvx", "cvx"),
        ("linux", "host"),
        ("paloalto_panos", "panos"),
        ("fortinet_fortigate", "fortigate"),
        ("cisco_iol", "iol"),
    ];

    for (canonical, alias) in cases {
        assert_eq!(
            canonical_kind(alias),
            canonical,
            "Alias '{}' should resolve to canonical '{}'",
            alias,
            canonical
        );
        assert_eq!(
            canonical_kind(canonical),
            canonical,
            "Canonical '{}' should resolve to itself",
            canonical
        );
        assert!(
            kinds_match(canonical, alias),
            "kinds_match('{}', '{}') should be true",
            canonical,
            alias
        );
        assert!(
            kinds_match(alias, canonical),
            "kinds_match('{}', '{}') should be true symmetric",
            alias,
            canonical
        );
    }
}

#[test]
fn test_kind_templates_and_default_profiles_match_schema() {
    let profiles = NodeProfile::default_profiles();
    let expected_official_kinds = [
        "nokia_srlinux",
        "arista_ceos",
        "cisco_c8000v",
        "cisco_xrv9k",
        "juniper_crpd",
        "sonic-vs",
        "linux",
        "checkpoint_cloudguard",
        "nokia_sros",
        "arista_veos",
        "juniper_vmx",
        "juniper_vrr",
        "nvidia_cumulusvx",
    ];

    for kind in expected_official_kinds {
        let tmpl = KIND_TEMPLATES.iter().find(|t| t.matches_kind(kind));
        assert!(
            tmpl.is_some(),
            "KIND_TEMPLATES must contain kind '{}'",
            kind
        );

        let prof = profiles.iter().find(|p| p.matches_kind(kind));
        assert!(
            prof.is_some(),
            "NodeProfile::default_profiles must contain kind '{}'",
            kind
        );
        assert_eq!(
            prof.unwrap().kind_name,
            kind,
            "Primary profile must use official schema name"
        );
    }
}

#[test]
fn test_shift_drag_marquee_box_selection_multi_nodes_and_delete() {
    let (tx, _rx) = unbounded_channel();
    let mut app = App::new(None, None, true);
    app.active_tab = ActiveTab::Canvas;
    let area = Rect::new(0, 0, 120, 40);
    app.last_area.set(area);

    // Setup 4 nodes in topology
    assert_eq!(app.canvas.nodes.len(), 4);
    assert!(app.canvas.nodes.contains_key("srl1"));
    assert!(app.canvas.nodes.contains_key("srl2"));
    assert!(app.canvas.nodes.contains_key("host1"));
    assert!(app.canvas.nodes.contains_key("host2"));

    // Check positions:
    // srl1: (15, 10, w=20, h=5)
    // srl2: (55, 10, w=20, h=5)
    // host1: (15, 25, w=20, h=5)
    // host2: (55, 25, w=20, h=5)

    // 1. Shift + MouseDown on empty space (col=2, row=5)
    let down_event = MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: 2,
        row: 5,
        modifiers: KeyModifiers::SHIFT,
    };
    app.handle_mouse(down_event);
    assert_eq!(app.canvas.mode, CanvasMode::BoxSelection);
    assert!(app.canvas.selection_box.is_some());
    assert!(app.canvas.selected_nodes.is_empty());

    // 2. Drag to cover srl1 and srl2 (x=80, row=21 => cy=18)
    let drag_event = MouseEvent {
        kind: MouseEventKind::Drag(MouseButton::Left),
        column: 80,
        row: 21,
        modifiers: KeyModifiers::SHIFT,
    };
    app.handle_mouse(drag_event);
    assert_eq!(app.canvas.mode, CanvasMode::BoxSelection);
    assert!(app.canvas.selected_nodes.contains("srl1"));
    assert!(app.canvas.selected_nodes.contains("srl2"));
    assert!(!app.canvas.selected_nodes.contains("host1"));
    assert!(!app.canvas.selected_nodes.contains("host2"));
    assert_eq!(app.canvas.selected_nodes.len(), 2);

    // 3. MouseUp finishes box selection while retaining selected_nodes
    let up_event = MouseEvent {
        kind: MouseEventKind::Up(MouseButton::Left),
        column: 80,
        row: 21,
        modifiers: KeyModifiers::NONE,
    };
    app.handle_mouse(up_event);
    assert_eq!(app.canvas.mode, CanvasMode::Normal);
    assert!(app.canvas.selection_box.is_none());
    assert_eq!(app.canvas.selected_nodes.len(), 2);
    assert!(app.canvas.selected_nodes.contains("srl1"));
    assert!(app.canvas.selected_nodes.contains("srl2"));

    // 4. Press 'x' or Delete key to delete all multi-selected nodes
    app.handle_key(key_event(KeyCode::Char('x')), &tx);

    // Both srl1 and srl2 should be deleted along with their links
    assert!(!app.canvas.nodes.contains_key("srl1"));
    assert!(!app.canvas.nodes.contains_key("srl2"));
    assert!(app.canvas.nodes.contains_key("host1"));
    assert!(app.canvas.nodes.contains_key("host2"));
    assert_eq!(app.canvas.nodes.len(), 2);

    // All links connected to srl1 or srl2 should be gone
    for link in &app.canvas.links {
        assert_ne!(link.source_node, "srl1");
        assert_ne!(link.target_node, "srl1");
        assert_ne!(link.source_node, "srl2");
        assert_ne!(link.target_node, "srl2");
    }
}

#[test]
fn test_shift_drag_selects_all_nodes_and_delete_all() {
    let (tx, _rx) = unbounded_channel();
    let mut app = App::new(None, None, true);
    app.active_tab = ActiveTab::Canvas;
    let area = Rect::new(0, 0, 120, 40);
    app.last_area.set(area);

    // Drag from (2, 5) to (80, 36) to enclose all 4 nodes
    app.handle_mouse(MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: 2,
        row: 5,
        modifiers: KeyModifiers::SHIFT,
    });
    assert_eq!(app.canvas.mode, CanvasMode::BoxSelection);

    app.handle_mouse(MouseEvent {
        kind: MouseEventKind::Drag(MouseButton::Left),
        column: 80,
        row: 36,
        modifiers: KeyModifiers::SHIFT,
    });
    assert_eq!(app.canvas.selected_nodes.len(), 4);

    app.handle_mouse(MouseEvent {
        kind: MouseEventKind::Up(MouseButton::Left),
        column: 80,
        row: 36,
        modifiers: KeyModifiers::NONE,
    });
    assert_eq!(app.canvas.mode, CanvasMode::Normal);
    assert_eq!(app.canvas.selected_nodes.len(), 4);

    // Press Delete key
    app.handle_key(key_event(KeyCode::Delete), &tx);
    assert!(app.canvas.nodes.is_empty());
    assert!(app.canvas.links.is_empty());
}

#[test]
fn test_click_empty_canvas_clears_multi_selection() {
    let mut app = App::new(None, None, true);
    app.active_tab = ActiveTab::Canvas;
    let area = Rect::new(0, 0, 120, 40);
    app.last_area.set(area);

    // Select nodes
    app.canvas.selected_nodes.clear();
    app.canvas.selected_nodes.insert("srl1".to_string());
    app.canvas.selected_nodes.insert("srl2".to_string());
    assert_eq!(app.canvas.selected_nodes.len(), 2);

    // Click on empty canvas space (e.g. column 2, row 5)
    app.handle_mouse(MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: 2,
        row: 5,
        modifiers: KeyModifiers::NONE,
    });

    // Multi-selection should be cleared
    assert!(app.canvas.selected_nodes.is_empty());
    assert!(app.canvas.selected_node.is_none());
}

#[test]
fn test_shift_click_on_node_initiates_panning() {
    let mut app = App::new(None, None, true);
    app.active_tab = ActiveTab::Canvas;
    let area = Rect::new(0, 0, 120, 40);
    app.last_area.set(area);

    // srl1 node center is at (25, 12) -> screen row = 3 + 12 = 15
    app.handle_mouse(MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: 25,
        row: 15,
        modifiers: KeyModifiers::SHIFT,
    });

    // Per existing test constraints, Shift+Click on a node enters PanningCanvas
    assert_eq!(app.canvas.mode, CanvasMode::PanningCanvas);
}

#[test]
fn test_marquee_box_rendering() {
    let mut canvas = CanvasState::new();
    canvas.selection_box = Some(((10.0, 5.0), (30.0, 15.0)));
    let theme = Theme::tokyo_night();

    let area = Rect::new(0, 0, 80, 25);
    let mut buf = Buffer::empty(area);

    CanvasRenderer::render_marquee_box(&canvas, area, &mut buf, &theme);

    let mut content = String::new();
    for y in 0..area.height {
        for x in 0..area.width {
            content.push_str(buf[(x, y)].symbol());
        }
        content.push('\n');
    }

    assert!(content.contains('┌'), "Marquee top-left corner rendered");
    assert!(content.contains('┐'), "Marquee top-right corner rendered");
    assert!(content.contains('└'), "Marquee bottom-left corner rendered");
    assert!(
        content.contains('┘'),
        "Marquee bottom-right corner rendered"
    );
    assert!(
        content.contains('┄'),
        "Marquee horizontal dashed line rendered"
    );
    assert!(
        content.contains('┆'),
        "Marquee vertical dashed line rendered"
    );
}

#[test]
fn test_multi_node_overlay_status_rendering() {
    let mut canvas = CanvasState::new();
    let topo = TopologyParser::create_sample_topology();
    canvas.load_from_topology(&topo);
    canvas.selected_nodes.clear();
    canvas.selected_nodes.insert("srl1".to_string());
    canvas.selected_nodes.insert("srl2".to_string());
    let theme = Theme::tokyo_night();

    let area = Rect::new(0, 0, 100, 30);
    let mut buf = Buffer::empty(area);

    CanvasRenderer::render_overlay(&canvas, area, &mut buf, &theme);

    let mut content = String::new();
    for y in 0..area.height {
        for x in 0..area.width {
            content.push_str(buf[(x, y)].symbol());
        }
        content.push('\n');
    }

    assert!(
        content.contains("Selected: 2 nodes"),
        "Status overlay must indicate multiple selected nodes count"
    );
}

#[test]
fn test_modal_widths_and_kind_selector_dimensions() {
    let area = Rect::new(0, 0, 120, 40);
    let profiles = NodeProfile::default_profiles();

    // 1. Drawer popup width
    let drawer_rect = Rect::new(82, 0, 38, 40);
    let popup =
        InspectorDrawer::popup_rect(area, drawer_rect, profiles.len()).expect("popup rect exists");
    assert_eq!(
        popup.width, 70,
        "Drawer kind selector popup width should be 70 columns"
    );

    // 2. Render Add Node modal in buffer
    let add_modal = AddNodeModal {
        is_open: true,
        ..Default::default()
    };
    let theme = Theme::tokyo_night();
    let mut buf = Buffer::empty(area);
    let canvas = CanvasState::new();
    let topo = TopologyParser::create_sample_topology();
    let drawer = InspectorDrawer::new();

    let view_params = CanvasViewParams {
        canvas: &canvas,
        topo: &topo,
        drawer: &drawer,
        add_modal: &add_modal,
        profile_modal: None,
        link_modal: None,
        profiles: &profiles,
        theme: &theme,
    };

    CanvasView::render(view_params, area, &mut buf);

    let mut content = String::new();
    for y in 0..area.height {
        for x in 0..area.width {
            content.push_str(buf[(x, y)].symbol());
        }
        content.push('\n');
    }

    // Modal width is responsive up to 85% width (between 82 and 116), and displays long kinds without truncation
    assert!(
        content.contains("Add Network Node"),
        "Add modal title present"
    );
    assert!(
        content.contains("nokia_srlinux"),
        "Add modal displays official kind name nokia_srlinux"
    );
    assert!(
        content.contains("checkpoint_cloudguard"),
        "Add modal displays long kind name checkpoint_cloudguard"
    );
    assert!(
        content.contains("nvidia_cumulusvx"),
        "Add modal displays long kind name nvidia_cumulusvx"
    );
}

#[test]
fn test_shift_drag_negative_direction_marquee_selection() {
    let (tx, _rx) = unbounded_channel();
    let mut app = App::new(None, None, true);
    app.active_tab = ActiveTab::Canvas;
    let area = Rect::new(0, 0, 120, 40);
    app.last_area.set(area);

    // Negative drag: Start from bottom-right (80, 36) and drag up-left to (2, 5)
    app.handle_mouse(MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: 80,
        row: 36,
        modifiers: KeyModifiers::SHIFT,
    });
    assert_eq!(app.canvas.mode, CanvasMode::BoxSelection);

    app.handle_mouse(MouseEvent {
        kind: MouseEventKind::Drag(MouseButton::Left),
        column: 2,
        row: 5,
        modifiers: KeyModifiers::SHIFT,
    });
    assert_eq!(
        app.canvas.selected_nodes.len(),
        4,
        "Negative drag must enclose all 4 nodes"
    );

    app.handle_mouse(MouseEvent {
        kind: MouseEventKind::Up(MouseButton::Left),
        column: 2,
        row: 5,
        modifiers: KeyModifiers::NONE,
    });
    assert_eq!(app.canvas.mode, CanvasMode::Normal);
    assert_eq!(app.canvas.selected_nodes.len(), 4);

    // Press 'x' to bulk delete
    app.handle_key(key_event(KeyCode::Char('x')), &tx);
    assert!(app.canvas.nodes.is_empty(), "All nodes should be deleted");
    assert!(app.canvas.links.is_empty(), "All links should be deleted");
}

#[test]
fn test_marquee_selection_with_zoom_and_pan() {
    let mut app = App::new(None, None, true);
    app.active_tab = ActiveTab::Canvas;
    let area = Rect::new(0, 0, 120, 40);
    app.last_area.set(area);

    // Set zoom to 1.5 and offset to (10, 5)
    app.canvas.zoom = 1.5;
    app.canvas.offset_x = 10.0;
    app.canvas.offset_y = 5.0;

    // Determine screen coordinates of srl1 (canvas x=15, y=10)
    let (srl1_sx, srl1_sy) = app.canvas.canvas_to_screen(15.0, 10.0);
    let srl1_col = (srl1_sx as u16).min(119);
    let srl1_row = (srl1_sy as u16 + 3).min(39);

    // Drag a box starting from empty space and enclosing srl1
    let start_col = srl1_col.saturating_sub(4);
    let start_row = srl1_row.saturating_sub(2);
    let end_col = srl1_col + 32;
    let end_row = srl1_row + 10;

    app.handle_mouse(MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: start_col,
        row: start_row,
        modifiers: KeyModifiers::SHIFT,
    });
    assert_eq!(app.canvas.mode, CanvasMode::BoxSelection);

    app.handle_mouse(MouseEvent {
        kind: MouseEventKind::Drag(MouseButton::Left),
        column: end_col,
        row: end_row,
        modifiers: KeyModifiers::SHIFT,
    });
    assert!(app.canvas.selected_nodes.contains("srl1"));

    app.handle_mouse(MouseEvent {
        kind: MouseEventKind::Up(MouseButton::Left),
        column: end_col,
        row: end_row,
        modifiers: KeyModifiers::NONE,
    });
    assert_eq!(app.canvas.mode, CanvasMode::Normal);
    assert!(app.canvas.selected_nodes.contains("srl1"));
}

#[test]
fn test_esc_key_clears_multi_selection_and_cancels_box() {
    let (tx, _rx) = unbounded_channel();
    let mut app = App::new(None, None, true);
    app.active_tab = ActiveTab::Canvas;
    let area = Rect::new(0, 0, 120, 40);
    app.last_area.set(area);

    // Select 2 nodes
    app.canvas.selected_nodes.insert("srl1".to_string());
    app.canvas.selected_nodes.insert("srl2".to_string());
    app.canvas.selected_node = Some("srl1".to_string());

    // Press Esc
    app.handle_key(key_event(KeyCode::Esc), &tx);
    assert!(
        app.canvas.selected_nodes.is_empty(),
        "Esc key must clear multi-selection"
    );
    assert!(app.canvas.selected_node.is_none());

    // Test Esc during BoxSelection mode
    app.canvas.mode = CanvasMode::BoxSelection;
    app.canvas.selection_box = Some(((10.0, 10.0), (30.0, 30.0)));
    app.handle_key(key_event(KeyCode::Esc), &tx);
    assert_eq!(app.canvas.mode, CanvasMode::Normal);
    assert!(
        app.canvas.selection_box.is_none(),
        "Esc must cancel BoxSelection"
    );
}

#[test]
fn test_click_outside_canvas_body_clears_multi_selection() {
    let mut app = App::new(None, None, true);
    app.active_tab = ActiveTab::Canvas;
    let area = Rect::new(0, 0, 120, 40);
    app.last_area.set(area);

    // Select 2 nodes
    app.canvas.selected_nodes.insert("srl1".to_string());
    app.canvas.selected_nodes.insert("srl2".to_string());

    // Click outside canvas area (e.g. column 0, row 0 which is top status header)
    app.handle_mouse(MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: 0,
        row: 0,
        modifiers: KeyModifiers::NONE,
    });

    assert!(
        app.canvas.selected_nodes.is_empty(),
        "Clicking outside body must clear multi-selection"
    );
}

#[test]
fn test_multi_node_keyboard_movement() {
    let (tx, _rx) = unbounded_channel();
    let mut app = App::new(None, None, true);
    app.active_tab = ActiveTab::Canvas;
    let area = Rect::new(0, 0, 120, 40);
    app.last_area.set(area);

    // Select srl1 and srl2
    app.canvas.selected_nodes.clear();
    app.canvas.selected_nodes.insert("srl1".to_string());
    app.canvas.selected_nodes.insert("srl2".to_string());

    let srl1_start_x = app.canvas.nodes.get("srl1").unwrap().x;
    let srl2_start_x = app.canvas.nodes.get("srl2").unwrap().x;

    // Press Right arrow (or 'l')
    app.handle_key(key_event(KeyCode::Right), &tx);

    let srl1_after_x = app.canvas.nodes.get("srl1").unwrap().x;
    let srl2_after_x = app.canvas.nodes.get("srl2").unwrap().x;

    assert!(srl1_after_x > srl1_start_x, "srl1 moved right");
    assert!(srl2_after_x > srl2_start_x, "srl2 moved right");
    assert_eq!(
        srl1_after_x - srl1_start_x,
        srl2_after_x - srl2_start_x,
        "Both moved by the same delta"
    );
}

#[test]
fn test_drawer_auto_closes_when_target_node_deleted() {
    let (tx, _rx) = unbounded_channel();
    let mut app = App::new(None, None, true);
    app.active_tab = ActiveTab::Canvas;
    let area = Rect::new(0, 0, 120, 40);
    app.last_area.set(area);

    // Open drawer on srl1
    app.drawer.open_for_node("srl1");
    assert!(app.drawer.is_open);
    assert_eq!(app.drawer.target_node_name.as_deref(), Some("srl1"));

    // Multi-select srl1 and srl2, then delete
    app.canvas.selected_nodes.clear();
    app.canvas.selected_nodes.insert("srl1".to_string());
    app.canvas.selected_nodes.insert("srl2".to_string());

    app.handle_key(key_event(KeyCode::Delete), &tx);
    assert!(
        !app.drawer.is_open,
        "Drawer must close when its target node is deleted in bulk delete"
    );
}

#[test]
fn test_cyclic_topology_with_parallel_links_bulk_delete() {
    let mut canvas = CanvasState::new();

    // Triangle topology: n1, n2, n3
    canvas.add_node_from_template("nokia_srlinux", 10.0, 10.0);
    canvas.add_node_from_template("nokia_srlinux", 50.0, 10.0);
    canvas.add_node_from_template("nokia_srlinux", 30.0, 30.0);

    let keys: Vec<String> = canvas.nodes.keys().cloned().collect();
    let (n1, n2, n3) = (&keys[0], &keys[1], &keys[2]);

    // Parallel links between n1 and n2, plus links between n2 and n3, and n3 and n1
    canvas.add_link(n1, "e1-1", n2, "e1-1");
    canvas.add_link(n1, "e1-2", n2, "e1-2");
    canvas.add_link(n2, "e1-3", n3, "e1-1");
    canvas.add_link(n3, "e1-2", n1, "e1-3");
    canvas.recalculate_all_links();
    assert_eq!(canvas.links.len(), 4);

    // Multi-select n1 and n2
    canvas.selected_nodes.clear();
    canvas.selected_nodes.insert(n1.clone());
    canvas.selected_nodes.insert(n2.clone());

    let deleted = canvas.remove_selected_nodes();
    assert_eq!(deleted.len(), 2);
    assert_eq!(canvas.nodes.len(), 1);
    assert!(canvas.nodes.contains_key(n3));

    // All links attached to n1 or n2 should be removed, leaving 0 links
    assert!(
        canvas.links.is_empty(),
        "All incident and parallel links should be removed"
    );
}

#[test]
fn test_deduce_category_for_all_schema_kinds() {
    use clab_tui::clab::model::{deduce_category, NodeCategory};

    // Routers
    assert_eq!(deduce_category("nokia_srlinux"), NodeCategory::Router);
    assert_eq!(deduce_category("srl"), NodeCategory::Router);
    assert_eq!(deduce_category("cisco_c8000v"), NodeCategory::Router);
    assert_eq!(deduce_category("cisco_xrv9k"), NodeCategory::Router);
    assert_eq!(deduce_category("juniper_crpd"), NodeCategory::Router);
    assert_eq!(deduce_category("juniper_vrr"), NodeCategory::Router);
    assert_eq!(deduce_category("vrr"), NodeCategory::Router);
    assert_eq!(deduce_category("nokia_sros"), NodeCategory::Router);
    assert_eq!(deduce_category("juniper_vmx"), NodeCategory::Router);
    assert_eq!(deduce_category("cisco_csr1000v"), NodeCategory::Router);
    assert_eq!(deduce_category("cisco_xrd"), NodeCategory::Router);

    // Switches
    assert_eq!(deduce_category("arista_ceos"), NodeCategory::Switch);
    assert_eq!(deduce_category("ceos"), NodeCategory::Switch);
    assert_eq!(deduce_category("arista_veos"), NodeCategory::Switch);
    assert_eq!(deduce_category("sonic-vs"), NodeCategory::Switch);
    assert_eq!(deduce_category("sonic_vs"), NodeCategory::Switch);
    assert_eq!(deduce_category("nvidia_cumulusvx"), NodeCategory::Switch);
    assert_eq!(deduce_category("cumulus_vx"), NodeCategory::Switch);
    assert_eq!(deduce_category("aruba_aoscx"), NodeCategory::Switch);
    assert_eq!(deduce_category("dell_ftosv"), NodeCategory::Switch);
    assert_eq!(deduce_category("dell_sonic"), NodeCategory::Switch);

    // Firewalls
    assert_eq!(
        deduce_category("checkpoint_cloudguard"),
        NodeCategory::Firewall
    );
    assert_eq!(deduce_category("checkpoint"), NodeCategory::Firewall);
    assert_eq!(deduce_category("firewall"), NodeCategory::Firewall);
    assert_eq!(deduce_category("paloalto_panos"), NodeCategory::Firewall);
    assert_eq!(
        deduce_category("fortinet_fortigate"),
        NodeCategory::Firewall
    );
    assert_eq!(deduce_category("juniper_vsrx"), NodeCategory::Firewall);
    assert_eq!(deduce_category("juniper_csrx"), NodeCategory::Firewall);
    assert_eq!(deduce_category("cisco_ftdv"), NodeCategory::Firewall);

    // Hosts
    assert_eq!(deduce_category("linux"), NodeCategory::Host);
    assert_eq!(deduce_category("host"), NodeCategory::Host);
}

#[test]
fn test_modal_responsive_width_on_wide_terminals() {
    let wide_area = Rect::new(0, 0, 140, 45);
    let profiles = NodeProfile::default_profiles();
    let add_modal = AddNodeModal {
        is_open: true,
        ..Default::default()
    };
    let theme = Theme::tokyo_night();
    let mut buf = Buffer::empty(wide_area);
    let canvas = CanvasState::new();
    let topo = TopologyParser::create_sample_topology();
    let drawer = InspectorDrawer::new();

    let view_params = CanvasViewParams {
        canvas: &canvas,
        topo: &topo,
        drawer: &drawer,
        add_modal: &add_modal,
        profile_modal: None,
        link_modal: None,
        profiles: &profiles,
        theme: &theme,
    };

    CanvasView::render(view_params, wide_area, &mut buf);

    let mut content = String::new();
    for y in 0..wide_area.height {
        for x in 0..wide_area.width {
            content.push_str(buf[(x, y)].symbol());
        }
        content.push('\n');
    }

    // Full description text is displayed on wide terminals without truncation
    assert!(
        content.contains("Modern network OS with open model-driven management"),
        "Wide terminal modal displays complete description for Nokia SR Linux"
    );
    assert!(
        content.contains("Check Point next-generation security firewall"),
        "Wide terminal modal displays complete description for Check Point CloudGuard"
    );
}

// ===========================================================================
// Visual / Box Select Keyboard Toggle & Alternative Modifiers Tests
// ===========================================================================

#[test]
fn test_default_startup_topology_srlinux_kind() {
    // 1. TopologyParser::create_sample_topology() defaults to nokia_srlinux
    let topo = TopologyParser::create_sample_topology();
    let srl1 = topo.topology.nodes.get("srl1").expect("srl1 node exists");
    assert_eq!(
        srl1.kind.as_deref(),
        Some("nokia_srlinux"),
        "srl1 in sample topology must default to nokia_srlinux"
    );
    let srl2 = topo.topology.nodes.get("srl2").expect("srl2 node exists");
    assert_eq!(
        srl2.kind.as_deref(),
        Some("nokia_srlinux"),
        "srl2 in sample topology must default to nokia_srlinux"
    );

    // 2. Launching clab-tui App without -f loads nokia_srlinux on canvas
    let app = App::new(None, None, true);
    let canvas_srl1 = app.canvas.nodes.get("srl1").expect("srl1 node on canvas");
    assert_eq!(
        canvas_srl1.kind, "nokia_srlinux",
        "Canvas srl1 must have kind nokia_srlinux"
    );
    let canvas_srl2 = app.canvas.nodes.get("srl2").expect("srl2 node on canvas");
    assert_eq!(
        canvas_srl2.kind, "nokia_srlinux",
        "Canvas srl2 must have kind nokia_srlinux"
    );
}

#[tokio::test]
async fn test_visual_mode_keyboard_toggle_v_b_space() {
    let (tx, _rx) = unbounded_channel();
    let mut app = App::new(None, None, true);
    app.active_tab = ActiveTab::Canvas;
    assert_eq!(app.canvas.mode, CanvasMode::Normal);

    // Press 'v' to toggle BoxSelection mode
    app.handle_key(key_event(KeyCode::Char('v')), &tx);
    assert_eq!(app.canvas.mode, CanvasMode::BoxSelection);

    // Press 'v' again to toggle back to Normal
    app.handle_key(key_event(KeyCode::Char('v')), &tx);
    assert_eq!(app.canvas.mode, CanvasMode::Normal);

    // Press 'b' to toggle BoxSelection mode
    app.handle_key(key_event(KeyCode::Char('b')), &tx);
    assert_eq!(app.canvas.mode, CanvasMode::BoxSelection);

    // Press 'b' again to toggle back to Normal
    app.handle_key(key_event(KeyCode::Char('b')), &tx);
    assert_eq!(app.canvas.mode, CanvasMode::Normal);

    // Press 's' to toggle BoxSelection mode
    app.handle_key(key_event(KeyCode::Char('s')), &tx);
    assert_eq!(app.canvas.mode, CanvasMode::BoxSelection);

    // Press 's' again to toggle back to Normal
    app.handle_key(key_event(KeyCode::Char('s')), &tx);
    assert_eq!(app.canvas.mode, CanvasMode::Normal);

    // Press 'S' to toggle BoxSelection mode
    app.handle_key(key_event(KeyCode::Char('S')), &tx);
    assert_eq!(app.canvas.mode, CanvasMode::BoxSelection);

    // Press 'S' again to toggle back to Normal
    app.handle_key(key_event(KeyCode::Char('S')), &tx);
    assert_eq!(app.canvas.mode, CanvasMode::Normal);

    // Press Space to toggle BoxSelection mode
    app.handle_key(key_event(KeyCode::Char(' ')), &tx);
    assert_eq!(app.canvas.mode, CanvasMode::BoxSelection);

    // Press Space again to toggle back to Normal
    app.handle_key(key_event(KeyCode::Char(' ')), &tx);
    assert_eq!(app.canvas.mode, CanvasMode::Normal);
}

#[test]
fn test_visual_mode_drag_without_shift_and_delete() {
    let (tx, _rx) = unbounded_channel();
    let mut app = App::new(None, None, true);
    app.active_tab = ActiveTab::Canvas;
    let area = Rect::new(0, 0, 120, 40);
    app.last_area.set(area);

    // 1. Enter Visual / Box Select mode via 'v' key
    app.handle_key(key_event(KeyCode::Char('v')), &tx);
    assert_eq!(app.canvas.mode, CanvasMode::BoxSelection);

    // 2. Click (MouseDown) on canvas WITHOUT holding Shift (modifiers: NONE)
    app.handle_mouse(MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: 2,
        row: 5,
        modifiers: KeyModifiers::NONE,
    });
    assert_eq!(app.canvas.mode, CanvasMode::BoxSelection);
    assert!(app.canvas.selection_box.is_some());

    // 3. Drag to cover srl1 and srl2 with NO modifiers
    app.handle_mouse(MouseEvent {
        kind: MouseEventKind::Drag(MouseButton::Left),
        column: 80,
        row: 21,
        modifiers: KeyModifiers::NONE,
    });
    assert_eq!(app.canvas.mode, CanvasMode::BoxSelection);
    assert!(app.canvas.selected_nodes.contains("srl1"));
    assert!(app.canvas.selected_nodes.contains("srl2"));
    assert_eq!(app.canvas.selected_nodes.len(), 2);

    // 4. MouseUp with NO modifiers
    app.handle_mouse(MouseEvent {
        kind: MouseEventKind::Up(MouseButton::Left),
        column: 80,
        row: 21,
        modifiers: KeyModifiers::NONE,
    });
    assert_eq!(app.canvas.mode, CanvasMode::Normal);
    assert!(app.canvas.selection_box.is_none());
    assert_eq!(app.canvas.selected_nodes.len(), 2);

    // 5. Delete multi-selected nodes via 'x'
    app.handle_key(key_event(KeyCode::Char('x')), &tx);
    assert!(!app.canvas.nodes.contains_key("srl1"));
    assert!(!app.canvas.nodes.contains_key("srl2"));
    assert!(app.canvas.nodes.contains_key("host1"));
    assert!(app.canvas.nodes.contains_key("host2"));
    assert_eq!(app.canvas.nodes.len(), 2);
}

#[test]
fn test_alt_click_and_drag_marquee_selection() {
    let (tx, _rx) = unbounded_channel();
    let mut app = App::new(None, None, true);
    app.active_tab = ActiveTab::Canvas;
    let area = Rect::new(0, 0, 120, 40);
    app.last_area.set(area);

    // 1. MouseDown with ALT modifier
    app.handle_mouse(MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: 2,
        row: 5,
        modifiers: KeyModifiers::ALT,
    });
    assert_eq!(app.canvas.mode, CanvasMode::BoxSelection);

    // 2. Drag with ALT modifier
    app.handle_mouse(MouseEvent {
        kind: MouseEventKind::Drag(MouseButton::Left),
        column: 80,
        row: 21,
        modifiers: KeyModifiers::ALT,
    });
    assert_eq!(app.canvas.selected_nodes.len(), 2);

    // 3. MouseUp
    app.handle_mouse(MouseEvent {
        kind: MouseEventKind::Up(MouseButton::Left),
        column: 80,
        row: 21,
        modifiers: KeyModifiers::NONE,
    });
    assert_eq!(app.canvas.mode, CanvasMode::Normal);
    assert_eq!(app.canvas.selected_nodes.len(), 2);

    // 4. Delete key deletes selected
    app.handle_key(key_event(KeyCode::Delete), &tx);
    assert!(!app.canvas.nodes.contains_key("srl1"));
    assert!(!app.canvas.nodes.contains_key("srl2"));
    assert_eq!(app.canvas.nodes.len(), 2);
}

#[test]
fn test_ctrl_click_empty_space_initiates_panning() {
    let mut app = App::new(None, None, true);
    app.active_tab = ActiveTab::Canvas;
    let area = Rect::new(0, 0, 120, 40);
    app.last_area.set(area);

    let init_offset_x = app.canvas.offset_x;
    let init_offset_y = app.canvas.offset_y;

    // 1. MouseDown with CONTROL modifier on canvas
    app.handle_mouse(MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: 20,
        row: 20,
        modifiers: KeyModifiers::CONTROL,
    });
    assert_eq!(app.canvas.mode, CanvasMode::PanningCanvas);

    // 2. Drag by +10 in x and +5 in y with CONTROL modifier
    app.handle_mouse(MouseEvent {
        kind: MouseEventKind::Drag(MouseButton::Left),
        column: 30,
        row: 25,
        modifiers: KeyModifiers::CONTROL,
    });
    assert_eq!(app.canvas.mode, CanvasMode::PanningCanvas);
    assert_eq!(app.canvas.offset_x, init_offset_x + 10.0);
    assert_eq!(app.canvas.offset_y, init_offset_y + 5.0);

    // 3. MouseUp
    app.handle_mouse(MouseEvent {
        kind: MouseEventKind::Up(MouseButton::Left),
        column: 30,
        row: 25,
        modifiers: KeyModifiers::NONE,
    });
    assert_eq!(app.canvas.mode, CanvasMode::Normal);
}

#[test]
fn test_panning_to_box_selection_drag_transition() {
    let mut app = App::new(None, None, true);
    app.active_tab = ActiveTab::Canvas;
    let area = Rect::new(0, 0, 120, 40);
    app.last_area.set(area);

    // 1. MouseDown on empty canvas with NONE (enters PanningCanvas)
    app.handle_mouse(MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: 2,
        row: 5,
        modifiers: KeyModifiers::NONE,
    });
    assert_eq!(app.canvas.mode, CanvasMode::PanningCanvas);

    // 2. While dragging, user holds ALT or SHIFT
    app.handle_mouse(MouseEvent {
        kind: MouseEventKind::Drag(MouseButton::Left),
        column: 80,
        row: 21,
        modifiers: KeyModifiers::ALT,
    });

    // Seamlessly transitions from PanningCanvas to BoxSelection!
    assert_eq!(app.canvas.mode, CanvasMode::BoxSelection);
    assert!(app.canvas.selected_nodes.contains("srl1"));
    assert!(app.canvas.selected_nodes.contains("srl2"));

    // 3. MouseUp completes selection
    app.handle_mouse(MouseEvent {
        kind: MouseEventKind::Up(MouseButton::Left),
        column: 80,
        row: 21,
        modifiers: KeyModifiers::NONE,
    });
    assert_eq!(app.canvas.mode, CanvasMode::Normal);
    assert_eq!(app.canvas.selected_nodes.len(), 2);
}

#[test]
fn test_space_key_repeat_hold_during_drag() {
    let (tx, _rx) = unbounded_channel();
    let mut app = App::new(None, None, true);
    app.active_tab = ActiveTab::Canvas;
    let area = Rect::new(0, 0, 120, 40);
    app.last_area.set(area);

    // 1. Initial press of Space enters BoxSelection
    app.handle_key(key_event(KeyCode::Char(' ')), &tx);
    assert_eq!(app.canvas.mode, CanvasMode::BoxSelection);

    // 2. MouseDown starts selection
    app.handle_mouse(MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: 2,
        row: 5,
        modifiers: KeyModifiers::NONE,
    });
    assert!(app.canvas.selection_box.is_some());

    // 3. While dragging with mouse, Space key repeat events arrive from holding Space
    let mut repeat_event = key_event(KeyCode::Char(' '));
    repeat_event.kind = crossterm::event::KeyEventKind::Repeat;
    app.handle_key(repeat_event, &tx);
    assert_eq!(app.canvas.mode, CanvasMode::BoxSelection);

    // Drag to cover srl1
    app.handle_mouse(MouseEvent {
        kind: MouseEventKind::Drag(MouseButton::Left),
        column: 40,
        row: 20,
        modifiers: KeyModifiers::NONE,
    });
    assert_eq!(app.canvas.mode, CanvasMode::BoxSelection);
    assert!(app.canvas.selected_nodes.contains("srl1"));

    // Space event during active selection box must not toggle off
    app.handle_key(key_event(KeyCode::Char(' ')), &tx);
    assert_eq!(app.canvas.mode, CanvasMode::BoxSelection);

    // 4. MouseUp finishes selection
    app.handle_mouse(MouseEvent {
        kind: MouseEventKind::Up(MouseButton::Left),
        column: 40,
        row: 20,
        modifiers: KeyModifiers::NONE,
    });
    assert_eq!(app.canvas.mode, CanvasMode::Normal);
    assert!(app.canvas.selected_nodes.contains("srl1"));
}

#[test]
fn test_visual_mode_single_click_selects_node() {
    let (tx, _rx) = unbounded_channel();
    let mut app = App::new(None, None, true);
    app.active_tab = ActiveTab::Canvas;
    let area = Rect::new(0, 0, 120, 40);
    app.last_area.set(area);

    // Enter visual mode with 'v'
    app.handle_key(key_event(KeyCode::Char('v')), &tx);
    assert_eq!(app.canvas.mode, CanvasMode::BoxSelection);

    // Click on srl1 (pos: 15, 10 -> screen row 14, col 20)
    app.handle_mouse(MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: 20,
        row: 14,
        modifiers: KeyModifiers::NONE,
    });
    assert_eq!(app.canvas.mode, CanvasMode::BoxSelection);

    // Release mouse at same location without dragging
    app.handle_mouse(MouseEvent {
        kind: MouseEventKind::Up(MouseButton::Left),
        column: 20,
        row: 14,
        modifiers: KeyModifiers::NONE,
    });
    assert_eq!(app.canvas.mode, CanvasMode::Normal);
    assert_eq!(app.canvas.selected_node.as_deref(), Some("srl1"));
    assert!(app.canvas.selected_nodes.contains("srl1"));
}

#[test]
fn test_shift_drag_starting_on_node_transitions_to_box_selection() {
    let mut app = App::new(None, None, true);
    app.active_tab = ActiveTab::Canvas;
    let area = Rect::new(0, 0, 120, 40);
    app.last_area.set(area);

    // 1. Shift + MouseDown directly on srl1 node
    app.handle_mouse(MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: 20,
        row: 14,
        modifiers: KeyModifiers::SHIFT,
    });
    // Initially enters PanningCanvas per legacy click constraint
    assert_eq!(app.canvas.mode, CanvasMode::PanningCanvas);

    // 2. Dragging with Shift transitions into BoxSelection even when started on node
    app.handle_mouse(MouseEvent {
        kind: MouseEventKind::Drag(MouseButton::Left),
        column: 80,
        row: 21,
        modifiers: KeyModifiers::SHIFT,
    });
    assert_eq!(app.canvas.mode, CanvasMode::BoxSelection);
    assert!(app.canvas.selected_nodes.contains("srl1"));
    assert!(app.canvas.selected_nodes.contains("srl2"));

    // 3. MouseUp completes selection
    app.handle_mouse(MouseEvent {
        kind: MouseEventKind::Up(MouseButton::Left),
        column: 80,
        row: 21,
        modifiers: KeyModifiers::NONE,
    });
    assert_eq!(app.canvas.mode, CanvasMode::Normal);
    assert_eq!(app.canvas.selected_nodes.len(), 2);
}

#[test]
fn test_tab_switch_clears_selection_box() {
    let (tx, _rx) = unbounded_channel();
    let mut app = App::new(None, None, true);
    app.active_tab = ActiveTab::Canvas;
    let area = Rect::new(0, 0, 120, 40);
    app.last_area.set(area);

    // Start a box drag
    app.handle_mouse(MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: 10,
        row: 10,
        modifiers: KeyModifiers::ALT,
    });
    assert_eq!(app.canvas.mode, CanvasMode::BoxSelection);
    assert!(app.canvas.selection_box.is_some());

    // Switch tab to Logs ('3')
    app.handle_key(key_event(KeyCode::Char('3')), &tx);
    assert_eq!(app.active_tab, ActiveTab::Logs);
    assert_eq!(app.canvas.mode, CanvasMode::Normal);
    assert!(app.canvas.selection_box.is_none());
}
