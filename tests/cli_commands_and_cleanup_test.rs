use clab_tui::clab::commands::{
    default_command_tree, CliArg, CliCategory, CliCommand, CliFocusedPane, CliViewState,
};
use clab_tui::clab::mock::MockClabClient;
use clab_tui::clab::parser::TopologyParser;
use clab_tui::event::AppEvent;
use clab_tui::ui::layout::ActiveTab;
use clab_tui::App;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use ratatui::backend::TestBackend;
use ratatui::Terminal;
use std::path::PathBuf;
use std::time::Duration;
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

#[tokio::test]
async fn test_deploy_modal_with_cleanup_and_reconfigure_toggles() {
    let (tx, mut rx) = unbounded_channel();
    let mut app = App::new(None, None, true);

    // Press 'd' to open deploy prompt modal
    app.handle_key(key_event(KeyCode::Char('d')), &tx);
    assert!(app.confirm_modal.is_open);
    assert_eq!(app.confirm_modal.action_name, "deploy");
    assert!(app.confirm_modal.show_cleanup_toggle);
    assert!(app.confirm_modal.show_reconfigure_toggle);
    assert!(!app.confirm_modal.cleanup);
    assert!(!app.confirm_modal.reconfigure);

    // Toggle cleanup with 'c'
    app.handle_key(key_event(KeyCode::Char('c')), &tx);
    assert!(app.confirm_modal.cleanup);

    // Toggle reconfigure with 'r'
    app.handle_key(key_event(KeyCode::Char('r')), &tx);
    assert!(app.confirm_modal.reconfigure);

    // Confirm deploy with 'y'
    app.handle_key(key_event(KeyCode::Char('y')), &tx);
    assert!(!app.confirm_modal.is_open);
    assert!(app.is_operating);
    assert_eq!(app.active_tab, ActiveTab::Logs);

    // Receive deploy finished event
    let mut received_finished = false;
    while let Ok(evt) = tokio::time::timeout(Duration::from_millis(1500), rx.recv()).await {
        if let Some(AppEvent::DeployFinished(success)) = evt {
            assert!(success);
            received_finished = true;
            break;
        }
    }
    assert!(received_finished);
}

#[tokio::test]
async fn test_destroy_modal_with_cleanup_toggle() {
    let (tx, mut rx) = unbounded_channel();
    let mut app = App::new(None, None, true);

    // Press 'D' (Shift+D) to open destroy prompt modal
    app.handle_key(key_event(KeyCode::Char('D')), &tx);
    assert!(app.confirm_modal.is_open);
    assert_eq!(app.confirm_modal.action_name, "destroy");
    assert!(app.confirm_modal.show_cleanup_toggle);
    assert!(!app.confirm_modal.show_reconfigure_toggle);
    assert!(app.confirm_modal.cleanup); // default true for destroy

    // Toggle cleanup with 'c' -> false
    app.handle_key(key_event(KeyCode::Char('c')), &tx);
    assert!(!app.confirm_modal.cleanup);

    // Toggle cleanup back with 'c' -> true
    app.handle_key(key_event(KeyCode::Char('c')), &tx);
    assert!(app.confirm_modal.cleanup);

    // Confirm destroy with Enter
    app.handle_key(key_event(KeyCode::Enter), &tx);
    assert!(!app.confirm_modal.is_open);
    assert!(app.is_operating);

    // Receive destroy finished event
    let mut received_finished = false;
    while let Ok(evt) = tokio::time::timeout(Duration::from_millis(1500), rx.recv()).await {
        if let Some(AppEvent::DestroyFinished(success)) = evt {
            assert!(success);
            received_finished = true;
            break;
        }
    }
    assert!(received_finished);
}

#[tokio::test]
async fn test_confirm_modal_mouse_toggle() {
    let (tx, _rx) = unbounded_channel();
    let mut app = App::new(None, None, true);

    app.prompt_deploy();
    assert!(app.confirm_modal.is_open);
    assert!(!app.confirm_modal.cleanup);
    assert!(!app.confirm_modal.reconfigure);

    // Calculate modal position
    let area = app.last_area.get();
    let width = 62.min(area.width.saturating_sub(4));
    let modal_x = area.x + (area.width.saturating_sub(width)) / 2;
    let height = 10.min(area.height.saturating_sub(2));
    let modal_y = area.y + (area.height.saturating_sub(height)) / 2;

    // Click cleanup row (modal_y + 4)
    app.handle_mouse(MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: modal_x + 5,
        row: modal_y + 4,
        modifiers: KeyModifiers::NONE,
    });
    assert!(app.confirm_modal.cleanup);

    // Click reconfigure row (modal_y + 5)
    app.handle_mouse(MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: modal_x + 5,
        row: modal_y + 5,
        modifiers: KeyModifiers::NONE,
    });
    assert!(app.confirm_modal.reconfigure);

    // Cancel modal with 'n'
    app.handle_key(key_event(KeyCode::Char('n')), &tx);
    assert!(!app.confirm_modal.is_open);
}

