use crate::canvas::state::{CanvasMode, CanvasState};
use crate::clab::client::ClabClient;
use crate::clab::commands::{CliArgKind, CliFocusedPane, CliViewState};
use crate::clab::mock::MockClabClient;
use crate::clab::model::{LabTopology, NodeProfile};
use crate::clab::parser::TopologyParser;
use crate::event::AppEvent;
use crate::graphics::kitty::GraphicsProtocol;
use crate::ui::layout::{ActiveTab, AppLayout, ConfirmModal};
use crate::ui::theme::Theme;
use crate::ui::views::canvas_view::{
    AddNodeModal, CanvasView, CanvasViewParams, LinkEditField, LinkEditModal, ProfileEditModal,
    ProfileField,
};
use crate::ui::views::cli_view::CliView;
use crate::ui::views::help_popup::HelpPopup;
use crate::ui::views::inspect_view::{InspectView, InspectViewState};
use crate::ui::views::logs_view::LogsView;
use crate::ui::views::yaml_view::{YamlView, YamlViewState};
use crate::ui::widgets::drawer::{DrawerField, InspectorDrawer};
use crate::ui::widgets::file_browser::FileBrowserModal;
use crate::ui::widgets::log_viewer::LogViewer;
use crate::ui::widgets::toast::ToastManager;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use ratatui::layout::Rect;
use ratatui::Frame;
use std::path::PathBuf;
use tokio::sync::mpsc::UnboundedSender;

pub struct App {
    pub topology: LabTopology,
    pub topology_path: Option<PathBuf>,
    pub canvas: CanvasState,
    pub active_tab: ActiveTab,
    pub theme: Theme,
    pub toast_mgr: ToastManager,
    pub drawer: InspectorDrawer,
    pub add_modal: AddNodeModal,
    pub profile_modal: ProfileEditModal,
    pub link_edit_modal: LinkEditModal,
    pub node_profiles: Vec<NodeProfile>,
    pub confirm_modal: ConfirmModal,
    pub file_browser: FileBrowserModal,
    pub log_viewer: LogViewer,
    pub inspect_state: InspectViewState,
    pub yaml_state: YamlViewState,
    pub cli_state: CliViewState,

    pub clab_client: ClabClient,
    pub mock_client: MockClabClient,
    pub is_mock: bool,

    pub graphics_proto: GraphicsProtocol,
    pub show_help: bool,
    pub running: bool,
    pub is_operating: bool,
    pub last_area: std::cell::Cell<Rect>,
}

impl App {
    pub fn new(topo: Option<LabTopology>, topo_path: Option<PathBuf>, mock: bool) -> Self {
        let topology = topo.unwrap_or_else(TopologyParser::create_sample_topology);
        let mut canvas = CanvasState::new();
        canvas.load_from_topology(&topology);

        let topo_path = topo_path.map(|p| {
            if p.is_absolute() {
                p
            } else if let Ok(cwd) = std::env::current_dir() {
                cwd.join(p)
            } else {
                p
            }
        });

        let mut yaml_state = YamlViewState::default();
        yaml_state.update(&topology);

        let default_topo_str = topo_path
            .as_ref()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|| format!("{}.clab.yml", topology.name));
        let default_node = topology.topology.nodes.keys().next().map(|s| s.as_str());
        let cli_state = CliViewState::new(Some(&default_topo_str), default_node);

        let graphics_proto = GraphicsProtocol::BrailleUnicodeFallback;

        let mut toast_mgr = ToastManager::new();
        toast_mgr.info("Welcome to clab-tui! Press '?' for help");

