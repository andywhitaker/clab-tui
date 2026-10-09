#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CliCategory {
    Core,
    Tools,
}

impl CliCategory {
    pub fn title(&self) -> &'static str {
        match self {
            CliCategory::Core => "Core Commands",
            CliCategory::Tools => "Tools Subcommands",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CliArgKind {
    Flag,
    Value { placeholder: String },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CliArg {
    pub flag: String,
    pub short: Option<String>,
    pub description: String,
    pub kind: CliArgKind,
    pub enabled: bool,
    pub value: String,
}

impl CliArg {
    pub fn flag(flag: &str, short: Option<&str>, desc: &str, default_enabled: bool) -> Self {
        Self {
            flag: flag.to_string(),
            short: short.map(|s| s.to_string()),
            description: desc.to_string(),
            kind: CliArgKind::Flag,
            enabled: default_enabled,
            value: String::new(),
        }
    }

    pub fn value(
        flag: &str,
        short: Option<&str>,
        desc: &str,
        placeholder: &str,
        default_val: &str,
        default_enabled: bool,
    ) -> Self {
        Self {
            flag: flag.to_string(),
            short: short.map(|s| s.to_string()),
            description: desc.to_string(),
            kind: CliArgKind::Value {
                placeholder: placeholder.to_string(),
            },
            enabled: default_enabled,
            value: default_val.to_string(),
        }
    }

    /// Check if this CLI parameter represents a file path (e.g. topology or template file)
    pub fn is_file_arg(&self) -> bool {
        if matches!(self.kind, CliArgKind::Flag) {
            return false;
        }
        self.flag == "--topo"
            || self.short.as_deref() == Some("-t")
            || self.flag == "--config"
            || self.flag == "--cfg"
            || self.flag.contains("topo")
            || self.flag.contains("file")
            || self.flag.contains("template")
            || self.flag.contains("license")
            || match &self.kind {
                CliArgKind::Value { placeholder } => {
                    let p = placeholder.to_lowercase();
                    p == "path"
                        || p == "file"
                        || p == "topo"
                        || p == "template"
                        || p.contains("file")
                        || p.contains("path")
                }
                _ => false,
            }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CliCommand {
    pub name: String,
    pub category: CliCategory,
    pub description: String,
    pub args: Vec<CliArg>,
    pub last_status: Option<bool>,
}

impl CliCommand {
    pub fn new(name: &str, category: CliCategory, desc: &str, args: Vec<CliArg>) -> Self {
        Self {
            name: name.to_string(),
            category,
            description: desc.to_string(),
            args,
            last_status: None,
        }
    }

    /// Extract argument tokens for process execution
    pub fn tokens(&self) -> Vec<String> {
        let mut tokens = Vec::new();
        for part in self.name.split_whitespace() {
            tokens.push(part.to_string());
        }
        for arg in &self.args {
            if !arg.enabled {
                continue;
            }
            match &arg.kind {
                CliArgKind::Flag => {
                    tokens.push(arg.flag.clone());
                }
                CliArgKind::Value { .. } => {
                    let val = arg.value.trim();
                    if !val.is_empty() {
                        tokens.push(arg.flag.clone());
                        tokens.push(val.to_string());
                    }
                }
            }
        }
        tokens
    }

    /// Generate human-readable shell preview command string
    pub fn preview_string(&self, binary: &str) -> String {
        let mut parts = vec![binary.to_string()];
        for part in self.name.split_whitespace() {
            parts.push(part.to_string());
        }
        for arg in &self.args {
            if !arg.enabled {
                continue;
            }
            match &arg.kind {
                CliArgKind::Flag => {
                    parts.push(arg.flag.clone());
                }
                CliArgKind::Value { .. } => {
                    let val = arg.value.trim();
                    if !val.is_empty() {
                        parts.push(arg.flag.clone());
                        if val.contains(' ') || val.contains('"') {
                            parts.push(format!("\"{}\"", val.replace('"', "\\\"")));
                        } else {
                            parts.push(val.to_string());
                        }
                    }
                }
            }
        }
        parts.join(" ")
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CliFocusedPane {
    Commands,
    Arguments,
    Output,
}

impl CliFocusedPane {
    pub fn next(&self) -> Self {
        match self {
            CliFocusedPane::Commands => CliFocusedPane::Arguments,
            CliFocusedPane::Arguments => CliFocusedPane::Output,
            CliFocusedPane::Output => CliFocusedPane::Commands,
        }
    }

    pub fn prev(&self) -> Self {
        match self {
            CliFocusedPane::Commands => CliFocusedPane::Output,
            CliFocusedPane::Arguments => CliFocusedPane::Commands,
            CliFocusedPane::Output => CliFocusedPane::Arguments,
        }
    }
}

#[derive(Debug, Clone)]
pub struct CliViewState {
    pub commands: Vec<CliCommand>,
    pub selected_command_idx: usize,
    pub selected_arg_idx: usize,
    pub focused_pane: CliFocusedPane,
    pub is_editing_arg: bool,
    pub edit_buffer: String,
    pub output_lines: Vec<String>,
    pub output_scroll: usize,
    pub is_running: bool,
    pub executing_cmd: Option<String>,
    pub last_status: Option<bool>,
}

impl Default for CliViewState {
    fn default() -> Self {
        Self::new(None, None)
    }
}

impl CliViewState {
    pub fn new(topo_path: Option<&str>, default_node: Option<&str>) -> Self {
        let commands = default_command_tree(topo_path, default_node);
        Self {
            commands,
            selected_command_idx: 0,
            selected_arg_idx: 0,
            focused_pane: CliFocusedPane::Commands,
            is_editing_arg: false,
            edit_buffer: String::new(),
            output_lines: vec![
                "Containerlab CLI Command Center initialized.".to_string(),
                "Select a command, configure flags & options, and press [r / Enter] to execute."
                    .to_string(),
            ],
            output_scroll: 0,
            is_running: false,
            executing_cmd: None,
            last_status: None,
        }
    }

    pub fn selected_command(&self) -> &CliCommand {
        &self.commands[self.selected_command_idx]
    }

    pub fn selected_command_mut(&mut self) -> &mut CliCommand {
        &mut self.commands[self.selected_command_idx]
    }

    pub fn selected_arg(&self) -> Option<&CliArg> {
        self.selected_command().args.get(self.selected_arg_idx)
    }

    pub fn selected_arg_mut(&mut self) -> Option<&mut CliArg> {
        let cmd_idx = self.selected_command_idx;
        let arg_idx = self.selected_arg_idx;
        self.commands[cmd_idx].args.get_mut(arg_idx)
    }

    pub fn select_next_command(&mut self) {
        if !self.commands.is_empty() {
            self.selected_command_idx = (self.selected_command_idx + 1) % self.commands.len();
            self.selected_arg_idx = 0;
            self.cancel_editing_arg();
        }
    }

    pub fn select_prev_command(&mut self) {
        if !self.commands.is_empty() {
            if self.selected_command_idx == 0 {
                self.selected_command_idx = self.commands.len() - 1;
            } else {
                self.selected_command_idx -= 1;
            }
            self.selected_arg_idx = 0;
            self.cancel_editing_arg();
        }
    }

    pub fn select_next_arg(&mut self) {
        let total = self.selected_command().args.len();
        if total > 0 {
            self.selected_arg_idx = (self.selected_arg_idx + 1) % total;
            self.cancel_editing_arg();
        }
    }

    pub fn select_prev_arg(&mut self) {
        let total = self.selected_command().args.len();
        if total > 0 {
            if self.selected_arg_idx == 0 {
                self.selected_arg_idx = total - 1;
            } else {
                self.selected_arg_idx -= 1;
            }
            self.cancel_editing_arg();
        }
    }

    pub fn toggle_selected_arg(&mut self) {
        if let Some(arg) = self.selected_arg_mut() {
            arg.enabled = !arg.enabled;
        }
    }

    pub fn start_editing_selected_arg(&mut self) {
        if let Some(arg) = self.selected_arg() {
            if matches!(arg.kind, CliArgKind::Value { .. }) {
                self.edit_buffer = arg.value.clone();
                self.is_editing_arg = true;
            }
        }
    }

    pub fn apply_editing_arg(&mut self) {
        if self.is_editing_arg {
            let new_val = self.edit_buffer.trim().to_string();
            if let Some(arg) = self.selected_arg_mut() {
                arg.value = new_val;
                arg.enabled = !arg.value.is_empty();
            }
            self.is_editing_arg = false;
        }
    }

    pub fn cancel_editing_arg(&mut self) {
        self.is_editing_arg = false;
        self.edit_buffer.clear();
    }

    pub fn update_topo_path(&mut self, topo_path: &str) {
        for cmd in &mut self.commands {
            for arg in &mut cmd.args {
                if arg.flag == "--topo" {
                    arg.value = topo_path.to_string();
                    arg.enabled = true;
                }
            }
        }
    }

    pub fn update_default_node(&mut self, node_name: &str) {
        if node_name.is_empty() {
            return;
        }
        for cmd in &mut self.commands {
            for arg in &mut cmd.args {
                if arg.flag == "--node" || arg.flag == "--a-node" {
                    arg.value = node_name.to_string();
                }
            }
        }
    }

    pub fn preview_string(&self, binary: &str) -> String {
        self.selected_command().preview_string(binary)
    }

    pub fn tokens(&self) -> Vec<String> {
        self.selected_command().tokens()
    }

    pub fn scroll_up(&mut self, delta: usize, visible_height: usize) {
        let max_scroll = self.output_lines.len().saturating_sub(visible_height);
        self.output_scroll = (self.output_scroll + delta).min(max_scroll);
    }

    pub fn scroll_down(&mut self, delta: usize) {
        self.output_scroll = self.output_scroll.saturating_sub(delta);
    }

    pub fn clear_output(&mut self) {
        self.output_lines.clear();
        self.output_scroll = 0;
    }

    pub fn push_output_line(&mut self, line: String) {
        self.output_lines.push(line);
        // Keep scroll at bottom if already at bottom or near it
        if self.output_lines.len() > 1000 {
            self.output_lines.drain(0..200);
        }
    }
}

pub fn default_command_tree(
    topo_path: Option<&str>,
    default_node: Option<&str>,
) -> Vec<CliCommand> {
    let tp = topo_path.unwrap_or("topology.clab.yml");
    let node = default_node.unwrap_or("srl1");

    vec![
        // Core Commands
        CliCommand::new(
            "deploy",
            CliCategory::Core,
            "Deploy a lab topology defined by a topology file",
            vec![
                CliArg::value(
                    "--topo",
                    Some("-t"),
                    "Topology definition file",
                    "path",
                    tp,
                    true,
                ),
                CliArg::flag(
                    "--cleanup",
                    Some("-c"),
                    "Remove existing lab artifacts before deployment",
                    false,
                ),
                CliArg::flag(
                    "--reconfigure",
                    None,
                    "Reconfigure existing containers",
                    false,
                ),
                CliArg::value(
                    "--max-workers",
                    None,
                    "Max concurrent workers for deploy",
                    "count",
                    "",
                    false,
                ),
                CliArg::flag(
                    "--skip-post-deploy",
                    None,
                    "Skip post-deployment actions",
                    false,
                ),
                CliArg::value(
                    "--export-template",
                    None,
                    "Export template file path",
                    "path",
                    "",
                    false,
                ),
            ],
        ),
        CliCommand::new(
            "destroy",
            CliCategory::Core,
            "Destroy a lab topology and tear down containers",
            vec![
                CliArg::value(
                    "--topo",
                    Some("-t"),
                    "Topology definition file",
                    "path",
                    tp,
                    true,
                ),
                CliArg::flag(
                    "--cleanup",
                    Some("-c"),
                    "Remove lab directory and bridge artifacts",
                    true,
                ),
                CliArg::flag(
                    "--all",
                    Some("-a"),
                    "Destroy all running containerlab labs",
                    false,
                ),
                CliArg::flag(
                    "--keep-mgmt-net",
                    None,
                    "Keep management network intact",
                    false,
                ),
                CliArg::flag(
                    "--graceful",
                    None,
                    "Gracefully stop containers before removal",
                    false,
                ),
            ],
        ),
        CliCommand::new(
            "redeploy",
            CliCategory::Core,
            "Destroy and re-deploy a lab topology in one step",
            vec![
                CliArg::value(
                    "--topo",
                    Some("-t"),
                    "Topology definition file",
                    "path",
                    tp,
                    true,
                ),
                CliArg::flag(
                    "--cleanup",
                    Some("-c"),
                    "Remove lab artifacts before redeployment",
                    true,
                ),
                CliArg::flag("--reconfigure", None, "Reconfigure existing nodes", false),
                CliArg::flag(
                    "--skip-post-deploy",
                    None,
                    "Skip post-deploy execution",
                    false,
                ),
            ],
        ),
        CliCommand::new(
            "inspect",
            CliCategory::Core,
            "Inspect running containerlab labs and node details",
            vec![
                CliArg::value(
                    "--topo",
                    Some("-t"),
                    "Topology definition file",
                    "path",
                    tp,
                    true,
                ),
                CliArg::flag(
                    "--all",
                    Some("-a"),
                    "Inspect all running containerlab labs",
                    false,
                ),
                CliArg::flag(
                    "--details",
                    None,
                    "Show detailed container inspect properties",
                    false,
                ),
                CliArg::value(
                    "--format",
                    Some("-f"),
                    "Output format (table, json)",
                    "table|json",
                    "table",
                    false,
                ),
            ],
        ),
        CliCommand::new(
            "graph",
            CliCategory::Core,
            "Generate topology diagrams and launch web graph",
            vec![
                CliArg::value(
                    "--topo",
                    Some("-t"),
                    "Topology definition file",
                    "path",
                    tp,
                    true,
                ),
                CliArg::flag(
                    "--srv",
                    None,
                    "Start local HTTP web server for interactive graph",
                    true,
                ),
                CliArg::value(
                    "--addr",
                    None,
                    "HTTP server listen address",
                    ":50080",
                    ":50080",
                    false,
                ),
                CliArg::flag("--drawio", None, "Generate Draw.io diagram XML", false),
                CliArg::flag("--offline", None, "Use embedded offline web assets", false),
            ],
        ),
        CliCommand::new(
            "save",
            CliCategory::Core,
            "Save running configuration of network containers",
            vec![
                CliArg::value(
                    "--topo",
                    Some("-t"),
                    "Topology definition file",
                    "path",
                    tp,
                    true,
                ),
                CliArg::value(
                    "--node",
                    Some("-n"),
                    "Target specific node name",
                    "node",
                    node,
                    false,
                ),
            ],
        ),
        CliCommand::new(
            "version",
            CliCategory::Core,
            "Show containerlab version and build information",
            vec![CliArg::flag(
                "--check",
                None,
                "Check for new containerlab release updates",
                false,
            )],
        ),
        CliCommand::new(
            "exec",
            CliCategory::Core,
            "Execute a command across one or more lab nodes",
            vec![
                CliArg::value(
                    "--topo",
                    Some("-t"),
                    "Topology definition file",
                    "path",
                    tp,
                    true,
                ),
                CliArg::value(
                    "--cmd",
                    None,
                    "Command string to execute",
                    "cmd",
                    "uname -a",
                    true,
                ),
                CliArg::value(
                    "--label",
                    None,
                    "Filter nodes matching label",
                    "key=val",
                    "",
                    false,
                ),
            ],
        ),
        CliCommand::new(
            "config",
            CliCategory::Core,
            "Configure network interfaces and apply configuration artifacts",
            vec![
                CliArg::value(
                    "--topo",
                    Some("-t"),
                    "Topology definition file",
                    "path",
                    tp,
                    true,
                ),
                CliArg::value(
                    "--filter",
                    None,
                    "Filter nodes or interface configs",
                    "filter",
                    "",
                    false,
                ),
                CliArg::value(
                    "--template",
                    None,
                    "Path to Jinja2 config template",
                    "path",
                    "",
                    false,
                ),
            ],
        ),
        // Tools Subcommands
        CliCommand::new(
            "tools capture",
            CliCategory::Tools,
            "Capture packet trace on a container interface into PCAP",
            vec![
                CliArg::value(
                    "--topo",
                    Some("-t"),
                    "Topology definition file",
                    "path",
                    tp,
                    true,
                ),
                CliArg::value(
                    "--node",
                    Some("-n"),
                    "Node name to capture from",
                    "node",
                    node,
                    true,
                ),
                CliArg::value(
                    "--interface",
                    Some("-i"),
                    "Interface to capture on",
                    "iface",
                    "eth1",
                    true,
                ),
                CliArg::value(
                    "--pcap-file",
                    None,
                    "Save capture to PCAP file",
                    "capture.pcap",
                    "",
                    false,
                ),
            ],
        ),
        CliCommand::new(
            "tools veth",
            CliCategory::Tools,
            "Create or manage virtual ethernet (veth) pairs between namespaces",
            vec![
                CliArg::value(
                    "--a-node",
                    None,
                    "Node A container name",
                    "node-a",
                    node,
                    true,
                ),
                CliArg::value(
                    "--b-node",
                    None,
                    "Node B container name",
                    "node-b",
                    "host1",
                    true,
                ),
                CliArg::value(
                    "--a-if",
                    None,
                    "Interface name on Node A",
                    "eth-a",
                    "eth1",
                    true,
                ),
                CliArg::value(
                    "--b-if",
                    None,
                    "Interface name on Node B",
                    "eth-b",
                    "eth1",
                    true,
                ),
            ],
        ),
        CliCommand::new(
            "tools disable-tx-offload",
            CliCategory::Tools,
            "Disable TX checksum offloading on network interfaces",
            vec![
                CliArg::value(
                    "--topo",
                    Some("-t"),
                    "Topology definition file",
                    "path",
                    tp,
                    true,
                ),
                CliArg::value("--node", Some("-n"), "Target node name", "node", node, true),
                CliArg::value(
                    "--interface",
                    Some("-i"),
                    "Interface to disable offload on",
                    "iface",
                    "eth1",
                    false,
                ),
            ],
        ),
        CliCommand::new(
            "tools vxlan",
            CliCategory::Tools,
            "Create and attach VXLAN tunnels to containerlab networks",
            vec![
                CliArg::value(
                    "--remote",
                    None,
                    "Remote VTEP IP address",
                    "ip",
                    "192.168.10.2",
                    true,
                ),
                CliArg::value(
                    "--id",
                    None,
                    "VXLAN Network Identifier (VNI)",
                    "vni",
                    "100",
                    true,
                ),
                CliArg::value(
                    "--link",
                    None,
                    "Interface / Bridge to attach tunnel",
                    "iface",
                    "br-vxlan",
                    true,
                ),
            ],
        ),
        CliCommand::new(
            "tools netem",
            CliCategory::Tools,
            "Apply network emulation (delay, jitter, loss) on an interface",
            vec![
                CliArg::value(
                    "--node",
                    Some("-n"),
                    "Target node container name",
                    "node",
                    node,
                    true,
                ),
                CliArg::value(
                    "--interface",
                    Some("-i"),
                    "Interface name",
                    "iface",
                    "eth1",
                    true,
                ),
                CliArg::value(
                    "--delay",
                    None,
                    "Network latency delay",
                    "time",
                    "15ms",
                    true,
                ),
                CliArg::value(
                    "--loss",
                    None,
                    "Packet loss percentage",
                    "percent",
                    "0.5%",
                    false,
                ),
                CliArg::value(
                    "--jitter",
                    None,
                    "Latency jitter variation",
                    "time",
                    "3ms",
                    false,
                ),
                CliArg::value(
                    "--rate",
                    None,
                    "Bandwidth rate limit",
                    "bandwidth",
                    "100mbit",
                    false,
                ),
            ],
        ),
        CliCommand::new(
            "tools cert",
            CliCategory::Tools,
            "Generate and manage TLS certificates and CA for lab nodes",
            vec![
                CliArg::value(
                    "--ca-cert",
                    None,
                    "Path to root CA certificate",
                    "ca.pem",
                    "",
                    false,
                ),
                CliArg::value(
                    "--ca-key",
                    None,
                    "Path to root CA private key",
                    "ca-key.pem",
                    "",
                    false,
                ),
                CliArg::value(
                    "--common-name",
                    None,
                    "Common Name (CN) for certificate",
                    "domain",
                    "clab.local",
                    true,
                ),
            ],
        ),
        CliCommand::new(
            "tools api-server",
            CliCategory::Tools,
            "Start the containerlab REST API and webhook server",
            vec![
                CliArg::value(
                    "--address",
                    None,
                    "Listen address and port",
                    "addr:port",
                    "0.0.0.0:8000",
                    true,
                ),
                CliArg::value(
                    "--cert",
                    None,
                    "TLS certificate file path",
                    "cert.pem",
                    "",
                    false,
                ),
                CliArg::value(
                    "--key",
                    None,
                    "TLS private key file path",
                    "key.pem",
                    "",
                    false,
                ),
            ],
        ),
    ]
}