#[tokio::test]
async fn test_mock_simulate_deploy_and_destroy_options() {
    let mock = MockClabClient::new();
    let topo = TopologyParser::create_sample_topology();
    let (log_tx, mut log_rx) = unbounded_channel();

    // Run deploy simulation with cleanup=true, reconfigure=true
    let deploy_res = mock.simulate_deploy(&topo, true, true, log_tx).await;
    assert!(deploy_res.is_ok());

    let mut collected = Vec::new();
    while let Ok(line) = log_rx.try_recv() {
        collected.push(line);
    }
    assert!(collected
        .iter()
        .any(|l| l.contains("Removing previous lab artifacts")));
    assert!(collected
        .iter()
        .any(|l| l.contains("Reconfiguring existing containers")));
    assert!(collected
        .iter()
        .any(|l| l.contains("successfully deployed")));

    // Run destroy simulation with cleanup=true
    let (log_tx2, mut log_rx2) = unbounded_channel();
    let destroy_res = mock
        .simulate_destroy(&PathBuf::from("test.clab.yml"), true, log_tx2)
        .await;
    assert!(destroy_res.is_ok());

    let mut destroy_logs = Vec::new();
    while let Ok(line) = log_rx2.try_recv() {
        destroy_logs.push(line);
    }
    assert!(destroy_logs
        .iter()
        .any(|l| l.contains("Removing lab directory")));
    assert!(destroy_logs
        .iter()
        .any(|l| l.contains("successfully destroyed")));
}

#[test]
fn test_default_command_tree_completeness() {
    let tree = default_command_tree(Some("/path/to/my-lab.clab.yml"), Some("srl1"));

    // Verify count of commands: 9 core + 7 tools = 16 commands
    assert_eq!(tree.len(), 16);

    let names: Vec<&str> = tree.iter().map(|c| c.name.as_str()).collect();

    // Verify Core commands
    assert!(names.contains(&"deploy"));
    assert!(names.contains(&"destroy"));
    assert!(names.contains(&"redeploy"));
    assert!(names.contains(&"inspect"));
    assert!(names.contains(&"graph"));
    assert!(names.contains(&"save"));
    assert!(names.contains(&"version"));
    assert!(names.contains(&"exec"));
    assert!(names.contains(&"config"));

    // Verify Tools subcommands
    assert!(names.contains(&"tools capture"));
    assert!(names.contains(&"tools veth"));
    assert!(names.contains(&"tools disable-tx-offload"));
    assert!(names.contains(&"tools vxlan"));
    assert!(names.contains(&"tools netem"));
    assert!(names.contains(&"tools cert"));
    assert!(names.contains(&"tools api-server"));

    // Verify deploy flags: --topo, --cleanup, --reconfigure
    let deploy_cmd = tree.iter().find(|c| c.name == "deploy").unwrap();
    assert_eq!(deploy_cmd.category, CliCategory::Core);
    let deploy_flags: Vec<&str> = deploy_cmd.args.iter().map(|a| a.flag.as_str()).collect();
    assert!(deploy_flags.contains(&"--topo"));
    assert!(deploy_flags.contains(&"--cleanup"));
    assert!(deploy_flags.contains(&"--reconfigure"));
    assert!(deploy_flags.contains(&"--max-workers"));

    // Verify destroy flags: --topo, --cleanup, --all
    let destroy_cmd = tree.iter().find(|c| c.name == "destroy").unwrap();
    let destroy_flags: Vec<&str> = destroy_cmd.args.iter().map(|a| a.flag.as_str()).collect();
    assert!(destroy_flags.contains(&"--topo"));
    assert!(destroy_flags.contains(&"--cleanup"));
    assert!(destroy_flags.contains(&"--all"));

    // Verify tools capture flags: --topo, --node, --interface
    let capture_cmd = tree.iter().find(|c| c.name == "tools capture").unwrap();
    assert_eq!(capture_cmd.category, CliCategory::Tools);
    let capture_flags: Vec<&str> = capture_cmd.args.iter().map(|a| a.flag.as_str()).collect();
    assert!(capture_flags.contains(&"--topo"));
    assert!(capture_flags.contains(&"--node"));
    assert!(capture_flags.contains(&"--interface"));
}