        Self {
            topology,
            topology_path: topo_path,
            canvas,
            active_tab: ActiveTab::Canvas,
            theme: Theme::tokyo_night(),
            toast_mgr,
            drawer: InspectorDrawer::new(),
            add_modal: AddNodeModal::default(),
            profile_modal: ProfileEditModal::default(),
            link_edit_modal: LinkEditModal::default(),
            node_profiles: NodeProfile::default_profiles(),
            confirm_modal: ConfirmModal::default(),
            file_browser: FileBrowserModal::new(),
            log_viewer: LogViewer::new(),
            inspect_state: InspectViewState::default(),
            yaml_state,
            cli_state,
            clab_client: ClabClient::new(),
            mock_client: MockClabClient::new(),
            is_mock: mock,
            graphics_proto,
            show_help: false,
            running: true,
            is_operating: false,
            last_area: std::cell::Cell::new(
                crossterm::terminal::size()
                    .map(|(w, h)| Rect::new(0, 0, w, h))
                    .unwrap_or_else(|_| Rect::new(0, 0, 120, 40)),
            ),
        }
    }

    /// Calculate canvas body viewport area from the last known frame area
    pub fn canvas_area(&self) -> Rect {
        let area = self.last_area.get();
        Rect::new(
            area.x,
            area.y + 3,
            area.width,
            area.height.saturating_sub(4),
        )
    }

    /// Calculate visible YAML content lines from the last known frame area
    pub fn yaml_visible_lines(&self) -> usize {
        let area = self.last_area.get();
        let body_h = area.height.saturating_sub(4);
        let footer_rows = if self.yaml_state.error_msg.is_some() {
            2
        } else {
            0
        };
        (body_h.saturating_sub(2 + footer_rows) as usize).max(1)
    }

    /// Update internal state periodically on tick
    pub fn on_tick(&mut self) {
        self.toast_mgr.tick();
    }

    /// Handle incoming multiplexed application event
    pub fn handle_event(&mut self, event: AppEvent, tx: &UnboundedSender<AppEvent>) {
        match event {
            AppEvent::Key(key) => self.handle_key(key, tx),
            AppEvent::Mouse(mouse) => self.handle_mouse(mouse),
            AppEvent::Resize(w, h) => {
                self.last_area.set(Rect::new(0, 0, w, h));
            }
            AppEvent::Tick => self.on_tick(),
            AppEvent::Log(line) => {
                self.log_viewer.push_line(line);
            }
            AppEvent::InspectUpdated(containers) => {
                self.inspect_state.containers = containers;
                self.inspect_state.is_loading = false;
                self.toast_mgr.success("Container inspect refreshed");
            }
            AppEvent::DeployFinished(success) => {
                self.is_operating = false;
                if success {
                    self.toast_mgr.success("Lab deployed successfully!");
                    self.trigger_inspect(tx);
                } else {
                    self.toast_mgr.error("Lab deployment encountered errors");
                }
            }
            AppEvent::DestroyFinished(success) => {
                self.is_operating = false;
                if success {
                    self.toast_mgr.success("Lab destroyed successfully!");
                    self.inspect_state.containers.clear();
                } else {
                    let mut toast_err =
                        "Lab teardown failed (see Logs tab for details)".to_string();
                    for line in self.log_viewer.lines.iter().rev().take(15) {
                        let lower = line.to_lowercase();
                        if lower.contains("does not exist") || lower.contains("no such file") {
                            toast_err = "Teardown failed: Topology file does not exist".to_string();
                            break;
                        } else if lower.contains("permission denied")
                            || lower.contains("unprivileged")
                            || lower.contains("elevated privileges")
                            || lower.contains("password is required")
                        {
                            toast_err =
                                "Teardown failed: Permission denied (root or sudo required)"
                                    .to_string();
                            break;
                        }
                    }
                    self.toast_mgr.error(toast_err);
                }
            }
            AppEvent::CliLog(line) => {
                self.cli_state.push_output_line(line.clone());
                self.log_viewer.push_line(line);
            }
            AppEvent::CliCommandFinished { cmd_name, success } => {
                self.cli_state.is_running = false;
                self.cli_state.executing_cmd = None;
                self.cli_state.last_status = Some(success);
                if let Some(cmd) = self
                    .cli_state
                    .commands
                    .iter_mut()
                    .find(|c| c.name == cmd_name)
                {
                    cmd.last_status = Some(success);
                }
                if success {
                    self.toast_mgr
                        .success(format!("Command '{}' completed successfully", cmd_name));
                    self.cli_state.push_output_line(format!(
                        "[SUCCESS] Command '{}' completed successfully.",
                        cmd_name
                    ));
                    if cmd_name == "deploy" || cmd_name == "redeploy" {
                        self.trigger_inspect(tx);
                    } else if cmd_name == "destroy" {
                        self.inspect_state.containers.clear();
                    }
                } else {
                    self.toast_mgr
                        .error(format!("Command '{}' failed", cmd_name));
                    self.cli_state.push_output_line(format!(
                        "[FAILED] Command '{}' exited with error.",
                        cmd_name
                    ));
                }
            }
        }
    }

    /// Handle keyboard input
    pub fn handle_key(&mut self, key: KeyEvent, tx: &UnboundedSender<AppEvent>) {
        // Global quit check
        if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
            self.running = false;
            return;
        }

        // Help popup active
        if self.show_help {
            if key.code == KeyCode::Esc
                || key.code == KeyCode::Char('?')
                || key.code == KeyCode::Char('q')
            {
                self.show_help = false;
            }
            return;
        }

        // Confirm modal active
        if self.confirm_modal.is_open {
            match key.code {
                KeyCode::Char('y') | KeyCode::Char('Y') | KeyCode::Enter => {
                    self.confirm_modal.is_open = false;
                    if self.confirm_modal.action_name == "destroy" {
                        self.trigger_destroy_with_opts(tx, self.confirm_modal.cleanup);
                    } else if self.confirm_modal.action_name == "deploy" {
                        self.trigger_deploy_with_opts(
                            tx,
                            self.confirm_modal.reconfigure,
                            self.confirm_modal.cleanup,
                        );
                    }
                }
                KeyCode::Char('c') | KeyCode::Char('C') => {
                    if self.confirm_modal.show_cleanup_toggle {
                        self.confirm_modal.cleanup = !self.confirm_modal.cleanup;
                    }
                }
                KeyCode::Char('r') | KeyCode::Char('R') => {
                    if self.confirm_modal.show_reconfigure_toggle {
                        self.confirm_modal.reconfigure = !self.confirm_modal.reconfigure;
                    }
                }
                KeyCode::Char(' ') => {
                    if self.confirm_modal.show_cleanup_toggle {
                        self.confirm_modal.cleanup = !self.confirm_modal.cleanup;
                    }
                }
                KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => {
                    self.confirm_modal.is_open = false;
                    self.toast_mgr.info("Action cancelled");
                }
                _ => {}
            }
            return;
        }

        // File Browser modal active
        if self.file_browser.is_open {
            match key.code {
                KeyCode::Esc | KeyCode::Char('q') => {
                    self.file_browser.close();
                }
                KeyCode::Up | KeyCode::Char('k') => {
                    self.file_browser.select_prev();
                }
                KeyCode::Down | KeyCode::Char('j') => {
                    self.file_browser.select_next();
                }
                KeyCode::Left | KeyCode::Char('h') | KeyCode::Backspace => {
                    self.file_browser.navigate_up();
                }
                KeyCode::Right | KeyCode::Char('l') => {
                    if let Some(entry) = self
                        .file_browser
                        .entries
                        .get(self.file_browser.selected_index)
                    {
                        if entry.is_dir {
                            self.file_browser.activate_selected();
                        }
                    }
                }
                KeyCode::Enter => {
                    if let Some(path) = self.file_browser.activate_selected() {
                        let path_str = path.to_string_lossy().to_string();
                        let sel_idx = self.cli_state.selected_arg_idx;
                        let cmd = self.cli_state.selected_command_mut();
                        if let Some(arg) = cmd.args.get_mut(sel_idx) {
                            arg.value = path_str.clone();
                            arg.enabled = true;
                        }
                        self.cli_state.is_editing_arg = false;
                        self.cli_state.edit_buffer.clear();
                        self.toast_mgr.info(format!("Selected file: {}", path_str));
                    }
                }
                _ => {}
            }
            return;
        }

        // Link Edit modal active
        if self.link_edit_modal.is_open {
            match key.code {
                KeyCode::Esc => {
                    self.link_edit_modal.close();
                }
                KeyCode::Tab | KeyCode::Down => {
                    self.link_edit_modal.focused_field = self.link_edit_modal.focused_field.next();
                }
                KeyCode::BackTab | KeyCode::Up => {
                    self.link_edit_modal.focused_field = self.link_edit_modal.focused_field.prev();
                }
                KeyCode::Enter => {
                    if self.link_edit_modal.focused_field == LinkEditField::Cancel {
                        self.link_edit_modal.close();
                    } else {
                        let _ = self.save_link_from_modal();
                    }
                }
                KeyCode::Backspace => {
                    match self.link_edit_modal.focused_field {
                        LinkEditField::SourcePort => {
                            self.link_edit_modal.source_port.pop();
                        }
                        LinkEditField::TargetPort => {
                            self.link_edit_modal.target_port.pop();
                        }
                        _ => {}
                    }
                    self.link_edit_modal.error_msg = None;
                }
                KeyCode::Char(c) => {
                    match self.link_edit_modal.focused_field {
                        LinkEditField::SourcePort => {
                            self.link_edit_modal.source_port.push(c);
                        }
                        LinkEditField::TargetPort => {
                            self.link_edit_modal.target_port.push(c);
                        }
                        _ => {}
                    }
                    self.link_edit_modal.error_msg = None;
                }
                _ => {}
            }
            return;
        }

        // Profile modal active
        if self.profile_modal.is_open {
            if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('s') {
                let _ = self.save_profile_from_modal();
                return;
            }

            match key.code {
                KeyCode::Esc => {
                    self.profile_modal.is_open = false;
                }
                KeyCode::Tab | KeyCode::Down => {
                    self.profile_modal.focused_field = self.profile_modal.focused_field.next();
                }
                KeyCode::BackTab | KeyCode::Up => {
                    self.profile_modal.focused_field = self.profile_modal.focused_field.prev();
                }
                KeyCode::Enter => {
                    if self.profile_modal.focused_field == ProfileField::Cancel {
                        self.profile_modal.is_open = false;
                    } else {
                        let _ = self.save_profile_from_modal();
                    }
                }
                KeyCode::Char(' ')
                    if self.profile_modal.focused_field == ProfileField::Category =>
                {
                    self.profile_modal.category = self.profile_modal.category.next();
                }
                KeyCode::Backspace => {
                    match self.profile_modal.focused_field {
                        ProfileField::KindName => {
                            self.profile_modal.kind_name.pop();
                        }
                        ProfileField::DisplayName => {
                            self.profile_modal.display_name.pop();
                        }
                        ProfileField::Image => {
                            self.profile_modal.image.pop();
                        }
                        ProfileField::Ports => {
                            self.profile_modal.ports.pop();
                        }
                        ProfileField::InterfacePattern => {
                            self.profile_modal.interface_pattern.pop();
                        }
                        ProfileField::Description => {
                            self.profile_modal.description.pop();
                        }
                        _ => {}
                    }
                    self.profile_modal.error_msg = None;
                }
                KeyCode::Char(c) => {
                    match self.profile_modal.focused_field {
                        ProfileField::KindName => {
                            self.profile_modal.kind_name.push(c);
                        }
                        ProfileField::DisplayName => {
                            self.profile_modal.display_name.push(c);
                        }
                        ProfileField::Image => {
                            self.profile_modal.image.push(c);
                        }
                        ProfileField::Ports => {
                            self.profile_modal.ports.push(c);
                        }
                        ProfileField::InterfacePattern => {
                            self.profile_modal.interface_pattern.push(c);
                        }
                        ProfileField::Description => {
                            self.profile_modal.description.push(c);
                        }
                        _ => {}
                    }
                    self.profile_modal.error_msg = None;
                }
                _ => {}
            }
            return;
        }

        // Add node modal active
        if self.add_modal.is_open {
            match key.code {
                KeyCode::Up | KeyCode::Char('k') => {
                    if self.add_modal.selected_index > 0 {
                        self.add_modal.selected_index -= 1;
                    } else {
                        self.add_modal.selected_index = self.node_profiles.len().saturating_sub(1);
                    }
                }
                KeyCode::Down | KeyCode::Char('j') => {
                    if !self.node_profiles.is_empty() {
                        self.add_modal.selected_index =
                            (self.add_modal.selected_index + 1) % self.node_profiles.len();
                    }
                }
                KeyCode::Char('n') => {
                    self.profile_modal.open_for_new();
                }
                KeyCode::Char('e') => {
                    if let Some(prof) = self.node_profiles.get(self.add_modal.selected_index) {
                        self.profile_modal
                            .open_for_edit(self.add_modal.selected_index, prof);
                    }
                }
                KeyCode::Char('d') | KeyCode::Char('x') | KeyCode::Delete => {
                    if self.node_profiles.len() > 1 {
                        let deleted = self.node_profiles.remove(self.add_modal.selected_index);
                        if self.add_modal.selected_index >= self.node_profiles.len() {
                            self.add_modal.selected_index =
                                self.node_profiles.len().saturating_sub(1);
                        }
                        self.toast_mgr
                            .warning(format!("Deleted profile '{}'", deleted.kind_name));
                    } else {
                        self.toast_mgr
                            .error("Cannot delete the last remaining node profile");
                    }
                }
                KeyCode::Enter => {
                    if let Some(profile) = self
                        .node_profiles
                        .get(self.add_modal.selected_index)
                        .cloned()
                    {
                        let name = self.canvas.add_node_from_profile(
                            &profile,
                            self.add_modal.spawn_pos.0,
                            self.add_modal.spawn_pos.1,
                        );
                        self.sync_canvas_to_model();
                        self.toast_mgr
                            .success(format!("Added node '{}' ({})", name, profile.kind_name));
                        self.add_modal.is_open = false;
                    }
                }
                KeyCode::Esc | KeyCode::Char('q') => {
                    self.add_modal.is_open = false;
                }
                _ => {}
            }
            return;
        }

        // Inspector drawer active
        if self.drawer.is_open {
            if self.drawer.is_editing {
                match key.code {
                    KeyCode::Enter => {
                        if let Some(msg) = self.drawer.apply_edit_with_profiles(
                            &mut self.canvas,
                            &mut self.topology,
                            &self.node_profiles,
                        ) {
                            self.toast_mgr.success(msg);
                            self.sync_canvas_to_model();
                        }
                    }
                    KeyCode::Esc => {
                        self.drawer.is_editing = false;
                        self.drawer.kind_selector_open = false;
                        self.drawer.edit_buffer.clear();
                    }
                    KeyCode::Backspace => {
                        self.drawer.edit_buffer.pop();
                        if self.drawer.focused_field == DrawerField::Kind {
                            self.drawer
                                .sync_kind_selector_from_buffer(&self.node_profiles);
                        }
                    }
                    KeyCode::Delete => {
                        self.drawer.edit_buffer.clear();
                        if self.drawer.focused_field == DrawerField::Kind {
                            self.drawer
                                .sync_kind_selector_from_buffer(&self.node_profiles);
                        }
                    }
                    KeyCode::Up | KeyCode::BackTab => {
                        if self.drawer.focused_field == DrawerField::Kind {
                            self.drawer.select_prev_kind_profile(&self.node_profiles);
                        }
                    }
                    KeyCode::Down | KeyCode::Tab => {
                        if self.drawer.focused_field == DrawerField::Kind {
                            self.drawer.select_next_kind_profile(&self.node_profiles);
                        }
                    }
                    KeyCode::Char('u') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                        self.drawer.edit_buffer.clear();
                        if self.drawer.focused_field == DrawerField::Kind {
                            self.drawer
                                .sync_kind_selector_from_buffer(&self.node_profiles);
                        }
                    }
                    KeyCode::Char('w') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                        self.drawer.edit_buffer.clear();
                        if self.drawer.focused_field == DrawerField::Kind {
                            self.drawer
                                .sync_kind_selector_from_buffer(&self.node_profiles);
                        }
                    }
                    KeyCode::Char(c) if !key.modifiers.contains(KeyModifiers::CONTROL) => {
                        self.drawer.edit_buffer.push(c);
                        if self.drawer.focused_field == DrawerField::Kind {
                            self.drawer
                                .sync_kind_selector_from_buffer(&self.node_profiles);
                        }
                    }
                    _ => {}
                }
                return;
            }

            match key.code {
                KeyCode::Up | KeyCode::Char('k') => {
                    self.drawer.focused_field = self.drawer.focused_field.prev();
                }
                KeyCode::Down | KeyCode::Char('j') => {
                    self.drawer.focused_field = self.drawer.focused_field.next();
                }
                KeyCode::Char(' ') => {
                    if self.drawer.focused_field == DrawerField::Kind {
                        self.drawer.cycle_kind_with_profiles(
                            &mut self.canvas,
                            &mut self.topology,
                            &self.node_profiles,
                        );
                        self.sync_canvas_to_model();
                        self.toast_mgr.info("Cycled node kind template");
                    }
                }
                KeyCode::Delete | KeyCode::Char('x') => {
                    let deleted = if !self.canvas.selected_nodes.is_empty() {
                        self.canvas.remove_selected_nodes()
                    } else if let Some(deleted) = self.canvas.remove_selected_node() {
                        vec![deleted]
                    } else if let Some(target) = self.drawer.target_node_name.clone() {
                        self.canvas.selected_node = Some(target);
                        self.canvas.remove_selected_node().into_iter().collect()
                    } else {
                        Vec::new()
                    };
                    self.drawer.close();
                    self.sync_canvas_to_model();
                    if deleted.len() == 1 {
                        self.toast_mgr
                            .warning(format!("Deleted node '{}'", deleted[0]));
                    } else if !deleted.is_empty() {
                        self.toast_mgr.warning(format!(
                            "Deleted {} nodes: {}",
                            deleted.len(),
                            deleted.join(", ")
                        ));
                    }
                }
                KeyCode::Enter => match self.drawer.focused_field {
                    DrawerField::DeleteNode => {
                        if let Some(msg) =
                            self.drawer.apply_edit(&mut self.canvas, &mut self.topology)
                        {
                            self.toast_mgr.warning(msg);
                            self.sync_canvas_to_model();
                        }
                    }
                    DrawerField::Kind => {
                        let curr = if let Some(node_name) = &self.drawer.target_node_name {
                            self.canvas
                                .nodes
                                .get(node_name)
                                .map(|n| n.kind.clone())
                                .unwrap_or_default()
                        } else {
                            String::new()
                        };
                        self.drawer.start_editing_kind(&curr, &self.node_profiles);
                    }
                    _ => {
                        let curr = if let Some(node_name) = &self.drawer.target_node_name {
                            match self.drawer.focused_field {
                                DrawerField::Name => node_name.clone(),
                                DrawerField::Image => self
                                    .canvas
                                    .nodes
                                    .get(node_name)
                                    .map(|n| n.image.clone())
                                    .unwrap_or_default(),
                                DrawerField::MgmtIpv4 => self
                                    .topology
                                    .topology
                                    .nodes
                                    .get(node_name)
                                    .and_then(|d| d.mgmt_ipv4.clone())
                                    .unwrap_or_default(),
                                DrawerField::InterfacePattern => self
                                    .canvas
                                    .nodes
                                    .get(node_name)
                                    .map(|n| n.interface_pattern.clone())
                                    .unwrap_or_default(),
                                _ => String::new(),
                            }
                        } else {
                            String::new()
                        };
                        self.drawer.start_editing(&curr);
                    }
                },
                KeyCode::Esc | KeyCode::Char('q') => {
                    self.drawer.close();
                }
                _ => {}
            }
            return;
        }

        // If in YAML tab and editing, dispatch directly to handle_yaml_key so global shortcuts do not interfere
        if self.active_tab == ActiveTab::Yaml && self.yaml_state.is_editing {
            self.handle_yaml_key(key, tx);
            return;
        }

        // If in CLI tab and editing argument, dispatch directly to handle_cli_edit_key
        if self.active_tab == ActiveTab::Cli && self.cli_state.is_editing_arg {
            self.handle_cli_edit_key(key);
            return;
        }

        // If in CLI tab, Tab / BackTab switches panes inside the CLI tab
        if self.active_tab == ActiveTab::Cli
            && (key.code == KeyCode::Tab || key.code == KeyCode::BackTab)
            && !key.modifiers.contains(KeyModifiers::CONTROL)
        {
            self.handle_cli_key(key, tx);
            return;
        }

        let prev_tab = self.active_tab;

        // Global shortcuts
        match key.code {
            KeyCode::Char('1') => self.active_tab = ActiveTab::Canvas,
            KeyCode::Char('2') => {
                self.active_tab = ActiveTab::Inspect;
                self.trigger_inspect(tx);
            }
            KeyCode::Char('3') => self.active_tab = ActiveTab::Logs,
            KeyCode::Char('4') => {
                self.active_tab = ActiveTab::Yaml;
                self.yaml_state.update(&self.topology);
            }
            KeyCode::Char('5') => {
                self.active_tab = ActiveTab::Cli;
                self.sync_cli_topo_path();
            }
            KeyCode::Tab => {
                if key.modifiers.contains(KeyModifiers::SHIFT) {
                    self.active_tab = self.active_tab.prev();
                } else {
                    self.active_tab = self.active_tab.next();
                }
                if self.active_tab == ActiveTab::Yaml && !self.yaml_state.is_editing {
                    self.yaml_state.update(&self.topology);
                }
                if self.active_tab == ActiveTab::Cli {
                    self.sync_cli_topo_path();
                }
            }
            KeyCode::Char('?') | KeyCode::F(1) => {
                self.show_help = true;
            }
            KeyCode::Char('q') => {
                if self.canvas.mode == CanvasMode::Wiring {
                    self.canvas.cancel_wiring();
                } else {
                    self.running = false;
                }
            }
            KeyCode::Esc => {
                if self.canvas.mode == CanvasMode::Wiring {
                    self.canvas.cancel_wiring();
                    self.toast_mgr.info("Wiring cancelled");
                } else if self.canvas.mode == CanvasMode::BoxSelection {
                    self.canvas.mode = CanvasMode::Normal;
                    self.canvas.selection_box = None;
                } else if self.active_tab == ActiveTab::Canvas {
                    self.canvas.selected_nodes.clear();
                    self.canvas.selected_node = None;
                    self.canvas.selected_link = None;
                }
            }
            _ => {
                // Tab-specific keybindings
                match self.active_tab {
                    ActiveTab::Canvas => self.handle_canvas_key(key, tx),
                    ActiveTab::Inspect => self.handle_inspect_key(key, tx),
                    ActiveTab::Logs => self.handle_logs_key(key),
                    ActiveTab::Yaml => self.handle_yaml_key(key, tx),
                    ActiveTab::Cli => self.handle_cli_key(key, tx),
                }
            }
        }

        if prev_tab == ActiveTab::Canvas
            && self.active_tab != ActiveTab::Canvas
            && self.canvas.selection_box.is_some()
        {
            self.canvas.mode = CanvasMode::Normal;
            self.canvas.selection_box = None;
        }
    }

    fn handle_canvas_key(&mut self, key: KeyEvent, _tx: &UnboundedSender<AppEvent>) {
        if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('s') {
            self.save_topology();
            return;
        }

        let is_pan_mod = key.modifiers.contains(KeyModifiers::SHIFT)
            || key.modifiers.contains(KeyModifiers::CONTROL);
        match key.code {
            // Movement or Panning (Vim & Arrow keys)
            KeyCode::Left => {
                if is_pan_mod || self.canvas.selected_node.is_none() {
                    self.canvas.pan(4.0, 0.0);
                } else {
                    self.canvas.move_selected_node(-2.0, 0.0);
                    self.sync_canvas_to_model();
                }
            }
            KeyCode::Right => {
                if is_pan_mod || self.canvas.selected_node.is_none() {
                    self.canvas.pan(-4.0, 0.0);
                } else {
                    self.canvas.move_selected_node(2.0, 0.0);
                    self.sync_canvas_to_model();
                }
            }
            KeyCode::Up => {
                if is_pan_mod || self.canvas.selected_node.is_none() {
                    self.canvas.pan(0.0, 4.0);
                } else {
                    self.canvas.move_selected_node(0.0, -2.0);
                    self.sync_canvas_to_model();
                }
            }
            KeyCode::Down => {
                if is_pan_mod || self.canvas.selected_node.is_none() {
                    self.canvas.pan(0.0, -4.0);
                } else {
                    self.canvas.move_selected_node(0.0, 2.0);
                    self.sync_canvas_to_model();
                }
            }
            KeyCode::Char('h') => {
                if is_pan_mod || self.canvas.selected_node.is_none() {
                    self.canvas.pan(4.0, 0.0);
                } else {
                    self.canvas.move_selected_node(-2.0, 0.0);
                    self.sync_canvas_to_model();
                }
            }
            KeyCode::Char('l') => {
                if is_pan_mod || self.canvas.selected_node.is_none() {
                    self.canvas.pan(-4.0, 0.0);
                } else {
                    self.canvas.move_selected_node(2.0, 0.0);
                    self.sync_canvas_to_model();
                }
            }
            KeyCode::Char('k') => {
                if is_pan_mod || self.canvas.selected_node.is_none() {
                    self.canvas.pan(0.0, 4.0);
                } else {
                    self.canvas.move_selected_node(0.0, -2.0);
                    self.sync_canvas_to_model();
                }
            }
            KeyCode::Char('j') => {
                if is_pan_mod || self.canvas.selected_node.is_none() {
                    self.canvas.pan(0.0, -4.0);
                } else {
                    self.canvas.move_selected_node(0.0, 2.0);
                    self.sync_canvas_to_model();
                }
            }
            // Panning (Uppercase vim keys)
            KeyCode::Char('H') => self.canvas.pan(4.0, 0.0),
            KeyCode::Char('L') => self.canvas.pan(-4.0, 0.0),
            KeyCode::Char('K') => self.canvas.pan(0.0, 4.0),
            KeyCode::Char('J') => self.canvas.pan(0.0, -4.0),

            // Zoom
            KeyCode::Char('+') | KeyCode::Char('=') => {
                self.canvas.zoom_in();
                self.toast_mgr.info(format!(
                    "Zoom: {}%",
                    (self.canvas.zoom * 100.0).round() as u32
                ));
            }
            KeyCode::Char('-') => {
                self.canvas.zoom_out();
                self.toast_mgr.info(format!(
                    "Zoom: {}%",
                    (self.canvas.zoom * 100.0).round() as u32
                ));
            }
            KeyCode::Char('0') => {
                self.canvas.reset_view();
                self.toast_mgr.info("View reset to 100%");
            }

            // Cycling nodes
            KeyCode::Char('n') => self.canvas.select_next_node(),
            KeyCode::Char('p') => self.canvas.select_prev_node(),

            // Cycling links
            KeyCode::Char('[') => self.canvas.select_prev_link(),
            KeyCode::Char(']') => self.canvas.select_next_link(),

            // Wiring
            KeyCode::Char('w') => {
                if self.canvas.mode == CanvasMode::Wiring {
                    self.canvas.cancel_wiring();
                    self.toast_mgr.info("Wiring cancelled");
                } else if self.canvas.start_wiring() {
                    self.toast_mgr
                        .info("Wiring mode: navigate/click target node port, or Esc to cancel");
                } else {
                    self.toast_mgr
                        .warning("Select a node first to start wiring");
                }
            }

            // Add Node
            KeyCode::Char('a') => {
                let center_x = -self.canvas.offset_x / self.canvas.zoom + 30.0;
                let center_y = -self.canvas.offset_y / self.canvas.zoom + 15.0;
                self.add_modal.spawn_pos = (center_x.max(5.0), center_y.max(5.0));
                self.add_modal.is_open = true;
            }

            // Node Profiles Modal (Shift+P)
            KeyCode::Char('P') => {
                let prof_idx = if let Some(ref node_name) = self.canvas.selected_node {
                    if let Some(node) = self.canvas.nodes.get(node_name) {
                        self.node_profiles.iter().position(|p| {
                            p.kind_name == node.kind || p.node_category == node.category
                        })
                    } else {
                        None
                    }
                } else {
                    None
                };
                let target_idx = prof_idx.unwrap_or(self.add_modal.selected_index);
                if let Some(prof) = self.node_profiles.get(target_idx) {
                    self.profile_modal.open_for_edit(target_idx, prof);
                } else {
                    self.profile_modal.open_for_new();
                }
            }

            // Edit Selected Link in Modal or Node in Drawer
            KeyCode::Char('e') => {
                if let Some(link_idx) = self.canvas.selected_link {
                    if let Some(link) = self.canvas.links.get(link_idx) {
                        self.link_edit_modal.open_for_link(
                            link_idx,
                            &link.source_node,
                            &link.source_port,
                            &link.target_node,
                            &link.target_port,
                        );
                    }
                } else if let Some(selected_name) = self.canvas.selected_node.clone() {
                    self.drawer.open_for_node(&selected_name);
                } else {
                    self.toast_mgr
                        .warning("Select a node or link to edit properties");
                }
            }

            // Clone Node
            KeyCode::Char('c') => {
                if let Some(new_name) = self.canvas.clone_selected_node() {
                    self.sync_canvas_to_model();
                    self.toast_mgr
                        .success(format!("Cloned node to '{}'", new_name));
                } else {
                    self.toast_mgr.warning("Select a node to clone");
                }
            }

            // Delete
            KeyCode::Delete | KeyCode::Backspace | KeyCode::Char('x') => {
                if self.canvas.selected_link.is_some() {
                    self.canvas.remove_selected_link();
                    self.sync_canvas_to_model();
                    self.toast_mgr.warning("Deleted wire link");
                } else if !self.canvas.selected_nodes.is_empty() {
                    let deleted = self.canvas.remove_selected_nodes();
                    if let Some(ref target) = self.drawer.target_node_name {
                        if deleted.contains(target) {
                            self.drawer.close();
                        }
                    }
                    if self.canvas.mode == CanvasMode::BoxSelection {
                        self.canvas.mode = CanvasMode::Normal;
                        self.canvas.selection_box = None;
                    }
                    self.sync_canvas_to_model();
                    if deleted.len() == 1 {
                        self.toast_mgr
                            .warning(format!("Deleted node '{}'", deleted[0]));
                    } else if !deleted.is_empty() {
                        self.toast_mgr.warning(format!(
                            "Deleted {} nodes: {}",
                            deleted.len(),
                            deleted.join(", ")
                        ));
                    }
                } else if let Some(deleted) = self.canvas.remove_selected_node() {
                    if self.drawer.target_node_name.as_deref() == Some(&deleted) {
                        self.drawer.close();
                    }
                    if self.canvas.mode == CanvasMode::BoxSelection {
                        self.canvas.mode = CanvasMode::Normal;
                        self.canvas.selection_box = None;
                    }
                    self.sync_canvas_to_model();
                    self.toast_mgr
                        .warning(format!("Deleted node '{}'", deleted));
                }
            }

            // Visual / Marquee Box Select Mode Toggle (v / b / s / Space)
            KeyCode::Char('v')
            | KeyCode::Char('V')
            | KeyCode::Char('b')
            | KeyCode::Char('B')
            | KeyCode::Char('s')
            | KeyCode::Char('S')
            | KeyCode::Char(' ') => {
                if key.kind == crossterm::event::KeyEventKind::Repeat
                    || (key.code == KeyCode::Char(' ') && self.canvas.selection_box.is_some())
                {
                    return;
                }

                if self.canvas.mode == CanvasMode::BoxSelection {
                    self.canvas.mode = CanvasMode::Normal;
                    self.canvas.selection_box = None;
                    self.toast_mgr.info("Visual selection mode off");
                } else {
                    self.canvas.mode = CanvasMode::BoxSelection;
                    self.toast_mgr.info(
                        "Visual / Box Select mode: click & drag to select nodes (v/b/s/Esc to exit)",
                    );
                }
            }

            // Cycle link routing style (Orthogonal -> Direct -> Octilinear)
            KeyCode::Char('r') | KeyCode::Char('R') => {
                let new_style = self.canvas.cycle_routing_style();
                self.sync_canvas_to_model();
                self.toast_mgr
                    .info(format!("Routing style: {}", new_style.name()));
            }

            // Operations
            KeyCode::Char('d') => {
                self.prompt_deploy();
            }
            KeyCode::Char('D') => {
                self.prompt_destroy();
            }

            _ => {}
        }
    }

    fn handle_inspect_key(&mut self, key: KeyEvent, tx: &UnboundedSender<AppEvent>) {
        match key.code {
            KeyCode::Up | KeyCode::Char('k') => self.inspect_state.select_prev(),
            KeyCode::Down | KeyCode::Char('j') => self.inspect_state.select_next(),
            KeyCode::Char('r') => self.trigger_inspect(tx),
            KeyCode::Char('d') => self.prompt_deploy(),
            KeyCode::Char('D') => self.prompt_destroy(),
            _ => {}
        }
    }

    fn handle_logs_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Up | KeyCode::Char('k') => self.log_viewer.scroll_up(2),
            KeyCode::Down | KeyCode::Char('j') => self.log_viewer.scroll_down(2, 25),
            KeyCode::PageUp => self.log_viewer.scroll_up(10),
            KeyCode::PageDown => self.log_viewer.scroll_down(10, 25),
            KeyCode::Char('c') => {
                self.log_viewer.clear();
                self.toast_mgr.info("Logs cleared");
            }
            KeyCode::Char(' ') => {
                self.log_viewer.auto_scroll = !self.log_viewer.auto_scroll;
                let tag = if self.log_viewer.auto_scroll {
                    "enabled"
                } else {
                    "paused"
                };
                self.toast_mgr.info(format!("Auto-scroll {}", tag));
            }
            _ => {}
        }
    }

    fn handle_yaml_key(&mut self, key: KeyEvent, _tx: &UnboundedSender<AppEvent>) {
        let visible = self.yaml_visible_lines();
        if self.yaml_state.is_editing {
            if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('s') {
                let _ = self.apply_yaml_edits();
                return;
            }

            match key.code {
                KeyCode::Esc => {
                    self.yaml_state.is_editing = false;
                    self.toast_mgr
                        .info("Exited YAML edit mode (Ctrl+S to apply, r to reload)");
                }
                KeyCode::Up => self.yaml_state.move_up(),
                KeyCode::Down => self.yaml_state.move_down(visible),
                KeyCode::Left => self.yaml_state.move_left(),
                KeyCode::Right => self.yaml_state.move_right(visible),
                KeyCode::Home => self.yaml_state.move_home(),
                KeyCode::End => self.yaml_state.move_end(),
                KeyCode::PageUp => self.yaml_state.scroll_up(10),
                KeyCode::PageDown => self.yaml_state.scroll_down(10, visible),
                KeyCode::Enter => self.yaml_state.insert_newline(visible),
                KeyCode::Backspace => self.yaml_state.backspace(),
                KeyCode::Delete => self.yaml_state.delete(),
                KeyCode::Tab => self.yaml_state.insert_tab(),
                KeyCode::Char(c) => self.yaml_state.insert_char(c),
                _ => {}
            }
        } else {
            if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('s') {
                let _ = self.apply_yaml_edits();
                return;
            }

            match key.code {
                KeyCode::Char('e') | KeyCode::Char('/') | KeyCode::Char('i') | KeyCode::Enter => {
                    self.yaml_state.start_editing();
                    self.toast_mgr
                        .info("YAML edit mode enabled. Press Ctrl+S to apply, Esc to exit.");
                }
                KeyCode::Up | KeyCode::Char('k') => self.yaml_state.scroll_up(2),
                KeyCode::Down | KeyCode::Char('j') => self.yaml_state.scroll_down(2, visible),
                KeyCode::PageUp => self.yaml_state.scroll_up(10),
                KeyCode::PageDown => self.yaml_state.scroll_down(10, visible),
                KeyCode::Char('s') => {
                    if self.yaml_state.get_text() != self.yaml_state.yaml_cache {
                        if self.apply_yaml_edits().is_ok() {
                            self.save_topology();
                        }
                    } else {
                        self.save_topology();
                    }
                }
                KeyCode::Char('r') => {
                    self.yaml_state.update(&self.topology);
                    self.toast_mgr.info("YAML reloaded from canvas model");
                }
                KeyCode::Char('d') => self.prompt_deploy(),
                KeyCode::Char('D') => self.prompt_destroy(),
                _ => {}
            }
        }
    }

    /// Handle mouse events
    pub fn handle_mouse(&mut self, mouse: MouseEvent) {
        if self.show_help || self.add_modal.is_open || self.profile_modal.is_open {
            return;
        }

        if self.drawer.is_open {
            let area = self.last_area.get();
            let drawer_width = 38.min(area.width.saturating_sub(4));
            let drawer_x = area.right().saturating_sub(drawer_width);
            let drawer_rect = Rect::new(drawer_x, area.y, drawer_width, area.height);

            if self.drawer.kind_selector_open && self.drawer.is_editing {
                if let Some(popup_rect) =
                    InspectorDrawer::popup_rect(area, drawer_rect, self.node_profiles.len())
                {
                    let list_top = popup_rect.y + 3;
                    let list_bottom = popup_rect.bottom().saturating_sub(2);
                    let max_visible = (list_bottom.saturating_sub(list_top) as usize).max(1);

                    match mouse.kind {
                        MouseEventKind::Down(MouseButton::Left) => {
                            if mouse.column >= popup_rect.x
                                && mouse.column < popup_rect.right()
                                && mouse.row >= list_top
                                && mouse.row < list_bottom
                            {
                                let scroll_offset =
                                    if self.drawer.kind_selector_index >= max_visible {
                                        self.drawer.kind_selector_index - max_visible + 1
                                    } else {
                                        0
                                    };
                                let row_offset = (mouse.row - list_top) as usize;
                                let target_idx = scroll_offset + row_offset;
                                if let Some(prof) = self.node_profiles.get(target_idx) {
                                    if self.drawer.kind_selector_index == target_idx {
                                        if let Some(msg) = self.drawer.apply_edit_with_profiles(
                                            &mut self.canvas,
                                            &mut self.topology,
                                            &self.node_profiles,
                                        ) {
                                            self.toast_mgr.success(msg);
                                            self.sync_canvas_to_model();
                                        }
                                    } else {
                                        self.drawer.kind_selector_index = target_idx;
                                        self.drawer.edit_buffer = prof.kind_name.clone();
                                    }
                                }
                            } else if mouse.column < popup_rect.x
                                || mouse.column >= popup_rect.right()
                                || mouse.row < popup_rect.y
                                || mouse.row >= popup_rect.bottom()
                            {
                                // Clicked outside popup: cancel/dismiss popup
                                self.drawer.is_editing = false;
                                self.drawer.kind_selector_open = false;
                                self.drawer.edit_buffer.clear();
                            }
                        }
                        MouseEventKind::ScrollUp => {
                            self.drawer.select_prev_kind_profile(&self.node_profiles);
                        }
                        MouseEventKind::ScrollDown => {
                            self.drawer.select_next_kind_profile(&self.node_profiles);
                        }
                        _ => {}
                    }
                }
            } else if let MouseEventKind::Down(MouseButton::Left) = mouse.kind {
                if mouse.column >= drawer_x && mouse.column < area.right() {
                    let field_top = area.y + 4;
                    if mouse.row >= field_top {
                        let field_idx = (mouse.row - field_top) / 3;
                        match field_idx {
                            0 => {
                                self.drawer.focused_field = DrawerField::Name;
                                let curr = self.drawer.target_node_name.clone().unwrap_or_default();
                                self.drawer.start_editing(&curr);
                            }
                            1 => {
                                let curr = if let Some(node_name) = &self.drawer.target_node_name {
                                    self.canvas
                                        .nodes
                                        .get(node_name)
                                        .map(|n| n.kind.clone())
                                        .unwrap_or_default()
                                } else {
                                    String::new()
                                };
                                self.drawer.start_editing_kind(&curr, &self.node_profiles);
                            }
                            2 => {
                                self.drawer.focused_field = DrawerField::Image;
                                let curr = if let Some(node_name) = &self.drawer.target_node_name {
                                    self.canvas
                                        .nodes
                                        .get(node_name)
                                        .map(|n| n.image.clone())
                                        .unwrap_or_default()
                                } else {
                                    String::new()
                                };
                                self.drawer.start_editing(&curr);
                            }
                            3 => {
                                self.drawer.focused_field = DrawerField::MgmtIpv4;
                                let curr = if let Some(node_name) = &self.drawer.target_node_name {
                                    self.topology
                                        .topology
                                        .nodes
                                        .get(node_name)
                                        .and_then(|d| d.mgmt_ipv4.clone())
                                        .unwrap_or_default()
                                } else {
                                    String::new()
                                };
                                self.drawer.start_editing(&curr);
                            }
                            4 => {
                                self.drawer.focused_field = DrawerField::InterfacePattern;
                                let curr = if let Some(node_name) = &self.drawer.target_node_name {
                                    self.canvas
                                        .nodes
                                        .get(node_name)
                                        .map(|n| n.interface_pattern.clone())
                                        .unwrap_or_default()
                                } else {
                                    String::new()
                                };
                                self.drawer.start_editing(&curr);
                            }
                            5 => {
                                self.drawer.focused_field = DrawerField::AddPort;
                                self.drawer.start_editing("");
                            }
                            6 => {
                                self.drawer.focused_field = DrawerField::DeleteNode;
                                if let Some(msg) =
                                    self.drawer.apply_edit(&mut self.canvas, &mut self.topology)
                                {
                                    self.toast_mgr.warning(msg);
                                    self.sync_canvas_to_model();
                                }
                            }
                            _ => {}
                        }
                    }
                } else {
                    // Clicked outside drawer: close drawer
                    self.drawer.close();
                }
            }
            return;
        }

        if self.file_browser.is_open {
            if let MouseEventKind::Down(MouseButton::Left) = mouse.kind {
                let area = self.last_area.get();
                let width = 74.min(area.width.saturating_sub(4));
                let height = 22.min(area.height.saturating_sub(2));
                let modal_x = area.x + (area.width.saturating_sub(width)) / 2;
                let modal_y = area.y + (area.height.saturating_sub(height)) / 2;

                if mouse.column >= modal_x
                    && mouse.column < modal_x + width
                    && mouse.row >= modal_y
                    && mouse.row < modal_y + height
                {
                    let list_y = modal_y + 3;
                    let list_h = height.saturating_sub(6) as usize;
                    if mouse.row >= list_y && (mouse.row as usize) < list_y as usize + list_h {
                        let click_row = (mouse.row - list_y) as usize;
                        let scroll = if self.file_browser.selected_index >= list_h {
                            self.file_browser.selected_index.saturating_sub(list_h - 1)
                        } else {
                            0
                        };
                        let target_idx = scroll + click_row;
                        if target_idx < self.file_browser.entries.len() {
                            if target_idx == self.file_browser.selected_index {
                                if let Some(path) = self.file_browser.activate_selected() {
                                    let path_str = path.to_string_lossy().to_string();
                                    let sel_idx = self.cli_state.selected_arg_idx;
                                    let cmd = self.cli_state.selected_command_mut();
                                    if let Some(arg) = cmd.args.get_mut(sel_idx) {
                                        arg.value = path_str.clone();
                                        arg.enabled = true;
                                    }
                                    self.cli_state.is_editing_arg = false;
                                    self.cli_state.edit_buffer.clear();
                                    self.toast_mgr.info(format!("Selected file: {}", path_str));
                                }
                            } else {
                                self.file_browser.selected_index = target_idx;
                            }
                        }
                    }
                } else {
                    self.file_browser.close();
                }
            } else if let MouseEventKind::ScrollUp = mouse.kind {
                self.file_browser.select_prev();
            } else if let MouseEventKind::ScrollDown = mouse.kind {
                self.file_browser.select_next();
            }
            return;
        }

        if self.link_edit_modal.is_open {
            if let MouseEventKind::Down(MouseButton::Left) = mouse.kind {
                let area = self.last_area.get();
                let modal_width = 64.min(area.width.saturating_sub(4));
                let modal_height = 14.min(area.height.saturating_sub(2));
                let modal_x = area.x + (area.width.saturating_sub(modal_width)) / 2;
                let modal_y = area.y + (area.height.saturating_sub(modal_height)) / 2;

                if mouse.column >= modal_x && mouse.column < modal_x + modal_width {
                    // Source port row: modal_y + 4
                    if mouse.row == modal_y + 4 {
                        self.link_edit_modal.focused_field = LinkEditField::SourcePort;
                        return;
                    }
                    // Target port row: modal_y + 6
                    if mouse.row == modal_y + 6 {
                        self.link_edit_modal.focused_field = LinkEditField::TargetPort;
                        return;
                    }
                    // Button row: modal_y + modal_height - 3
                    if mouse.row == modal_y + modal_height.saturating_sub(3) {
                        // Save button: modal_x + 3 .. modal_x + 26
                        if mouse.column >= modal_x + 3 && mouse.column < modal_x + 26 {
                            let _ = self.save_link_from_modal();
                            return;
                        }
                        // Cancel button: modal_x + 28 .. modal_x + 42
                        if mouse.column >= modal_x + 28 && mouse.column < modal_x + 42 {
                            self.link_edit_modal.close();
                            return;
                        }
                    }
                }
            }
            return;
        }

        if self.confirm_modal.is_open {
            if let MouseEventKind::Down(MouseButton::Left) = mouse.kind {
                let area = self.last_area.get();
                let has_options = self.confirm_modal.show_cleanup_toggle
                    || self.confirm_modal.show_reconfigure_toggle;
                let option_lines = (if self.confirm_modal.show_cleanup_toggle {
                    1
                } else {
                    0
                }) + (if self.confirm_modal.show_reconfigure_toggle {
                    1
                } else {
                    0
                });
                let width = 62.min(area.width.saturating_sub(4));
                let height = (if has_options { 8 + option_lines } else { 8 })
                    .min(area.height.saturating_sub(2));
                let modal_x = area.x + (area.width.saturating_sub(width)) / 2;
                let modal_y = area.y + (area.height.saturating_sub(height)) / 2;

                if mouse.column >= modal_x && mouse.column < modal_x + width {
                    let mut check_y = modal_y + 4;
                    if self.confirm_modal.show_cleanup_toggle {
                        if mouse.row == check_y {
                            self.confirm_modal.cleanup = !self.confirm_modal.cleanup;
                            return;
                        }
                        check_y += 1;
                    }
                    if self.confirm_modal.show_reconfigure_toggle && mouse.row == check_y {
                        self.confirm_modal.reconfigure = !self.confirm_modal.reconfigure;
                        return;
                    }
                }
            }
            return;
        }

        let area = self.last_area.get();
        let tabs_area = Rect::new(area.x, area.y + 1, area.width, 2);
        if mouse.kind == MouseEventKind::Down(MouseButton::Left)
            && mouse.row >= tabs_area.y
            && mouse.row < tabs_area.bottom()
            && mouse.column >= tabs_area.x
            && mouse.column < tabs_area.right()
        {
            let mut curr_x = tabs_area.x + 2;
            for tab in ActiveTab::ALL {
                let title_len = format!("  {}  ", tab.title()).len() as u16;
                if mouse.column >= curr_x && mouse.column < curr_x + title_len {
                    self.active_tab = tab;
                    if self.active_tab == ActiveTab::Yaml && !self.yaml_state.is_editing {
                        self.yaml_state.update(&self.topology);
                    }
                    if self.active_tab == ActiveTab::Cli {
                        self.sync_cli_topo_path();
                    }
                    return;
                }
                curr_x += title_len + 1;
            }
        }

        if self.active_tab == ActiveTab::Cli {
            match mouse.kind {
                MouseEventKind::ScrollUp => {
                    match self.cli_state.focused_pane {
                        CliFocusedPane::Commands => self.cli_state.select_prev_command(),
                        CliFocusedPane::Arguments => self.cli_state.select_prev_arg(),
                        CliFocusedPane::Output => self.cli_state.scroll_up(3, 20),
                    }
                    return;
                }
                MouseEventKind::ScrollDown => {
                    match self.cli_state.focused_pane {
                        CliFocusedPane::Commands => self.cli_state.select_next_command(),
                        CliFocusedPane::Arguments => self.cli_state.select_next_arg(),
                        CliFocusedPane::Output => self.cli_state.scroll_down(3),
                    }
                    return;
                }
                MouseEventKind::Down(MouseButton::Left) => {
                    let body = self.canvas_area();
                    if mouse.column >= body.x
                        && mouse.column < body.right()
                        && mouse.row >= body.y
                        && mouse.row < body.bottom()
                    {
                        let tree_width = if body.width >= 60 {
                            (body.width * 28 / 100)
                                .clamp(24, 34)
                                .min(body.width.saturating_sub(30))
                        } else {
                            (body.width * 35 / 100)
                                .max(12)
                                .min(body.width.saturating_sub(15))
                        };

                        if mouse.column < body.x + tree_width {
                            self.cli_state.cancel_editing_arg();
                            self.cli_state.focused_pane = CliFocusedPane::Commands;

                            let inner_y = body.y + 1;
                            let inner_h = body.height.saturating_sub(2) as usize;
                            if inner_h > 0
                                && mouse.row >= inner_y
                                && mouse.row < inner_y + inner_h as u16
                            {
                                let click_screen_row = (mouse.row - inner_y) as usize;
                                let mut rows_cmds: Vec<Option<usize>> = Vec::new();
                                rows_cmds.push(None);
                                for (i, cmd) in self.cli_state.commands.iter().enumerate() {
                                    if cmd.category == crate::clab::commands::CliCategory::Core {
                                        rows_cmds.push(Some(i));
                                    }
                                }
                                rows_cmds.push(None);
                                for (i, cmd) in self.cli_state.commands.iter().enumerate() {
                                    if cmd.category == crate::clab::commands::CliCategory::Tools {
                                        rows_cmds.push(Some(i));
                                    }
                                }

                                let selected_row_idx = rows_cmds
                                    .iter()
                                    .position(|r| *r == Some(self.cli_state.selected_command_idx))
                                    .unwrap_or(0);
                                let scroll_offset = if selected_row_idx >= inner_h {
                                    selected_row_idx.saturating_sub(inner_h - 1)
                                } else {
                                    0
                                };

                                let actual_row = scroll_offset + click_screen_row;
                                if let Some(Some(cmd_idx)) = rows_cmds.get(actual_row) {
                                    self.cli_state.selected_command_idx = *cmd_idx;
                                    self.cli_state.selected_arg_idx = 0;
                                }
                            }
                        } else {
                            let right_h = body.height;
                            let preview_h = 3.min(right_h.saturating_sub(2));
                            let avail_h = right_h.saturating_sub(preview_h);
                            let min_args_h = 4;
                            let min_out_h = 2;
                            let max_args_h = avail_h.saturating_sub(min_out_h);
                            let args_h = if min_args_h <= max_args_h {
                                ((avail_h * 50) / 100).clamp(min_args_h, max_args_h)
                            } else {
                                (avail_h / 2).min(max_args_h).max(1)
                            };

                            let rel_y = mouse.row.saturating_sub(body.y);
                            if rel_y < args_h {
                                self.cli_state.cancel_editing_arg();
                                self.cli_state.focused_pane = CliFocusedPane::Arguments;
                                let arg_click_row = rel_y.saturating_sub(3);
                                let list_h = (args_h.saturating_sub(4)) as usize;
                                let scroll_offset =
                                    if list_h > 0 && self.cli_state.selected_arg_idx >= list_h {
                                        self.cli_state.selected_arg_idx.saturating_sub(list_h - 1)
                                    } else {
                                        0
                                    };
                                let actual_idx = scroll_offset + arg_click_row as usize;
                                if actual_idx < self.cli_state.selected_command().args.len() {
                                    self.cli_state.selected_arg_idx = actual_idx;
                                    let inner_col =
                                        mouse.column.saturating_sub(body.x + tree_width + 1);
                                    if inner_col <= 4 {
                                        self.cli_state.toggle_selected_arg();
                                    } else {
                                        let arg =
                                            &self.cli_state.selected_command().args[actual_idx];
                                        if arg.is_file_arg() {
                                            let init = if !arg.value.is_empty() {
                                                Some(std::path::Path::new(&arg.value))
                                            } else {
                                                self.topology_path.as_deref()
                                            };
                                            self.file_browser.open(init);
                                        } else if matches!(
                                            arg.kind,
                                            crate::clab::commands::CliArgKind::Value { .. }
                                        ) {
                                            self.cli_state.start_editing_selected_arg();
                                        }
                                    }
                                }
                            } else if rel_y >= args_h + preview_h {
                                self.cli_state.cancel_editing_arg();
                                self.cli_state.focused_pane = CliFocusedPane::Output;
                            }
                        }
                    }
                }
                _ => {}
            }
            return;
        }

        if self.active_tab != ActiveTab::Canvas {
            return;
        }

        let body = self.canvas_area();

        // Always handle MouseUp if in DraggingNode, PanningCanvas, BoxSelection, or Wiring mode, even if cursor is outside body
        if matches!(
            mouse.kind,
            MouseEventKind::Up(MouseButton::Left) | MouseEventKind::Up(MouseButton::Middle)
        ) {
            if self.canvas.mode == CanvasMode::BoxSelection {
                if self.canvas.selected_nodes.is_empty() {
                    let clamped_col = mouse.column.clamp(body.x, body.right().saturating_sub(1));
                    let clamped_row = mouse.row.clamp(body.y, body.bottom().saturating_sub(1));
                    let (screen_x, screen_y) = (clamped_col - body.x, clamped_row - body.y);
                    let (cx, cy) = self.canvas.screen_to_canvas(screen_x, screen_y);
                    if let Some((name, _)) = self
                        .canvas
                        .nodes
                        .iter()
                        .find(|(_, n)| n.contains_point(cx, cy))
                    {
                        self.canvas.selected_nodes.insert(name.clone());
                        self.canvas.selected_node = Some(name.clone());
                    }
                }
                self.canvas.mode = CanvasMode::Normal;
                self.canvas.selection_box = None;
                return;
            } else if self.canvas.mode == CanvasMode::DraggingNode {
                self.canvas.mode = CanvasMode::Normal;
                self.sync_canvas_to_model();
                return;
            } else if self.canvas.mode == CanvasMode::PanningCanvas {
                self.canvas.mode = CanvasMode::Normal;
                return;
            } else if self.canvas.mode == CanvasMode::Wiring {
                let clamped_col = mouse.column.clamp(body.x, body.right().saturating_sub(1));
                let clamped_row = mouse.row.clamp(body.y, body.bottom().saturating_sub(1));
                let screen_x = clamped_col - body.x;
                let screen_y = clamped_row - body.y;
                let (cx, cy) = self.canvas.screen_to_canvas(screen_x, screen_y);

                let mut completed = false;
                for (name, node) in &self.canvas.nodes {
                    if let Some((ref src_name, _)) = self.canvas.wiring_source {
                        if src_name == name {
                            continue;
                        }
                    }
                    let hit_port = node.find_port_near(cx, cy, 1.5);
                    let hit_node = node.contains_point(cx, cy);
                    if hit_port.is_some() || hit_node {
                        let tgt_name = name.clone();
                        let tgt_port = hit_port.map(|p| p.name.clone()).unwrap_or_default();
                        if self.canvas.complete_wiring(&tgt_name, &tgt_port) {
                            self.sync_canvas_to_model();
                            if let Some(link) = self.canvas.links.last() {
                                self.toast_mgr.success(format!(
                                    "Linked {}:{} <-> {}:{}",
                                    link.source_node,
                                    link.source_port,
                                    link.target_node,
                                    link.target_port
                                ));
                            }
                        }
                        completed = true;
                        break;
                    }
                }
                if !completed {
                    // If the user dragged away from start position and released on empty space, cancel wiring
                    let dist = ((cx - self.canvas.drag_start_mouse.0).powi(2)
                        + (cy - self.canvas.drag_start_mouse.1).powi(2))
                    .sqrt();
                    if dist > 1.0 {
                        self.canvas.cancel_wiring();
                    }
                }
                return;
            }
        }

        // For Drag events when active, clamp coordinates to body boundaries so dragging near edges tracks smoothly
        let (screen_x, screen_y) = if self.canvas.mode == CanvasMode::DraggingNode
            || self.canvas.mode == CanvasMode::Wiring
            || self.canvas.mode == CanvasMode::PanningCanvas
            || self.canvas.mode == CanvasMode::BoxSelection
        {
            let clamped_col = mouse.column.clamp(body.x, body.right().saturating_sub(1));
            let clamped_row = mouse.row.clamp(body.y, body.bottom().saturating_sub(1));
            (clamped_col - body.x, clamped_row - body.y)
        } else {
            if mouse.column < body.x
                || mouse.column >= body.right()
                || mouse.row < body.y
                || mouse.row >= body.bottom()
            {
                if matches!(mouse.kind, MouseEventKind::Down(_)) {
                    self.canvas.selected_nodes.clear();
                    self.canvas.selected_node = None;
                    self.canvas.selected_link = None;
                }
                return;
            }
            (mouse.column - body.x, mouse.row - body.y)
        };

        let (cx, cy) = self.canvas.screen_to_canvas(screen_x, screen_y);

        match mouse.kind {
            MouseEventKind::Down(MouseButton::Left) => {
                let has_shift = mouse.modifiers.contains(KeyModifiers::SHIFT);
                let has_ctrl = mouse.modifiers.contains(KeyModifiers::CONTROL);
                let has_alt = mouse.modifiers.contains(KeyModifiers::ALT);
                let is_box_select_mode = self.canvas.mode == CanvasMode::BoxSelection;

                if is_box_select_mode {
                    self.canvas.mode = CanvasMode::BoxSelection;
                    self.canvas.drag_start_mouse = (cx, cy);
                    self.canvas.selection_box = Some(((cx, cy), (cx, cy)));
                    self.canvas.selected_nodes.clear();
                    self.canvas.selected_node = None;
                    self.canvas.selected_link = None;
                    return;
                }

                if has_shift {
                    let is_on_node = self.canvas.nodes.values().any(|n| n.contains_point(cx, cy));
                    if is_on_node {
                        self.canvas.mode = CanvasMode::PanningCanvas;
                        self.canvas.drag_start_mouse = (screen_x as f64, screen_y as f64);
                        self.canvas.drag_start_offset =
                            (self.canvas.offset_x, self.canvas.offset_y);
                        return;
                    } else {
                        self.canvas.mode = CanvasMode::BoxSelection;
                        self.canvas.drag_start_mouse = (cx, cy);
                        self.canvas.selection_box = Some(((cx, cy), (cx, cy)));
                        self.canvas.selected_nodes.clear();
                        self.canvas.selected_node = None;
                        self.canvas.selected_link = None;
                        return;
                    }
                }

                if has_ctrl {
                    self.canvas.mode = CanvasMode::PanningCanvas;
                    self.canvas.drag_start_mouse = (screen_x as f64, screen_y as f64);
                    self.canvas.drag_start_offset = (self.canvas.offset_x, self.canvas.offset_y);
                    return;
                }

                if has_alt {
                    self.canvas.mode = CanvasMode::BoxSelection;
                    self.canvas.drag_start_mouse = (cx, cy);
                    self.canvas.selection_box = Some(((cx, cy), (cx, cy)));
                    self.canvas.selected_nodes.clear();
                    self.canvas.selected_node = None;
                    self.canvas.selected_link = None;
                    return;
                }

                let was_wiring = self.canvas.mode == CanvasMode::Wiring;
                let wiring_src = self.canvas.wiring_source.clone();
                self.canvas.handle_click(cx, cy);
                if self.canvas.mode == CanvasMode::PanningCanvas {
                    self.canvas.drag_start_mouse = (screen_x as f64, screen_y as f64);
                    self.canvas.drag_start_offset = (self.canvas.offset_x, self.canvas.offset_y);
                }
                self.sync_canvas_to_model();
                if was_wiring && self.canvas.mode == CanvasMode::Normal {
                    if let Some((src_name, src_port)) = wiring_src {
                        if let Some(link) = self.canvas.links.last() {
                            if (link.source_node == src_name && link.source_port == src_port)
                                || (link.target_node == src_name && link.target_port == src_port)
                            {
                                self.toast_mgr.success(format!(
                                    "Linked {}:{} <-> {}:{}",
                                    link.source_node,
                                    link.source_port,
                                    link.target_node,
                                    link.target_port
                                ));
                            }
                        }
                    }
                }
            }
            MouseEventKind::Down(MouseButton::Middle) => {
                self.canvas.mode = CanvasMode::PanningCanvas;
                self.canvas.drag_start_mouse = (screen_x as f64, screen_y as f64);
                self.canvas.drag_start_offset = (self.canvas.offset_x, self.canvas.offset_y);
            }
            MouseEventKind::Drag(MouseButton::Left) | MouseEventKind::Drag(MouseButton::Middle) => {
                let has_shift = mouse.modifiers.contains(KeyModifiers::SHIFT);
                let has_ctrl = mouse.modifiers.contains(KeyModifiers::CONTROL);
                let has_alt = mouse.modifiers.contains(KeyModifiers::ALT);

                // If in PanningCanvas (e.g. from clicking empty canvas or Shift-clicking on a node)
                // but user holds Shift or Alt during drag, transition to BoxSelection
                if self.canvas.mode == CanvasMode::PanningCanvas
                    && (has_shift || has_alt)
                    && !has_ctrl
                    && matches!(mouse.kind, MouseEventKind::Drag(MouseButton::Left))
                {
                    self.canvas.offset_x = self.canvas.drag_start_offset.0;
                    self.canvas.offset_y = self.canvas.drag_start_offset.1;

                    let start_cx = (self.canvas.drag_start_mouse.0
                        - self.canvas.drag_start_offset.0)
                        / self.canvas.zoom;
                    let start_cy = (self.canvas.drag_start_mouse.1
                        - self.canvas.drag_start_offset.1)
                        / self.canvas.zoom;

                    self.canvas.mode = CanvasMode::BoxSelection;
                    self.canvas.drag_start_mouse = (start_cx, start_cy);
                    self.canvas.selection_box = Some(((start_cx, start_cy), (cx, cy)));
                    self.canvas.selected_nodes.clear();
                    self.canvas.selected_node = None;
                    self.canvas.selected_link = None;
                }

                if self.canvas.mode == CanvasMode::Wiring {
                    self.canvas.wiring_cursor = (cx, cy);
                    self.canvas.cursor_pos = (cx, cy);
                } else if self.canvas.mode == CanvasMode::BoxSelection {
                    let start = self.canvas.drag_start_mouse;
                    self.canvas.selection_box = Some((start, (cx, cy)));
                    let min_x = start.0.min(cx);
                    let max_x = start.0.max(cx);
                    let min_y = start.1.min(cy);
                    let max_y = start.1.max(cy);
                    self.canvas.selected_nodes = self
                        .canvas
                        .nodes
                        .iter()
                        .filter(|(_, n)| n.intersects_rect(min_x, min_y, max_x, max_y))
                        .map(|(name, _)| name.clone())
                        .collect();
                    if self.canvas.selected_nodes.len() == 1 {
                        self.canvas.selected_node =
                            self.canvas.selected_nodes.iter().next().cloned();
                    } else if self.canvas.selected_nodes.is_empty() {
                        self.canvas.selected_node = None;
                    } else if let Some(ref sel) = self.canvas.selected_node {
                        if !self.canvas.selected_nodes.contains(sel) {
                            self.canvas.selected_node =
                                self.canvas.selected_nodes.iter().next().cloned();
                        }
                    } else {
                        self.canvas.selected_node =
                            self.canvas.selected_nodes.iter().next().cloned();
                    }
                } else if self.canvas.mode == CanvasMode::DraggingNode {
                    if let Some(ref sel) = self.canvas.selected_node {
                        if let Some(node) = self.canvas.nodes.get_mut(sel) {
                            let snap = 1.0; // 1:1 smooth tracking with terminal cursor
                            let dx = cx - self.canvas.drag_start_mouse.0;
                            let dy = cy - self.canvas.drag_start_mouse.1;
                            let nx =
                                ((self.canvas.drag_start_node_pos.0 + dx) / snap).round() * snap;
                            let ny =
                                ((self.canvas.drag_start_node_pos.1 + dy) / snap).round() * snap;
                            node.set_pos(nx.max(0.0), ny.max(0.0));
                        }
                        self.canvas.recalculate_all_links();
                        self.sync_canvas_to_model();
                    }
                } else if self.canvas.mode == CanvasMode::PanningCanvas {
                    let dx = screen_x as f64 - self.canvas.drag_start_mouse.0;
                    let dy = screen_y as f64 - self.canvas.drag_start_mouse.1;
                    self.canvas.offset_x = self.canvas.drag_start_offset.0 + dx;
                    self.canvas.offset_y = self.canvas.drag_start_offset.1 + dy;
                }
            }
            MouseEventKind::ScrollUp => {
                self.canvas.pan(0.0, 3.0);
            }
            MouseEventKind::ScrollDown => {
                self.canvas.pan(0.0, -3.0);
            }
            MouseEventKind::Moved => {
                self.canvas.cursor_pos = (cx, cy);
                if self.canvas.mode == CanvasMode::Wiring {
                    self.canvas.wiring_cursor = (cx, cy);
                }
                let mut hovered_node = None;
                let mut hovered_port = None;
                for (name, node) in &self.canvas.nodes {
                    if let Some(port) = node.find_port_near(cx, cy, 1.0) {
                        hovered_node = Some(name.clone());
                        hovered_port = Some(port.name.clone());
                        break;
                    } else if node.contains_point(cx, cy) {
                        hovered_node = Some(name.clone());
                    }
                }
                self.canvas.hover_node = hovered_node;
                self.canvas.hover_port = hovered_port;
            }
            _ => {}
        }
    }

    /// Sync canvas changes into underlying LabTopology model
    pub fn sync_canvas_to_model(&mut self) {
        self.canvas.sync_to_topology(&mut self.topology);
        self.yaml_state.update(&self.topology);
    }

    /// Save topology to file
    pub fn save_topology(&mut self) {
        let path = self
            .topology_path
            .clone()
            .unwrap_or_else(|| PathBuf::from(format!("{}.clab.yml", self.topology.name)));

        match TopologyParser::save_file(&self.topology, &path) {
            Ok(()) => {
                self.topology_path = Some(path.clone());
                self.toast_mgr
                    .success(format!("Saved to '{}'", path.display()));
            }
            Err(e) => {
                self.toast_mgr.error(format!("Save error: {}", e));
            }
        }
    }

    /// Apply edited YAML text to topology and canvas
    pub fn apply_yaml_edits(&mut self) -> Result<(), String> {
        let text = self.yaml_state.get_text();
        match TopologyParser::from_yaml_str(&text) {
            Ok(new_topo) => {
                self.topology = new_topo;
                self.canvas.load_from_topology(&self.topology);
                self.yaml_state.yaml_cache = text.clone();
                self.yaml_state.cached_name = self.topology.name.clone();
                self.yaml_state.is_editing = false;
                self.yaml_state.error_msg = None;
                if let Some(ref path) = self.topology_path {
                    if let Err(e) = std::fs::write(path, &text) {
                        self.toast_mgr.error(format!("File write error: {}", e));
                    }
                }
                self.toast_mgr
                    .success("YAML changes validated and applied to topology!");
                Ok(())
            }
            Err(e) => {
                let err_msg = format!("YAML error: {}", e);
                self.yaml_state.error_msg = Some(err_msg.clone());
                self.toast_mgr.error(err_msg.clone());
                Err(err_msg)
            }
        }
    }

    /// Save or update node profile from ProfileEditModal
    pub fn save_profile_from_modal(&mut self) -> Result<(), String> {
        match self.profile_modal.to_profile() {
            Ok(profile) => {
                let display = profile.display_name.clone();
                if self.profile_modal.is_new {
                    if self
                        .node_profiles
                        .iter()
                        .any(|p| p.matches_kind(&profile.kind_name))
                    {
                        let err =
                            format!("Profile with kind '{}' already exists", profile.kind_name);
                        self.profile_modal.error_msg = Some(err.clone());
                        self.toast_mgr.error(err.clone());
                        return Err(err);
                    }
                    self.node_profiles.push(profile);
                    self.add_modal.selected_index = self.node_profiles.len().saturating_sub(1);
                    self.profile_modal.is_open = false;
                    self.toast_mgr
                        .success(format!("Added node profile '{}'", display));
                } else if let Some(idx) = self.profile_modal.editing_index {
                    if idx < self.node_profiles.len() {
                        if self
                            .node_profiles
                            .iter()
                            .enumerate()
                            .any(|(i, p)| i != idx && p.matches_kind(&profile.kind_name))
                        {
                            let err =
                                format!("Profile with kind '{}' already exists", profile.kind_name);
                            self.profile_modal.error_msg = Some(err.clone());
                            self.toast_mgr.error(err.clone());
                            return Err(err);
                        }
                        self.node_profiles[idx] = profile;
                        self.profile_modal.is_open = false;
                        self.toast_mgr
                            .success(format!("Updated node profile '{}'", display));
                    }
                }
                Ok(())
            }
            Err(e) => {
                self.profile_modal.error_msg = Some(e.clone());
                self.toast_mgr.error(e.clone());
                Err(e)
            }
        }
    }

    /// Save updated link endpoints from LinkEditModal
    pub fn save_link_from_modal(&mut self) -> Result<(), String> {
        let link_idx = self.link_edit_modal.link_index;
        let s_port = self.link_edit_modal.source_port.clone();
        let t_port = self.link_edit_modal.target_port.clone();

        if let Err(err) = self.canvas.update_link_ports(link_idx, &s_port, &t_port) {
            self.link_edit_modal.error_msg = Some(err.clone());
            self.toast_mgr.error(err.clone());
            return Err(err);
        }

        self.canvas.sync_to_topology(&mut self.topology);
        self.link_edit_modal.close();
        let (src_node, tgt_node) = if let Some(link) = self.canvas.links.get(link_idx) {
            (link.source_node.clone(), link.target_node.clone())
        } else {
            ("?".to_string(), "?".to_string())
        };
        self.toast_mgr.success(format!(
            "Updated link interfaces: {}:{} <-> {}:{}",
            src_node, s_port, tgt_node, t_port
        ));
        Ok(())
    }

    /// Prompt deployment confirmation modal
    pub fn prompt_deploy(&mut self) {
        self.confirm_modal.is_open = true;
        self.confirm_modal.title = "Deploy Lab".to_string();
        self.confirm_modal.prompt = format!("Deploy topology for lab '{}'?", self.topology.name);
        self.confirm_modal.action_name = "deploy".to_string();
        self.confirm_modal.cleanup = false;
        self.confirm_modal.reconfigure = false;
        self.confirm_modal.show_cleanup_toggle = true;
        self.confirm_modal.show_reconfigure_toggle = true;
    }

    /// Trigger asynchronous deployment with default options
    pub fn trigger_deploy(&mut self, tx: &UnboundedSender<AppEvent>) {
        self.trigger_deploy_with_opts(tx, false, false);
    }

    /// Trigger asynchronous deployment with explicit reconfigure and cleanup options
    pub fn trigger_deploy_with_opts(
        &mut self,
        tx: &UnboundedSender<AppEvent>,
        reconfigure: bool,
        cleanup: bool,
    ) {
        if self.is_operating || self.cli_state.is_running {
            self.toast_mgr
                .warning("An operation is already in progress");
            return;
        }

        self.is_operating = true;
        self.active_tab = ActiveTab::Logs;
        let opt_desc = match (cleanup, reconfigure) {
            (true, true) => " (-c, --reconfigure)",
            (true, false) => " (-c)",
            (false, true) => " (--reconfigure)",
            (false, false) => "",
        };
        self.toast_mgr
            .info(format!("Starting deployment{}...", opt_desc));

        // Ensure current topology is saved to temp or real file
        let topo = self.topology.clone();
        let safe_name: String = topo
            .name
            .chars()
            .map(|c| {
                if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                    c
                } else {
                    '_'
                }
            })
            .collect();
        let path = self
            .topology_path
            .clone()
            .unwrap_or_else(|| std::env::temp_dir().join(format!("{}.clab.yml", safe_name)));

        let _ = TopologyParser::save_file(&topo, &path);

        let tx_clone = tx.clone();
        let is_mock = self.is_mock;
        let mock_client = self.mock_client.clone();
        let clab_client = self.clab_client.clone();

        tokio::spawn(async move {
            let (log_tx, mut log_rx) = tokio::sync::mpsc::unbounded_channel::<String>();

            // Forward logs to app event
            let fwd_tx = tx_clone.clone();
            let fwd_handle = tokio::spawn(async move {
                while let Some(line) = log_rx.recv().await {
                    let _ = fwd_tx.send(AppEvent::Log(line));
                }
            });

            let success = if is_mock {
                mock_client
                    .simulate_deploy(&topo, reconfigure, cleanup, log_tx)
                    .await
                    .is_ok()
            } else {
                match clab_client
                    .deploy(&path, reconfigure, cleanup, log_tx)
                    .await
                {
                    Ok(status) => status.success(),
                    Err(e) => {
                        let _ = tx_clone.send(AppEvent::Log(format!("ERROR: {}", e)));
                        false
                    }
                }
            };

            let _ = fwd_handle.await;
            let _ = tx_clone.send(AppEvent::DeployFinished(success));
        });
    }

    /// Prompt teardown confirmation
    pub fn prompt_destroy(&mut self) {
        self.confirm_modal.is_open = true;
        self.confirm_modal.title = "Destroy Lab".to_string();
        self.confirm_modal.prompt = format!(
            "Destroy all containers and networks for lab '{}'?",
            self.topology.name
        );
        self.confirm_modal.action_name = "destroy".to_string();
        self.confirm_modal.cleanup = true;
        self.confirm_modal.reconfigure = false;
        self.confirm_modal.show_cleanup_toggle = true;
        self.confirm_modal.show_reconfigure_toggle = false;
    }

    /// Trigger teardown with default cleanup option
    pub fn trigger_destroy(&mut self, tx: &UnboundedSender<AppEvent>) {
        self.trigger_destroy_with_opts(tx, true);
    }

    /// Trigger teardown with explicit cleanup option
    pub fn trigger_destroy_with_opts(&mut self, tx: &UnboundedSender<AppEvent>, cleanup: bool) {
        if self.is_operating || self.cli_state.is_running {
            self.toast_mgr
                .warning("An operation is already in progress");
            return;
        }

        self.is_operating = true;
        self.active_tab = ActiveTab::Logs;
        let opt_desc = if cleanup {
            " with cleanup (--cleanup)"
        } else {
            ""
        };
        self.toast_mgr
            .info(format!("Tearing down lab{}...", opt_desc));

        let topo = self.topology.clone();
        let safe_name: String = topo
            .name
            .chars()
            .map(|c| {
                if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                    c
                } else {
                    '_'
                }
            })
            .collect();
        let safe_name = if safe_name.is_empty() {
            "lab".to_string()
        } else {
            safe_name
        };

        let mut path = if let Some(ref p) = self.topology_path {
            if p.is_absolute() {
                p.clone()
            } else if let Ok(cwd) = std::env::current_dir() {
                cwd.join(p)
            } else {
                p.clone()
            }
        } else {
            std::env::temp_dir().join(format!("{}.clab.yml", safe_name))
        };

        // Ensure current topology definition exists on disk for containerlab destroy
        if !path.exists() {
            if let Err(e) = TopologyParser::save_file(&topo, &path) {
                let temp_path = std::env::temp_dir().join(format!("{}.clab.yml", safe_name));
                if let Err(temp_err) = TopologyParser::save_file(&topo, &temp_path) {
                    let err_msg = format!(
                        "Cannot teardown: topology file could not be written to '{}' ({}) or '{}' ({})",
                        path.display(),
                        e,
                        temp_path.display(),
                        temp_err
                    );
                    self.toast_mgr.error(&err_msg);
                    self.log_viewer.push_line(format!("ERROR: {}", err_msg));
                    self.is_operating = false;
                    return;
                } else {
                    path = temp_path;
                }
            }
        }

        let tx_clone = tx.clone();
        let is_mock = self.is_mock;
        let mock_client = self.mock_client.clone();
        let clab_client = self.clab_client.clone();

        tokio::spawn(async move {
            let (log_tx, mut log_rx) = tokio::sync::mpsc::unbounded_channel::<String>();

            let fwd_tx = tx_clone.clone();
            let fwd_handle = tokio::spawn(async move {
                while let Some(line) = log_rx.recv().await {
                    let _ = fwd_tx.send(AppEvent::Log(line));
                }
            });

            let success = if is_mock {
                mock_client
                    .simulate_destroy(&path, cleanup, log_tx)
                    .await
                    .is_ok()
            } else {
                match clab_client.destroy(&path, cleanup, log_tx).await {
                    Ok(status) => status.success(),
                    Err(e) => {
                        let _ = tx_clone.send(AppEvent::Log(format!("ERROR: {}", e)));
                        false
                    }
                }
            };

            let _ = fwd_handle.await;
            let _ = tx_clone.send(AppEvent::DestroyFinished(success));
        });
    }

    /// Sync active topology path and node names into CLI command tree
    pub fn sync_cli_topo_path(&mut self) {
        let path_str = self
            .topology_path
            .as_ref()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|| format!("{}.clab.yml", self.topology.name));
        self.cli_state.update_topo_path(&path_str);

        let mut node_iter = self.topology.topology.nodes.keys();
        if let Some(first_node) = node_iter.next() {
            self.cli_state.update_default_node(first_node);
            if let Some(second_node) = node_iter.next() {
                for cmd in &mut self.cli_state.commands {
                    for arg in &mut cmd.args {
                        if arg.flag == "--b-node" && (arg.value.is_empty() || arg.value == "host1")
                        {
                            arg.value = second_node.to_string();
                        }
                    }
                }
            }
        }
    }

    /// Handle keyboard input when inline-editing a parameter in the CLI tab
    pub fn handle_cli_edit_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Esc => {
                self.cli_state.cancel_editing_arg();
                self.toast_mgr.info("Argument edit cancelled");
            }
            KeyCode::Enter => {
                self.cli_state.apply_editing_arg();
                self.toast_mgr.success("Argument value saved");
            }
            KeyCode::Tab | KeyCode::F(2) => {
                let is_file = self
                    .cli_state
                    .selected_arg()
                    .map(|a| a.is_file_arg())
                    .unwrap_or(false);
                if is_file {
                    let buf_val = self.cli_state.edit_buffer.clone();
                    self.cli_state.cancel_editing_arg();
                    let init = if !buf_val.is_empty() {
                        Some(std::path::PathBuf::from(buf_val))
                    } else {
                        None
                    };
                    self.file_browser.open(init.as_deref());
                }
            }
            KeyCode::Backspace => {
                self.cli_state.edit_buffer.pop();
            }
            KeyCode::Delete => {
                self.cli_state.edit_buffer.clear();
            }
            KeyCode::Char(c) => {
                self.cli_state.edit_buffer.push(c);
            }
            _ => {}
        }
    }

    /// Handle keyboard navigation and interaction in CLI command tree tab
    pub fn handle_cli_key(&mut self, key: KeyEvent, tx: &UnboundedSender<AppEvent>) {
        match key.code {
            KeyCode::Tab => {
                if key.modifiers.contains(KeyModifiers::SHIFT) {
                    self.cli_state.focused_pane = self.cli_state.focused_pane.prev();
                } else {
                    self.cli_state.focused_pane = self.cli_state.focused_pane.next();
                }
            }
            KeyCode::BackTab => {
                self.cli_state.focused_pane = self.cli_state.focused_pane.prev();
            }
            KeyCode::Char('r') => {
                self.trigger_cli_command(tx);
            }
            _ => match self.cli_state.focused_pane {
                CliFocusedPane::Commands => match key.code {
                    KeyCode::Up | KeyCode::Char('k') => self.cli_state.select_prev_command(),
                    KeyCode::Down | KeyCode::Char('j') => self.cli_state.select_next_command(),
                    KeyCode::Right | KeyCode::Char('l') => {
                        self.cli_state.focused_pane = CliFocusedPane::Arguments;
                    }
                    KeyCode::Enter => self.trigger_cli_command(tx),
                    _ => {}
                },
                CliFocusedPane::Arguments => match key.code {
                    KeyCode::Up | KeyCode::Char('k') => self.cli_state.select_prev_arg(),
                    KeyCode::Down | KeyCode::Char('j') => self.cli_state.select_next_arg(),
                    KeyCode::Left | KeyCode::Char('h') => {
                        self.cli_state.focused_pane = CliFocusedPane::Commands;
                    }
                    KeyCode::Char(' ') => self.cli_state.toggle_selected_arg(),
                    KeyCode::Char('b') | KeyCode::Char('B') => {
                        if let Some(arg) = self.cli_state.selected_arg() {
                            if arg.is_file_arg() {
                                let init = if !arg.value.is_empty() {
                                    Some(std::path::Path::new(&arg.value))
                                } else {
                                    self.topology_path.as_deref()
                                };
                                self.file_browser.open(init);
                            }
                        }
                    }
                    KeyCode::Char('e') => {
                        if let Some(arg) = self.cli_state.selected_arg() {
                            if matches!(arg.kind, CliArgKind::Value { .. }) {
                                self.cli_state.start_editing_selected_arg();
                            }
                        }
                    }
                    KeyCode::Enter => {
                        if let Some(arg) = self.cli_state.selected_arg() {
                            if arg.is_file_arg() {
                                let init = if !arg.value.is_empty() {
                                    Some(std::path::Path::new(&arg.value))
                                } else {
                                    self.topology_path.as_deref()
                                };
                                self.file_browser.open(init);
                            } else if matches!(arg.kind, CliArgKind::Value { .. }) {
                                self.cli_state.start_editing_selected_arg();
                            } else {
                                self.trigger_cli_command(tx);
                            }
                        }
                    }
                    _ => {}
                },
                CliFocusedPane::Output => match key.code {
                    KeyCode::Up | KeyCode::Char('k') => self.cli_state.scroll_up(2, 20),
                    KeyCode::Down | KeyCode::Char('j') => self.cli_state.scroll_down(2),
                    KeyCode::PageUp => self.cli_state.scroll_up(10, 20),
                    KeyCode::PageDown => self.cli_state.scroll_down(10),
                    KeyCode::Char('c') => {
                        self.cli_state.clear_output();
                        self.toast_mgr.info("CLI execution log cleared");
                    }
                    KeyCode::Left | KeyCode::Char('h') => {
                        self.cli_state.focused_pane = CliFocusedPane::Arguments;
                    }
                    KeyCode::Enter => self.trigger_cli_command(tx),
                    _ => {}
                },
            },
        }
    }

    /// Trigger asynchronous execution of currently configured CLI command
    pub fn trigger_cli_command(&mut self, tx: &UnboundedSender<AppEvent>) {
        if self.cli_state.is_running || self.is_operating {
            self.toast_mgr
                .warning("An operation is already in progress");
            return;
        }

        let cmd = self.cli_state.selected_command().clone();
        let tokens = cmd.tokens();
        if tokens.is_empty() {
            self.toast_mgr.warning("No command specified");
            return;
        }

        self.cli_state.is_running = true;
        self.cli_state.executing_cmd = Some(cmd.name.clone());
        self.cli_state.last_status = None;
        let preview = cmd.preview_string("containerlab");
        self.cli_state.push_output_line(format!("$ {}", preview));
        self.toast_mgr.info(format!("Executing '{}'...", cmd.name));

        let tx_clone = tx.clone();
        let is_mock = self.is_mock;
        let mock_client = self.mock_client.clone();
        let clab_client = self.clab_client.clone();
        let cmd_name = cmd.name.clone();

        tokio::spawn(async move {
            let (log_tx, mut log_rx) = tokio::sync::mpsc::unbounded_channel::<String>();

            let fwd_tx = tx_clone.clone();
            let fwd_handle = tokio::spawn(async move {
                while let Some(line) = log_rx.recv().await {
                    let _ = fwd_tx.send(AppEvent::CliLog(line));
                }
            });

            let success = if is_mock {
                mock_client
                    .simulate_command(&cmd_name, &tokens, log_tx)
                    .await
                    .is_ok()
            } else {
                match clab_client.run_custom_command(&tokens, log_tx).await {
                    Ok(status) => status.success(),
                    Err(e) => {
                        let _ = tx_clone.send(AppEvent::CliLog(format!("ERROR: {}", e)));
                        false
                    }
                }
            };

            let _ = fwd_handle.await;
            let _ = tx_clone.send(AppEvent::CliCommandFinished {
                cmd_name: cmd_name.clone(),
                success,
            });
        });
    }

    /// Trigger inspect
    pub fn trigger_inspect(&mut self, tx: &UnboundedSender<AppEvent>) {
        self.inspect_state.is_loading = true;
        let is_mock = self.is_mock;
        let topo = self.topology.clone();
        let clab_client = self.clab_client.clone();
        let tx_clone = tx.clone();

        tokio::spawn(async move {
            let containers = if is_mock {
                MockClabClient::mock_inspect(&topo)
            } else {
                clab_client.inspect(None, true).await.unwrap_or_default()
            };
            let _ = tx_clone.send(AppEvent::InspectUpdated(containers));
        });
    }

    /// Render entire app frame
    pub fn render(&self, frame: &mut Frame) {
        let area = frame.area();
        self.last_area.set(area);
        let buf = frame.buffer_mut();

        if area.width < 20 || area.height < 10 {
            return;
        }

        // Layout zones
        let header_area = Rect::new(area.x, area.y, area.width, 1);
        let tabs_area = Rect::new(area.x, area.y + 1, area.width, 2);
        let footer_area = Rect::new(area.x, area.bottom().saturating_sub(1), area.width, 1);
        let body_area = Rect::new(
            area.x,
            area.y + 3,
            area.width,
            area.height.saturating_sub(4),
        );

        // Render header, tabs, footer
        let priv_desc = self.clab_client.privilege_status_desc();
        AppLayout::render_header(
            &self.topology.name,
            priv_desc,
            header_area,
            buf,
            &self.theme,
        );

        AppLayout::render_tabs(self.active_tab, tabs_area, buf, &self.theme);
        AppLayout::render_footer(self.active_tab, footer_area, buf, &self.theme);

        // Render active body view
        match self.active_tab {
            ActiveTab::Canvas => {
                let params = CanvasViewParams {
                    canvas: &self.canvas,
                    topo: &self.topology,
                    drawer: &self.drawer,
                    add_modal: &self.add_modal,
                    profile_modal: Some(&self.profile_modal),
                    link_modal: Some(&self.link_edit_modal),
                    profiles: &self.node_profiles,
                    theme: &self.theme,
                };
                CanvasView::render(params, body_area, buf);
            }
            ActiveTab::Inspect => {
                InspectView::render(&self.inspect_state, body_area, buf, &self.theme);
            }
            ActiveTab::Logs => {
                LogsView::render(&self.log_viewer, body_area, buf, &self.theme);
            }
            ActiveTab::Yaml => {
                YamlView::render(&self.yaml_state, body_area, buf, &self.theme);
            }
            ActiveTab::Cli => {
                CliView::render(&self.cli_state, body_area, buf, &self.theme);
            }
        }

        // Modals overlay
        if self.confirm_modal.is_open {
            AppLayout::render_confirm_modal(&self.confirm_modal, area, buf, &self.theme);
        }

        if self.file_browser.is_open {
            self.file_browser.render(area, buf, &self.theme);
        }

        if self.show_help {
            HelpPopup::render(area, buf, &self.theme);
        }

        // Toasts overlay
        self.toast_mgr.render(area, buf, &self.theme);
    }
}
