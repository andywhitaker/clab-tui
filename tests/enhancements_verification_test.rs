use clab_tui::canvas::render::CanvasRenderer;
use clab_tui::canvas::state::CanvasState;
use clab_tui::clab::commands::{CliArg, CliFocusedPane};
use clab_tui::clab::model::{LabTopology, TopologyData};
use clab_tui::clab::parser::TopologyParser;
use clab_tui::graphics::kitty::GraphicsProtocol;
use clab_tui::ui::layout::ActiveTab;
use clab_tui::ui::theme::Theme;
use clab_tui::ui::widgets::drawer::{DrawerField, InspectorDrawer};
use clab_tui::ui::widgets::file_browser::{FileBrowserModal, FileEntryType};
use clab_tui::ui::widgets::log_viewer::{classify_log_line, LogLevel};
use clab_tui::App;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::backend::TestBackend;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::Terminal;
use std::fs::File;
use std::io::Write;
use std::path::PathBuf;
use tokio::sync::mpsc::unbounded_channel;

fn key_event(code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, KeyModifiers::NONE)
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

#[test]
fn test_ascii_canvas_interface_label_pill_badges() {
    let sample = TopologyParser::create_sample_topology();
    let mut canvas = CanvasState::new();
    canvas.load_from_topology(&sample);

    let area = Rect::new(0, 0, 100, 30);
    let mut buf = Buffer::empty(area);
    let theme = Theme::tokyo_night();

    CanvasRenderer::render(&canvas, area, &mut buf, &theme);
    let content = buffer_to_string(&buf);

    // Verify wire interface pill badges rendered on ASCII canvas
    assert!(
        content.contains("[e1-1]") || content.contains("[eth1]") || content.contains("[e1-2]"),
        "ASCII canvas should render interface pill badges like [e1-1] or [eth1]. Content:\n{}",
        content
    );
}

#[tokio::test]
async fn test_ascii_universal_default_mode() {
    let app = App::new(None, None, true);

    assert_eq!(
        app.graphics_proto,
        GraphicsProtocol::BrailleUnicodeFallback,
        "Universal default graphics protocol should be BrailleUnicodeFallback"
    );

    let backend = TestBackend::new(120, 40);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal.draw(|f| app.render(f)).unwrap();

    assert!(!buffer_contains(&terminal, "[KITTY HI-RES ACTIVE]"));
    assert!(!buffer_contains(&terminal, "[Kitty: ON]"));
}

#[test]
fn test_linux_host_drawer_editing_small_terminal_resilience() {
    let mut topo = TopologyParser::create_sample_topology();
    let mut canvas = CanvasState::new();
    canvas.load_from_topology(&topo);
    let theme = Theme::tokyo_night();

    // Verify host1 exists and has default multitool image
    assert!(topo.topology.nodes.contains_key("host1"));
    let host1_node = topo.topology.nodes.get("host1").unwrap();
    assert_eq!(
        host1_node.image.as_deref(),
        Some("ghcr.io/srl-labs/network-multitool:latest")
    );

    let mut drawer = InspectorDrawer::new();
    drawer.open_for_node("host1");

    // Standard 80x24 terminal buffer: previously caused overflow panic in drawer
    let standard_area = Rect::new(0, 0, 80, 24);
    let mut standard_buf = Buffer::empty(standard_area);
    drawer.render(&canvas, &topo, standard_area, &mut standard_buf, &theme);

    // Constrained 60x15 terminal buffer: ensure strict vertical and horizontal clipping
    let small_area = Rect::new(0, 0, 60, 15);
    let mut small_buf = Buffer::empty(small_area);
    drawer.render(&canvas, &topo, small_area, &mut small_buf, &theme);

    // Edit fields on host1 without panic
    drawer.focused_field = DrawerField::Image;
    drawer.start_editing("ghcr.io/srl-labs/network-multitool:latest");
    drawer.edit_buffer.push_str("-custom-tag");
    drawer.apply_edit(&mut canvas, &mut topo);

    assert_eq!(
        topo.topology.nodes.get("host1").unwrap().image.as_deref(),
        Some("ghcr.io/srl-labs/network-multitool:latest-custom-tag")
    );

    // Cycle kind on host1
    let profiles = clab_tui::clab::model::NodeProfile::default_profiles();
    drawer.cycle_kind_with_profiles(&mut canvas, &mut topo, &profiles);

    // Render again to ensure state consistency
    drawer.render(&canvas, &topo, standard_area, &mut standard_buf, &theme);
}