#[test]
fn test_cli_command_tokens_and_preview_generation() {
    let mut cmd = CliCommand::new(
        "deploy",
        CliCategory::Core,
        "Deploy a lab topology",
        vec![
            CliArg::value(
                "--topo",
                Some("-t"),
                "Topology file",
                "path",
                "/tmp/lab.clab.yml",
                true,
            ),
            CliArg::flag("--cleanup", Some("-c"), "Cleanup", false),
            CliArg::flag("--reconfigure", None, "Reconfigure", false),
        ],
    );

    // Initial tokens: ["deploy", "--topo", "/tmp/lab.clab.yml"]
    assert_eq!(cmd.tokens(), vec!["deploy", "--topo", "/tmp/lab.clab.yml"]);
    assert_eq!(
        cmd.preview_string("containerlab"),
        "containerlab deploy --topo /tmp/lab.clab.yml"
    );

    // Enable --cleanup flag
    cmd.args[1].enabled = true;
    assert_eq!(
        cmd.tokens(),
        vec!["deploy", "--topo", "/tmp/lab.clab.yml", "--cleanup"]
    );
    assert_eq!(
        cmd.preview_string("containerlab"),
        "containerlab deploy --topo /tmp/lab.clab.yml --cleanup"
    );

    // Enable --reconfigure flag
    cmd.args[2].enabled = true;
    assert_eq!(
        cmd.preview_string("containerlab"),
        "containerlab deploy --topo /tmp/lab.clab.yml --cleanup --reconfigure"
    );

    // Quoting test: parameter with spaces
    let exec_cmd = CliCommand::new(
        "exec",
        CliCategory::Core,
        "Execute command",
        vec![CliArg::value(
            "--cmd",
            None,
            "Command string",
            "cmd",
            "uname -a -s",
            true,
        )],
    );
    assert_eq!(
        exec_cmd.preview_string("containerlab"),
        "containerlab exec --cmd \"uname -a -s\""
    );
}

#[tokio::test]
async fn test_cli_state_navigation_and_editing() {
    let mut state = CliViewState::new(Some("topo.clab.yml"), Some("srl1"));

    // Default focused pane is Commands
    assert_eq!(state.focused_pane, CliFocusedPane::Commands);
    assert_eq!(state.selected_command_idx, 0);
    assert_eq!(state.selected_command().name, "deploy");

    // Move next command -> destroy
    state.select_next_command();
    assert_eq!(state.selected_command().name, "destroy");

    // Move prev command -> deploy
    state.select_prev_command();
    assert_eq!(state.selected_command().name, "deploy");

    // Switch pane to Arguments
    state.focused_pane = state.focused_pane.next();
    assert_eq!(state.focused_pane, CliFocusedPane::Arguments);

    // Navigate arguments
    assert_eq!(state.selected_arg_idx, 0); // --topo
    state.select_next_arg();
    assert_eq!(state.selected_arg_idx, 1); // --cleanup
    assert_eq!(state.selected_arg().unwrap().flag, "--cleanup");
    assert!(!state.selected_arg().unwrap().enabled);

    // Toggle --cleanup flag
    state.toggle_selected_arg();
    assert!(state.selected_arg().unwrap().enabled);

    // Switch back to --topo and edit its value
    state.select_prev_arg();
    assert_eq!(state.selected_arg().unwrap().flag, "--topo");

    state.start_editing_selected_arg();
    assert!(state.is_editing_arg);
    assert_eq!(state.edit_buffer, "topo.clab.yml");

    // Edit value
    state.edit_buffer.push_str(".custom");
    state.apply_editing_arg();
    assert!(!state.is_editing_arg);
    assert_eq!(state.selected_arg().unwrap().value, "topo.clab.yml.custom");

    // Switch pane to Output
    state.focused_pane = state.focused_pane.next();
    assert_eq!(state.focused_pane, CliFocusedPane::Output);

    // Test output push and clear
    state.push_output_line("Line 1".to_string());
    state.push_output_line("Line 2".to_string());
    assert!(state.output_lines.iter().any(|l| l == "Line 1"));
    state.clear_output();
    assert!(state.output_lines.is_empty());
}

#[tokio::test]
async fn test_headless_tui_tab_5_switching_and_rendering() {
    let backend = TestBackend::new(120, 40);
    let mut terminal = Terminal::new(backend).unwrap();
    let (tx, _rx) = unbounded_channel();
    let mut app = App::new(None, None, true);

    // Switch to Tab 5 via key '5'
    app.handle_key(key_event(KeyCode::Char('5')), &tx);
    assert_eq!(app.active_tab, ActiveTab::Cli);

    // Clear initial toast notification so it does not overlay the tab header
    app.toast_mgr.toasts.clear();
    terminal.draw(|f| app.render(f)).unwrap();

    // Verify key UI zones are present in the buffer
    assert!(buffer_contains(&terminal, "CLI Command Tree"));
    assert!(buffer_contains(&terminal, "Core Commands"));
    assert!(buffer_contains(&terminal, "Tools Subcommands"));
    assert!(buffer_contains(&terminal, "Flags & Arguments"));
    assert!(buffer_contains(&terminal, "5: CLI Commands"));
}

#[tokio::test]
async fn test_cli_tab_keyboard_shortcuts_and_execution() {
    let (tx, mut rx) = unbounded_channel();
    let mut app = App::new(None, None, true);

    // Switch to CLI tab
    app.handle_key(key_event(KeyCode::Char('5')), &tx);
    assert_eq!(app.active_tab, ActiveTab::Cli);

    // Navigate to 'version' command: deploy(0)->destroy(1)->redeploy(2)->inspect(3)->graph(4)->save(5)->version(6)
    for _ in 0..6 {
        app.handle_key(key_event(KeyCode::Char('j')), &tx);
    }
    assert_eq!(app.cli_state.selected_command().name, "version");

    // Press 'r' to execute version command
    app.handle_key(key_event(KeyCode::Char('r')), &tx);
    assert!(app.cli_state.is_running);

    // Receive CliLog and CliCommandFinished events
    let mut received_finished = false;
    while let Ok(evt) = tokio::time::timeout(Duration::from_millis(1500), rx.recv()).await {
        if let Some(event) = evt {
            app.handle_event(event.clone(), &tx);
            if let AppEvent::CliCommandFinished {
                ref cmd_name,
                success,
            } = event
            {
                assert_eq!(cmd_name, "version");
                assert!(success);
                received_finished = true;
                break;
            }
        }
    }
    assert!(received_finished);
    assert!(!app.cli_state.is_running);
    assert_eq!(app.cli_state.last_status, Some(true));
    assert!(app
        .cli_state
        .output_lines
        .iter()
        .any(|l| l.contains("version: 0.79.0")));
}

#[tokio::test]
async fn test_cli_tab_inline_param_editing_flow() {
    let (tx, _rx) = unbounded_channel();
    let mut app = App::new(None, None, true);

    app.handle_key(key_event(KeyCode::Char('5')), &tx);
    assert_eq!(app.active_tab, ActiveTab::Cli);

    // Switch focus to Arguments pane (Tab)
    app.handle_key(key_event(KeyCode::Tab), &tx);
    assert_eq!(app.cli_state.focused_pane, CliFocusedPane::Arguments);
    assert_eq!(app.cli_state.selected_arg_idx, 0); // --topo

    // Press 'e' to start inline edit
    app.handle_key(key_event(KeyCode::Char('e')), &tx);
    assert!(app.cli_state.is_editing_arg);

    // Clear buffer with backspaces
    while !app.cli_state.edit_buffer.is_empty() {
        app.handle_key(key_event(KeyCode::Backspace), &tx);
    }

    // Type "new-topo.yml"
    for c in "new-topo.yml".chars() {
        app.handle_key(key_event(KeyCode::Char(c)), &tx);
    }
    assert_eq!(app.cli_state.edit_buffer, "new-topo.yml");

    // Press Enter to apply
    app.handle_key(key_event(KeyCode::Enter), &tx);
    assert!(!app.cli_state.is_editing_arg);
    assert_eq!(app.cli_state.selected_arg().unwrap().value, "new-topo.yml");
    assert!(app
        .cli_state
        .preview_string("containerlab")
        .contains("new-topo.yml"));
}