#[test]
fn test_drawer_apply_edit_missing_topology_node() {
    let mut topo = LabTopology {
        name: "test-topo".to_string(),
        prefix: None,
        mgmt: None,
        topology: TopologyData::default(),
    };
    let mut canvas = CanvasState::new();
    let node_def = clab_tui::clab::model::NodeDefinition::default();
    canvas.nodes.insert(
        "unregistered_host".to_string(),
        clab_tui::canvas::node::CanvasNode::new("unregistered_host", &node_def, 10.0, 10.0),
    );
    assert!(!topo.topology.nodes.contains_key("unregistered_host"));

    let mut drawer = InspectorDrawer::new();
    drawer.open_for_node("unregistered_host");
    drawer.focused_field = DrawerField::Kind;
    drawer.start_editing("linux");
    drawer.apply_edit(&mut canvas, &mut topo);

    assert!(topo.topology.nodes.contains_key("unregistered_host"));
    assert_eq!(
        topo.topology
            .nodes
            .get("unregistered_host")
            .unwrap()
            .kind
            .as_deref(),
        Some("linux")
    );
}

#[test]
fn test_file_browser_modal_navigation_and_selection() {
    let temp_dir = std::env::temp_dir().join(format!("clab_fb_test_{}", std::process::id()));
    let _ = std::fs::create_dir_all(&temp_dir);

    let sub_dir = temp_dir.join("subfolder");
    let _ = std::fs::create_dir_all(&sub_dir);

    let topo_file = temp_dir.join("demo.clab.yml");
    {
        let mut f = File::create(&topo_file).unwrap();
        writeln!(f, "name: demo").unwrap();
    }

    let regular_file = temp_dir.join("notes.txt");
    {
        let mut f = File::create(&regular_file).unwrap();
        writeln!(f, "notes").unwrap();
    }

    let mut fb = FileBrowserModal::new();
    fb.open(Some(&temp_dir));
    assert!(fb.is_open);
    assert_eq!(fb.current_dir, temp_dir);

    // Entries should contain parent .., subfolder, demo.clab.yml, and notes.txt
    assert!(fb
        .entries
        .iter()
        .any(|e| e.name == ".." && e.entry_type == FileEntryType::ParentDir));
    assert!(fb
        .entries
        .iter()
        .any(|e| e.name == "subfolder" && e.entry_type == FileEntryType::Directory));
    assert!(fb
        .entries
        .iter()
        .any(|e| e.name == "demo.clab.yml" && e.entry_type == FileEntryType::TopologyFile));
    assert!(fb
        .entries
        .iter()
        .any(|e| e.name == "notes.txt" && e.entry_type == FileEntryType::OtherFile));

    // Select demo.clab.yml
    let topo_idx = fb
        .entries
        .iter()
        .position(|e| e.name == "demo.clab.yml")
        .unwrap();
    fb.selected_index = topo_idx;
    let activated = fb.activate_selected();
    assert_eq!(activated, Some(topo_file));
    assert!(!fb.is_open, "Selecting a file should close the modal");

    // Clean up
    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[tokio::test]
async fn test_cli_tab_file_browser_modal_trigger_and_apply() {
    let (tx, _rx) = unbounded_channel();
    let mut app = App::new(None, None, true);

    // Switch to CLI tab
    app.handle_key(key_event(KeyCode::Char('5')), &tx);
    assert_eq!(app.active_tab, ActiveTab::Cli);

    // Switch to Arguments pane
    app.handle_key(key_event(KeyCode::Tab), &tx);
    assert_eq!(app.cli_state.focused_pane, CliFocusedPane::Arguments);
    assert_eq!(app.cli_state.selected_arg_idx, 0); // --topo
    assert!(app.cli_state.selected_arg().unwrap().is_file_arg());

    // Press 'b' on file arg -> directly opens interactive file browser modal
    app.handle_key(key_event(KeyCode::Char('b')), &tx);
    assert!(
        app.file_browser.is_open,
        "Pressing 'b' on file arg should open file_browser"
    );

    // Render with file browser modal open
    let backend = TestBackend::new(100, 30);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal.draw(|f| app.render(f)).unwrap();
    assert!(buffer_contains(&terminal, "Select File / Topology"));

    // Press Esc to cancel modal
    app.handle_key(key_event(KeyCode::Esc), &tx);
    assert!(
        !app.file_browser.is_open,
        "Esc should close file_browser modal"
    );

    // Press Enter on file arg -> also directly opens interactive file browser modal
    app.handle_key(key_event(KeyCode::Enter), &tx);
    assert!(
        app.file_browser.is_open,
        "Pressing Enter on file arg should open file_browser"
    );

    // Simulate selecting an entry
    let dummy_path = PathBuf::from("/tmp/custom_lab.clab.yml");
    app.file_browser.entries.insert(
        0,
        clab_tui::ui::widgets::file_browser::FileBrowserEntry {
            name: "custom_lab.clab.yml".to_string(),
            path: dummy_path.clone(),
            entry_type: FileEntryType::TopologyFile,
            is_dir: false,
        },
    );
    app.file_browser.selected_index = 0;

    app.handle_key(key_event(KeyCode::Enter), &tx);
    assert!(!app.file_browser.is_open);
    assert_eq!(
        app.cli_state.selected_arg().unwrap().value,
        dummy_path.to_string_lossy()
    );
    assert!(app.cli_state.selected_arg().unwrap().enabled);

    // Non-file argument (e.g. --max-workers): 'e' should start inline text editing
    app.cli_state.selected_arg_idx = 3; // --max-workers
    assert!(!app.cli_state.selected_arg().unwrap().is_file_arg());
    app.handle_key(key_event(KeyCode::Char('e')), &tx);
    assert!(
        app.cli_state.is_editing_arg,
        "'e' on non-file arg should start inline edit"
    );
    assert!(
        !app.file_browser.is_open,
        "file_browser should not open for non-file arg"
    );
    app.handle_key(key_event(KeyCode::Char('8')), &tx);
    app.handle_key(key_event(KeyCode::Enter), &tx);
    assert!(!app.cli_state.is_editing_arg);
    assert_eq!(app.cli_state.selected_arg().unwrap().value, "8");
}

#[test]
fn test_cli_arg_is_file_arg_detection() {
    let topo_arg = CliArg::value(
        "--topo",
        Some("-t"),
        "Topology file path",
        "PATH",
        "lab.clab.yml",
        true,
    );
    assert!(topo_arg.is_file_arg());

    let template_arg = CliArg::value("--template", None, "Template file path", "FILE", "", false);
    assert!(template_arg.is_file_arg());

    let flag_arg = CliArg::flag("--reconfigure", Some("-c"), "Reconfigure nodes", false);
    assert!(!flag_arg.is_file_arg());

    let node_filter = CliArg::value("--node-filter", None, "Filter nodes", "NAME", "srl1", false);
    assert!(!node_filter.is_file_arg());
}

#[test]
fn test_log_line_classifier_categories() {
    // Info and progress logs (NOT errors!)
    assert_eq!(
        classify_log_line("INFO[0000] Parsing & validating topology: srl.clab.yml"),
        LogLevel::Info
    );
    assert_eq!(
        classify_log_line("INFO[0001] Creating docker network: clab"),
        LogLevel::Info
    );
    assert_eq!(
        classify_log_line("INFO[0002] Creating container: clab-srl-srl1"),
        LogLevel::Info
    );
    assert_eq!(
        classify_log_line("+---+-------------------+--------------+"),
        LogLevel::Info
    );
    assert_eq!(
        classify_log_line("| # |       Name        |    Image     |"),
        LogLevel::Info
    );
    assert_eq!(classify_log_line("Pulling fs layer"), LogLevel::Info);
    assert_eq!(
        classify_log_line("Downloading [========================>] 12.3MB"),
        LogLevel::Info
    );
    assert_eq!(
        classify_log_line("Digest: sha256:abcd1234ef5678"),
        LogLevel::Info
    );
    assert_eq!(
        classify_log_line("Status: Image is up to date"),
        LogLevel::Info
    );

    // Stderr stripped info lines must not be colored error
    assert_eq!(
        classify_log_line("[STDERR] INFO[0000] Parsing topology"),
        LogLevel::Info
    );
    assert_eq!(classify_log_line("[STDERR]Already exists"), LogLevel::Info);
    assert_eq!(classify_log_line("Download complete"), LogLevel::Info);
    assert_eq!(classify_log_line("Verifying Checksum"), LogLevel::Info);
    assert_eq!(
        classify_log_line("Pulling from srl-labs/network-multitool"),
        LogLevel::Info
    );

    // Warnings
    assert_eq!(
        classify_log_line("WARN[0000] Interface pattern exceeded"),
        LogLevel::Warning
    );
    assert_eq!(
        classify_log_line("Warning: Container already exists"),
        LogLevel::Warning
    );

    // Successes
    assert_eq!(
        classify_log_line("Lab successfully deployed!"),
        LogLevel::Success
    );
    assert_eq!(
        classify_log_line("[SUCCESS] All containers running"),
        LogLevel::Success
    );
    assert_eq!(
        classify_log_line("Lab successfully destroyed"),
        LogLevel::Success
    );

    // Genuine Errors
    assert_eq!(
        classify_log_line("ERRO[0001] failed to create container: name collision"),
        LogLevel::Error
    );
    assert_eq!(
        classify_log_line("FATAL[0000] permission denied"),
        LogLevel::Error
    );
    assert_eq!(
        classify_log_line("Error: topology file not found"),
        LogLevel::Error
    );
    assert_eq!(
        classify_log_line("[FAILED] Deployment aborted"),
        LogLevel::Error
    );

    // Normal / Shell preview
    assert_eq!(
        classify_log_line("$ containerlab deploy -t lab.clab.yml"),
        LogLevel::Normal
    );
}

#[tokio::test]
async fn test_file_browser_keyboard_and_mouse_interaction() {
    let (tx, _rx) = unbounded_channel();
    let mut app = App::new(None, None, true);

    // Switch to CLI tab, Arguments pane
    app.handle_key(key_event(KeyCode::Char('5')), &tx);
    app.handle_key(key_event(KeyCode::Tab), &tx);
    app.cli_state.selected_arg_idx = 0; // --topo

    // Test mouse click on file arg value to open file browser
    let backend = TestBackend::new(120, 40);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal.draw(|f| app.render(f)).unwrap();

    let body = app.canvas_area();
    let tree_width = if body.width >= 60 {
        (body.width * 28 / 100)
            .clamp(24, 34)
            .min(body.width.saturating_sub(30))
    } else {
        (body.width * 35 / 100)
            .max(12)
            .min(body.width.saturating_sub(15))
    };
    let click_col = body.x + tree_width + 10;
    let click_row = body.y + 3; // row for first argument (--topo)

    app.handle_mouse(crossterm::event::MouseEvent {
        kind: crossterm::event::MouseEventKind::Down(crossterm::event::MouseButton::Left),
        column: click_col,
        row: click_row,
        modifiers: KeyModifiers::NONE,
    });
    assert!(
        app.file_browser.is_open,
        "Clicking file arg value should open file browser"
    );

    // Test 'q' to close modal
    app.handle_key(key_event(KeyCode::Char('q')), &tx);
    assert!(
        !app.file_browser.is_open,
        "'q' should close file browser modal"
    );

    // Open again via Enter
    app.handle_key(key_event(KeyCode::Enter), &tx);
    assert!(
        app.file_browser.is_open,
        "Enter on file arg should open file browser modal"
    );

    // Test Left / 'h' to navigate up
    let orig_dir = app.file_browser.current_dir.clone();
    app.handle_key(key_event(KeyCode::Char('h')), &tx);
    if let Some(parent) = orig_dir.parent() {
        assert_eq!(app.file_browser.current_dir, parent);
    }
}

#[test]
fn test_drawer_delete_and_rename_robustness() {
    let mut topo = TopologyParser::create_sample_topology();
    let mut canvas = CanvasState::new();
    canvas.load_from_topology(&topo);

    assert!(canvas.nodes.contains_key("host2"));
    let mut drawer = InspectorDrawer::new();
    drawer.open_for_node("host2");

    // Test rename
    drawer.focused_field = DrawerField::Name;
    drawer.start_editing("host2");
    drawer.edit_buffer = "host2-renamed".to_string();
    let res = drawer.apply_edit(&mut canvas, &mut topo);
    assert!(res.is_some());
    assert!(canvas.nodes.contains_key("host2-renamed"));
    assert!(!canvas.nodes.contains_key("host2"));
    assert!(topo.topology.nodes.contains_key("host2-renamed"));

    // Test delete
    drawer.open_for_node("host2-renamed");
    drawer.focused_field = DrawerField::DeleteNode;
    let del_res = drawer.apply_edit(&mut canvas, &mut topo);
    assert!(del_res.is_some());
    assert!(!canvas.nodes.contains_key("host2-renamed"));
    assert!(!drawer.is_open);
}