#[tokio::test]
async fn test_cli_tab_mouse_interaction() {
    let mut app = App::new(None, None, true);
    app.active_tab = ActiveTab::Cli;

    let body = app.canvas_area();
    let tree_w = (body.width * 28 / 100)
        .clamp(24, 34)
        .min(body.width.saturating_sub(30));

    // Click inside Command Tree (left pane)
    app.handle_mouse(MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: body.x + 5,
        row: body.y + 5,
        modifiers: KeyModifiers::NONE,
    });
    assert_eq!(app.cli_state.focused_pane, CliFocusedPane::Commands);

    // Click inside Arguments (right pane, top)
    app.handle_mouse(MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: body.x + tree_w + 10,
        row: body.y + 4,
        modifiers: KeyModifiers::NONE,
    });
    assert_eq!(app.cli_state.focused_pane, CliFocusedPane::Arguments);

    // Click near checkbox in Arguments pane to toggle
    let initial_flag_state = app.cli_state.selected_command().args[1].enabled;
    app.cli_state.selected_arg_idx = 1;
    app.handle_mouse(MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: body.x + tree_w + 2,
        row: body.y + 4, // row index 1 (y+3 is index 0, y+4 is index 1)
        modifiers: KeyModifiers::NONE,
    });
    assert_ne!(
        app.cli_state.selected_command().args[1].enabled,
        initial_flag_state
    );
}

#[tokio::test]
async fn test_mock_command_simulation_all_tools() {
    let mock = MockClabClient::new();
    let tools_commands = vec![
        "tools capture",
        "tools veth",
        "tools disable-tx-offload",
        "tools vxlan",
        "tools netem",
        "tools cert",
        "tools api-server",
        "graph",
        "exec",
        "save",
        "config",
    ];

    for cmd in tools_commands {
        let (tx, mut rx) = unbounded_channel();
        let res = mock
            .simulate_command(cmd, &["--test".to_string()], tx)
            .await;
        assert!(res.is_ok(), "Failed simulating command {}", cmd);

        let mut lines = Vec::new();
        while let Ok(line) = rx.try_recv() {
            lines.push(line);
        }
        assert!(!lines.is_empty(), "No log output generated for {}", cmd);
    }
}

#[tokio::test]
async fn test_cli_tab_small_terminal_resilience() {
    let sizes = [(40, 12), (35, 10), (25, 8), (20, 6)];
    let (tx, _rx) = unbounded_channel();

    for (w, h) in sizes {
        let backend = TestBackend::new(w, h);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut app = App::new(None, None, true);
        app.active_tab = ActiveTab::Cli;
        app.toast_mgr.toasts.clear();

        // Must not panic on constrained terminal sizes
        let draw_res = terminal.draw(|f| app.render(f));
        assert!(
            draw_res.is_ok(),
            "Failed drawing on terminal size {}x{}",
            w,
            h
        );

        // Also test mouse click on small terminal to ensure no clamp panics
        app.handle_mouse(MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: w / 2,
            row: h / 2,
            modifiers: KeyModifiers::NONE,
        });

        // Test keypresses
        app.handle_key(key_event(KeyCode::Char('k')), &tx);
        app.handle_key(key_event(KeyCode::Char('j')), &tx);
    }
}

#[tokio::test]
async fn test_cli_command_switching_while_running() {
    let (tx, mut rx) = unbounded_channel();
    let mut app = App::new(None, None, true);
    app.active_tab = ActiveTab::Cli;

    // Command 0 is deploy
    assert_eq!(app.cli_state.selected_command().name, "deploy");

    // Execute deploy
    app.trigger_cli_command(&tx);
    assert!(app.cli_state.is_running);
    assert_eq!(app.cli_state.executing_cmd.as_deref(), Some("deploy"));

    // User navigates cursor to destroy (cmd 1) while deploy is running!
    app.cli_state.select_next_command();
    assert_eq!(app.cli_state.selected_command().name, "destroy");

    // Wait for deploy completion
    while let Ok(evt) = tokio::time::timeout(Duration::from_millis(1500), rx.recv()).await {
        if let Some(event) = evt {
            app.handle_event(event.clone(), &tx);
            if let AppEvent::CliCommandFinished {
                ref cmd_name,
                success,
            } = event
            {
                // Must be deploy that completed, not destroy!
                assert_eq!(cmd_name, "deploy");
                assert!(success);
                break;
            }
        }
    }

    // Deploy command in the command tree should be marked success
    let deploy_cmd = app
        .cli_state
        .commands
        .iter()
        .find(|c| c.name == "deploy")
        .unwrap();
    assert_eq!(deploy_cmd.last_status, Some(true));

    // Destroy command must NOT have been marked executed/success!
    let destroy_cmd = app
        .cli_state
        .commands
        .iter()
        .find(|c| c.name == "destroy")
        .unwrap();
    assert_eq!(destroy_cmd.last_status, None);
}

#[test]
fn test_cli_output_scrolling_direction() {
    let mut state = CliViewState::new(None, None);
    state.clear_output();

    for i in 0..30 {
        state.push_output_line(format!("Log line {}", i));
    }

    assert_eq!(state.output_scroll, 0);

    // Scroll up by 5 lines (visible height 10, total 30 -> max_scroll 20)
    state.scroll_up(5, 10);
    assert_eq!(state.output_scroll, 5);

    // Scroll up further
    state.scroll_up(10, 10);
    assert_eq!(state.output_scroll, 15);

    // Scroll up past max_scroll
    state.scroll_up(10, 10);
    assert_eq!(state.output_scroll, 20);

    // Scroll down towards bottom
    state.scroll_down(8);
    assert_eq!(state.output_scroll, 12);

    // Scroll down all the way to bottom
    state.scroll_down(20);
    assert_eq!(state.output_scroll, 0);
}

#[tokio::test]
async fn test_cli_tab_mouse_tree_selection() {
    let mut app = App::new(None, None, true);
    app.active_tab = ActiveTab::Cli;

    let body = app.canvas_area();

    // Click on Command Tree item (row 2: command 0 deploy, row 3: command 1 destroy)
    app.handle_mouse(MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: body.x + 5,
        row: body.y + 3, // row corresponding to destroy
        modifiers: KeyModifiers::NONE,
    });
    assert_eq!(app.cli_state.focused_pane, CliFocusedPane::Commands);
    assert_eq!(app.cli_state.selected_command().name, "destroy");

    // Click on row 4 (redeploy)
    app.handle_mouse(MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: body.x + 5,
        row: body.y + 4,
        modifiers: KeyModifiers::NONE,
    });
    assert_eq!(app.cli_state.selected_command().name, "redeploy");
}

#[tokio::test]
async fn test_cli_tab_mouse_wheel() {
    let mut app = App::new(None, None, true);
    app.active_tab = ActiveTab::Cli;
    app.cli_state.focused_pane = CliFocusedPane::Commands;
    assert_eq!(app.cli_state.selected_command_idx, 0);

    // Scroll down mouse wheel
    app.handle_mouse(MouseEvent {
        kind: MouseEventKind::ScrollDown,
        column: 10,
        row: 10,
        modifiers: KeyModifiers::NONE,
    });
    assert_eq!(app.cli_state.selected_command_idx, 1);

    // Scroll up mouse wheel
    app.handle_mouse(MouseEvent {
        kind: MouseEventKind::ScrollUp,
        column: 10,
        row: 10,
        modifiers: KeyModifiers::NONE,
    });
    assert_eq!(app.cli_state.selected_command_idx, 0);
}

#[tokio::test]
async fn test_operation_mutual_exclusion() {
    let (tx, _rx) = unbounded_channel();
    let mut app = App::new(None, None, true);

    // While CLI is running, trigger_deploy_with_opts is blocked
    app.cli_state.is_running = true;
    app.trigger_deploy_with_opts(&tx, false, false);
    assert!(!app.is_operating);
    assert!(app
        .toast_mgr
        .toasts
        .iter()
        .any(|t| t.message.contains("already in progress")));

    // While is_operating is true, trigger_cli_command is blocked
    app.cli_state.is_running = false;
    app.is_operating = true;
    app.trigger_cli_command(&tx);
    assert!(!app.cli_state.is_running);
}

#[test]
fn test_node_default_sync_with_custom_topology() {
    let mut app = App::new(None, None, true);
    // Replace nodes with custom node names
    app.topology.topology.nodes.clear();
    app.topology
        .topology
        .nodes
        .insert("leaf1".to_string(), Default::default());
    app.topology
        .topology
        .nodes
        .insert("spine1".to_string(), Default::default());

    app.sync_cli_topo_path();

    // Verify --node and --a-node updated to leaf1
    let save_cmd = app
        .cli_state
        .commands
        .iter()
        .find(|c| c.name == "save")
        .unwrap();
    let node_arg = save_cmd.args.iter().find(|a| a.flag == "--node").unwrap();
    assert_eq!(node_arg.value, "leaf1");

    // Verify tools veth --b-node updated to spine1
    let veth_cmd = app
        .cli_state
        .commands
        .iter()
        .find(|c| c.name == "tools veth")
        .unwrap();
    let b_node = veth_cmd.args.iter().find(|a| a.flag == "--b-node").unwrap();
    assert_eq!(b_node.value, "spine1");
}
