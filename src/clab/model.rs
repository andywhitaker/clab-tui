use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Top-level Containerlab topology file definition
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LabTopology {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prefix: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mgmt: Option<MgmtConfig>,
    pub topology: TopologyData,
}

impl Default for LabTopology {
    fn default() -> Self {
        Self {
            name: "clab-quickstart".to_string(),
            prefix: None,
            mgmt: Some(MgmtConfig::default()),
            topology: TopologyData::default(),
        }
    }
}

/// Management network configuration
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MgmtConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub network: Option<String>,
    #[serde(rename = "ipv4-subnet", skip_serializing_if = "Option::is_none")]
    pub ipv4_subnet: Option<String>,
    #[serde(rename = "ipv6-subnet", skip_serializing_if = "Option::is_none")]
    pub ipv6_subnet: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mtu: Option<u32>,
}

impl Default for MgmtConfig {
    fn default() -> Self {
        Self {
            network: Some("clab".to_string()),
            ipv4_subnet: Some("172.20.20.0/24".to_string()),
            ipv6_subnet: None,
            mtu: None,
        }
    }
}

/// Topology container containing defaults, kinds, nodes, and links
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
pub struct TopologyData {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub defaults: Option<NodeDefinition>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub kinds: Option<BTreeMap<String, KindConfig>>,
    #[serde(default)]
    pub nodes: BTreeMap<String, NodeDefinition>,
    #[serde(default)]
    pub links: Vec<LinkDefinition>,
}

/// Kind-level configuration defaults
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
pub struct KindConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub image: Option<String>,
    #[serde(rename = "type", skip_serializing_if = "Option::is_none")]
    pub kind_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub env: Option<BTreeMap<String, String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub binds: Option<Vec<String>>,
}

/// Individual node definition within a topology
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
pub struct NodeDefinition {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub kind: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub image: Option<String>,
    #[serde(rename = "type", skip_serializing_if = "Option::is_none")]
    pub node_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub group: Option<String>,
    #[serde(rename = "mgmt-ipv4", skip_serializing_if = "Option::is_none")]
    pub mgmt_ipv4: Option<String>,
    #[serde(rename = "mgmt-ipv6", skip_serializing_if = "Option::is_none")]
    pub mgmt_ipv6: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub env: Option<BTreeMap<String, String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub labels: Option<BTreeMap<String, String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub binds: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ports: Option<Vec<String>>,
    #[serde(skip)]
    pub interface_pattern: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cmd: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user: Option<String>,
    #[serde(rename = "startup-config", skip_serializing_if = "Option::is_none")]
    pub startup_config: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub license: Option<String>,
}

impl NodeDefinition {
    /// Extract canvas X position if stored in labels
    pub fn canvas_x(&self) -> Option<f64> {
        self.labels
            .as_ref()?
            .get("clab-tui-x")
            .and_then(|v| v.parse::<f64>().ok())
    }

    /// Extract canvas Y position if stored in labels
    pub fn canvas_y(&self) -> Option<f64> {
        self.labels
            .as_ref()?
            .get("clab-tui-y")
            .and_then(|v| v.parse::<f64>().ok())
    }

    /// Update or set canvas coordinates in labels
    pub fn set_canvas_pos(&mut self, x: f64, y: f64) {
        let labels = self.labels.get_or_insert_with(BTreeMap::new);
        labels.insert("clab-tui-x".to_string(), format!("{:.1}", x));
        labels.insert("clab-tui-y".to_string(), format!("{:.1}", y));
    }
}

/// Link definition connecting two endpoints
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LinkDefinition {
    pub endpoints: [String; 2],
    #[serde(skip_serializing_if = "Option::is_none")]
    pub labels: Option<BTreeMap<String, String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub vars: Option<BTreeMap<String, serde_yaml::Value>>,
}

impl LinkDefinition {
    pub fn new(source_node: &str, source_port: &str, target_node: &str, target_port: &str) -> Self {
        Self {
            endpoints: [
                format!("{}:{}", source_node, source_port),
                format!("{}:{}", target_node, target_port),
            ],
            labels: None,
            vars: None,
        }
    }

    /// Parse endpoint string "node:interface"
    pub fn parse_endpoint(endpoint: &str) -> Option<(String, String)> {
        let parts: Vec<&str> = endpoint.split(':').collect();
        if parts.len() == 2 {
            Some((parts[0].trim().to_string(), parts[1].trim().to_string()))
        } else {
            None
        }
    }

    /// Get source endpoint (node, port)
    pub fn source(&self) -> Option<(String, String)> {
        Self::parse_endpoint(&self.endpoints[0])
    }

    /// Get target endpoint (node, port)
    pub fn target(&self) -> Option<(String, String)> {
        Self::parse_endpoint(&self.endpoints[1])
    }
}

/// Node kind template definition for quick addition in UI
#[derive(Debug, Clone, Copy)]
pub struct KindTemplate {
    pub kind_name: &'static str,
    pub display_name: &'static str,
    pub default_image: &'static str,
    pub default_ports: &'static [&'static str],
    pub interface_pattern: &'static str,
    pub description: &'static str,
    pub node_category: NodeCategory,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum NodeCategory {
    Router,
    Switch,
    Host,
    Firewall,
}

impl std::fmt::Display for NodeCategory {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            NodeCategory::Router => write!(f, "Router"),
            NodeCategory::Switch => write!(f, "Switch"),
            NodeCategory::Host => write!(f, "Host"),
            NodeCategory::Firewall => write!(f, "Firewall"),
        }
    }
}

impl NodeCategory {
    pub const ALL: [NodeCategory; 4] = [
        NodeCategory::Router,
        NodeCategory::Switch,
        NodeCategory::Host,
        NodeCategory::Firewall,
    ];

    pub fn parse(s: &str) -> Self {
        match s.to_lowercase().trim() {
            "switch" => NodeCategory::Switch,
            "host" => NodeCategory::Host,
            "firewall" => NodeCategory::Firewall,
            _ => NodeCategory::Router,
        }
    }

    pub fn next(&self) -> Self {
        match self {
            NodeCategory::Router => NodeCategory::Switch,
            NodeCategory::Switch => NodeCategory::Host,
            NodeCategory::Host => NodeCategory::Firewall,
            NodeCategory::Firewall => NodeCategory::Router,
        }
    }

    pub fn prev(&self) -> Self {
        match self {
            NodeCategory::Router => NodeCategory::Firewall,
            NodeCategory::Switch => NodeCategory::Router,
            NodeCategory::Host => NodeCategory::Switch,
            NodeCategory::Firewall => NodeCategory::Host,
        }
    }
}

/// Dynamic, user-customizable node profile (kind template)
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NodeProfile {
    pub kind_name: String,
    pub display_name: String,
    pub default_image: String,
    pub default_ports: Vec<String>,
    #[serde(default = "NodeProfile::default_interface_pattern")]
    pub interface_pattern: String,
    pub description: String,
    pub node_category: NodeCategory,
}

impl From<&KindTemplate> for NodeProfile {
    fn from(t: &KindTemplate) -> Self {
        Self {
            kind_name: t.kind_name.to_string(),
            display_name: t.display_name.to_string(),
            default_image: t.default_image.to_string(),
            default_ports: t.default_ports.iter().map(|s| s.to_string()).collect(),
            interface_pattern: t.interface_pattern.to_string(),
            description: t.description.to_string(),
            node_category: t.node_category,
        }
    }
}

/// Returns default interface pattern for nodes
pub fn default_interface_pattern() -> String {
    NodeProfile::default_interface_pattern()
}

impl NodeProfile {
    pub fn default_interface_pattern() -> String {
        "eth{n:1}".to_string()
    }

    pub fn default_pattern_for_kind(kind: &str) -> String {
        let canon = canonical_kind(kind);
        match canon {
            "nokia_srlinux" => "e1-{n:1}".to_string(),
            "cisco_c8000v" => "GigabitEthernet{n:2}".to_string(),
            "cisco_xrv9k" => "GigabitEthernet0/0/0/{n:0}".to_string(),
            "nokia_sros" => "1/1/{n:1}".to_string(),
            "juniper_vmx" => "ge-0/0/{n:0}".to_string(),
            _ => {
                let lower = kind.to_lowercase();
                if lower.contains("srlinux") || lower.contains("srl") {
                    "e1-{n:1}".to_string()
                } else if lower.contains("c8000") {
                    "GigabitEthernet{n:2}".to_string()
                } else if lower.contains("xrv") {
                    "GigabitEthernet0/0/0/{n:0}".to_string()
                } else if lower.contains("sros") || lower.contains("srsim") {
                    "1/1/{n:1}".to_string()
                } else if lower.contains("vmx") {
                    "ge-0/0/{n:0}".to_string()
                } else {
                    "eth{n:1}".to_string()
                }
            }
        }
    }

    pub fn matches_kind(&self, kind: &str) -> bool {
        kinds_match(&self.kind_name, kind)
    }

    pub fn new(
        kind_name: impl Into<String>,
        display_name: impl Into<String>,
        default_image: impl Into<String>,
        default_ports: Vec<String>,
        description: impl Into<String>,
        node_category: NodeCategory,
    ) -> Self {
        let kind = kind_name.into();
        let default_pattern = Self::default_pattern_for_kind(&kind);
        Self {
            kind_name: kind,
            display_name: display_name.into(),
            default_image: default_image.into(),
            default_ports,
            interface_pattern: default_pattern,
            description: description.into(),
            node_category,
        }
    }

    pub fn with_interface_pattern(mut self, pattern: impl Into<String>) -> Self {
        self.interface_pattern = pattern.into();
        self
    }

    /// Return the built-in preset default profiles
    pub fn default_profiles() -> Vec<Self> {
        KIND_TEMPLATES.iter().map(NodeProfile::from).collect()
    }
}

/// Resolves a kind alias or shorthand to official Containerlab schema canonical kind name
pub fn canonical_kind(kind: &str) -> &'static str {
    let s = kind.trim().to_lowercase();
    match s.as_str() {
        "srl" | "nokia_srlinux" | "srlinux" | "nokia-srlinux" => "nokia_srlinux",
        "ceos" | "arista_ceos" | "arista-ceos" => "arista_ceos",
        "c8000v" | "cisco_c8000v" | "cisco-c8000v" | "c8000" | "cisco_c8000" => "cisco_c8000v",
        "xrv9k" | "cisco_xrv9k" | "cisco-xrv9k" => "cisco_xrv9k",
        "xrv" | "cisco_xrv" | "cisco-xrv" | "vr-xrv" => "cisco_xrv",
        "crpd" | "juniper_crpd" | "juniper-crpd" => "juniper_crpd",
        "csrx" | "juniper_csrx" | "juniper-csrx" => "juniper_csrx",
        "sonic-vs" | "sonic_vs" | "sonic" => "sonic-vs",
        "sonic-vm" | "sonic_vm" => "sonic-vm",
        "linux" | "host" => "linux",
        "checkpoint" | "checkpoint_cloudguard" | "checkpoint-cloudguard" | "cloudguard" => {
            "checkpoint_cloudguard"
        }
        "sros" | "nokia_sros" | "nokia-sros" | "srsim" | "nokia_srsim" | "vr-sros" | "vr_sros" => {
            "nokia_sros"
        }
        "veos" | "arista_veos" | "arista-veos" | "vr-veos" | "vr_veos" => "arista_veos",
        "vmx" | "juniper_vmx" | "juniper-vmx" | "vr-vmx" | "vr_vmx" => "juniper_vmx",
        "vrr" | "juniper_vrr" | "juniper-vrr" | "vr-vrr" | "vr_vrr" => "juniper_vrr",
        "vqfx" | "juniper_vqfx" | "juniper-vqfx" | "vr-vqfx" => "juniper_vqfx",
        "vsrx" | "juniper_vsrx" | "juniper-vsrx" | "vr-vsrx" => "juniper_vsrx",
        "vjunosrouter" | "juniper_vjunosrouter" | "vr-vjunosrouter" => "juniper_vjunosrouter",
        "vjunosswitch" | "juniper_vjunosswitch" | "vr-vjunosswitch" => "juniper_vjunosswitch",
        "vjunosevolved" | "juniper_vjunosevolved" | "vr-vjunosevolved" => "juniper_vjunosevolved",
        "csr" | "csr1000v" | "cisco_csr1000v" | "cisco-csr1000v" | "vr-csr" => "cisco_csr1000v",
        "panos" | "paloalto_panos" | "paloalto-panos" | "pan" | "vr-pan" => "paloalto_panos",
        "mikrotik" | "mikrotik_ros" | "routeros" | "vr-ros" => "mikrotik_ros",
        "6wind" | "6wind_vsr" => "6wind_vsr",
        "n9kv" | "cisco_n9kv" | "cisco-n9kv" | "vr-n9kv" => "cisco_n9kv",
        "ftdv" | "cisco_ftdv" | "cisco-ftdv" | "vr-ftdv" => "cisco_ftdv",
        "ftosv" | "dell_ftosv" | "dell-ftosv" | "vr-ftosv" => "dell_ftosv",
        "dell_sonic" | "dell-sonic" => "dell_sonic",
        "aoscx" | "aruba_aoscx" | "aruba-aoscx" | "vr-aoscx" => "aruba_aoscx",
        "bridge" => "bridge",
        "ovs-bridge" | "ovs_bridge" => "ovs-bridge",
        "border0" => "border0",
        "ixia" | "keysight_ixia-c-one" | "ixia-c-one" => "keysight_ixia-c-one",
        "ocnos" | "ipinfusion_ocnos" | "ipinfusion-ocnos" => "ipinfusion_ocnos",
        "saos" | "ciena_saos" | "ciena-saos" => "ciena_saos",
        "ext-container" => "ext-container",
        "rare" => "rare",
        "frr" | "frrouting" => "frr",
        "xrd" | "cisco_xrd" | "cisco-xrd" => "cisco_xrd",
        "cisco_xrd_vrouter" => "cisco_xrd_vrouter",
        "cat9kv" | "cisco_cat9kv" | "cisco-cat9kv" | "vr-cat9kv" => "cisco_cat9kv",
        "iol" | "cisco_iol" | "cisco-iol" => "cisco_iol",
        "asav" | "cisco_asav" | "cisco-asav" | "vr-asav" => "cisco_asav",
        "vios" | "cisco_vios" | "cisco-vios" => "cisco_vios",
        "cumulus_vx" | "cumulusvx" | "cumulus-vx" | "nvidia_cumulusvx" | "cvx" => {
            "nvidia_cumulusvx"
        }
        "huawei" | "huawei_vrp" | "vrp" => "huawei_vrp",
        "openbsd" => "openbsd",
        "freebsd" => "freebsd",
        "generic_vm" => "generic_vm",
        "fortigate" | "fortinet_fortigate" | "fortinet-fortigate" => "fortinet_fortigate",
        "fortiproxy" | "fortinet_fortiproxy" | "fortinet-fortiproxy" => "fortinet_fortiproxy",
        "firewall" => "firewall",
        _ => "",
    }
}

/// Deduce node category from kind or alias
pub fn deduce_category(kind: &str) -> NodeCategory {
    let clean = kind.trim();
    if let Some(tmpl) = KIND_TEMPLATES.iter().find(|t| t.matches_kind(clean)) {
        return tmpl.node_category;
    }
    let canon = canonical_kind(clean);
    let target = if !canon.is_empty() { canon } else { clean };
    let lower = target.to_lowercase();

    if lower.contains("firewall")
        || lower.contains("fw")
        || lower.contains("checkpoint")
        || lower.contains("cloudguard")
        || lower.contains("forti")
        || lower.contains("palo")
        || lower.contains("panos")
        || lower.contains("vsrx")
        || lower.contains("csrx")
        || lower.contains("ftdv")
        || lower.contains("asav")
    {
        NodeCategory::Firewall
    } else if lower.contains("switch")
        || lower.contains("leaf")
        || lower.contains("eos")
        || lower.contains("cumulus")
        || lower.contains("sonic")
        || lower.contains("bridge")
        || lower.contains("aoscx")
        || lower.contains("ftosv")
        || lower.contains("vqfx")
        || lower.contains("vjunosswitch")
        || lower.contains("ciena")
        || lower.contains("saos")
    {
        NodeCategory::Switch
    } else if lower.contains("router")
        || lower.contains("spine")
        || lower.contains("srlinux")
        || lower.contains("srl")
        || lower.contains("sros")
        || lower.contains("srsim")
        || lower.contains("c8000")
        || lower.contains("xrv")
        || lower.contains("crpd")
        || lower.contains("vmx")
        || lower.contains("vrr")
        || lower.contains("csr")
        || lower.contains("xrd")
        || lower.contains("n9kv")
        || lower.contains("cat9k")
        || lower.contains("iol")
        || lower.contains("vios")
        || lower.contains("mikrotik")
        || lower.contains("ros")
        || lower.contains("huawei")
        || lower.contains("vrp")
        || lower.contains("6wind")
        || lower.contains("vsr")
        || lower.contains("ocnos")
        || lower.contains("vjunosrouter")
        || lower.contains("vjunosevolved")
        || lower.contains("frr")
    {
        NodeCategory::Router
    } else if lower.contains("host")
        || lower.contains("linux")
        || lower.contains("multitool")
        || lower.contains("alpine")
        || lower.contains("ubuntu")
        || lower.contains("debian")
        || lower.contains("centos")
    {
        NodeCategory::Host
    } else {
        NodeCategory::Router
    }
}

/// Check if two kind strings refer to the same kind either exactly or via alias resolution
pub fn kinds_match(a: &str, b: &str) -> bool {
    let a_clean = a.trim();
    let b_clean = b.trim();
    if a_clean.eq_ignore_ascii_case(b_clean) {
        return true;
    }
    let ca = canonical_kind(a_clean);
    let cb = canonical_kind(b_clean);
    if !ca.is_empty() && ca == cb {
        return true;
    }
    false
}

impl KindTemplate {
    pub fn matches_kind(&self, kind: &str) -> bool {
        kinds_match(self.kind_name, kind)
    }
}

pub static KIND_TEMPLATES: &[KindTemplate] = &[
    KindTemplate {
        kind_name: "nokia_srlinux",
        display_name: "Nokia SR Linux",
        default_image: "ghcr.io/nokia/srlinux:latest",
        default_ports: &["e1-1", "e1-2", "e1-3", "e1-4"],
        interface_pattern: "e1-{n:1}",
        description: "Modern network OS with open model-driven management",
        node_category: NodeCategory::Router,
    },
    KindTemplate {
        kind_name: "arista_ceos",
        display_name: "Arista cEOS",
        default_image: "ceos:latest",
        default_ports: &["eth1", "eth2", "eth3", "eth4"],
        interface_pattern: "eth{n:1}",
        description: "Arista containerized EOS switch/router",
        node_category: NodeCategory::Switch,
    },
    KindTemplate {
        kind_name: "cisco_c8000v",
        display_name: "Cisco Catalyst 8000v",
        default_image: "c8000v:latest",
        default_ports: &["GigabitEthernet2", "GigabitEthernet3", "GigabitEthernet4"],
        interface_pattern: "GigabitEthernet{n:2}",
        description: "Cisco enterprise virtual router",
        node_category: NodeCategory::Router,
    },
    KindTemplate {
        kind_name: "cisco_xrv9k",
        display_name: "Cisco XRv9k",
        default_image: "xrv9k:latest",
        default_ports: &["GigabitEthernet0/0/0/0", "GigabitEthernet0/0/0/1"],
        interface_pattern: "GigabitEthernet0/0/0/{n:0}",
        description: "Cisco IOS-XR virtual carrier-grade router",
        node_category: NodeCategory::Router,
    },
    KindTemplate {
        kind_name: "juniper_crpd",
        display_name: "Juniper cRPD",
        default_image: "crpd:latest",
        default_ports: &["eth1", "eth2", "eth3"],
        interface_pattern: "eth{n:1}",
        description: "Juniper containerized Routing Protocol Daemon",
        node_category: NodeCategory::Router,
    },
    KindTemplate {
        kind_name: "sonic-vs",
        display_name: "SONiC Virtual Switch",
        default_image: "docker-sonic-vs:latest",
        default_ports: &["eth1", "eth2", "eth3", "eth4"],
        interface_pattern: "eth{n:1}",
        description: "Software for Open Networking in the Cloud",
        node_category: NodeCategory::Switch,
    },
    KindTemplate {
        kind_name: "linux",
        display_name: "Generic Linux Host",
        default_image: "ghcr.io/srl-labs/network-multitool:latest",
        default_ports: &["eth1", "eth2"],
        interface_pattern: "eth{n:1}",
        description: "Network diagnostic host with curl, iperf3, tcpdump, etc.",
        node_category: NodeCategory::Host,
    },
    KindTemplate {
        kind_name: "checkpoint_cloudguard",
        display_name: "Check Point CloudGuard",
        default_image: "checkpoint:latest",
        default_ports: &["eth1", "eth2", "eth3"],
        interface_pattern: "eth{n:1}",
        description: "Check Point next-generation security firewall",
        node_category: NodeCategory::Firewall,
    },
    KindTemplate {
        kind_name: "nokia_sros",
        display_name: "Nokia SR OS",
        default_image: "sros:latest",
        default_ports: &["1/1/1", "1/1/2", "1/1/3"],
        interface_pattern: "1/1/{n:1}",
        description: "Nokia Service Router Operating System",
        node_category: NodeCategory::Router,
    },
    KindTemplate {
        kind_name: "arista_veos",
        display_name: "Arista vEOS",
        default_image: "veos:latest",
        default_ports: &["eth1", "eth2", "eth3", "eth4"],
        interface_pattern: "eth{n:1}",
        description: "Arista virtual EOS virtual machine router/switch",
        node_category: NodeCategory::Switch,
    },
    KindTemplate {
        kind_name: "juniper_vmx",
        display_name: "Juniper vMX",
        default_image: "vmx:latest",
        default_ports: &["ge-0/0/0", "ge-0/0/1", "ge-0/0/2"],
        interface_pattern: "ge-0/0/{n:0}",
        description: "Juniper virtual MX router",
        node_category: NodeCategory::Router,
    },
    KindTemplate {
        kind_name: "juniper_vrr",
        display_name: "Juniper vRR",
        default_image: "vr-vrr:latest",
        default_ports: &["eth1", "eth2", "eth3"],
        interface_pattern: "eth{n:1}",
        description: "Juniper virtual Route Reflector",
        node_category: NodeCategory::Router,
    },
    KindTemplate {
        kind_name: "nvidia_cumulusvx",
        display_name: "NVIDIA Cumulus VX",
        default_image: "cumulus-vx:latest",
        default_ports: &["eth1", "eth2", "eth3", "eth4"],
        interface_pattern: "eth{n:1}",
        description: "Open networking NOS for enterprise data centers",
        node_category: NodeCategory::Switch,
    },
    KindTemplate {
        kind_name: "firewall",
        display_name: "Generic Firewall Gateway",
        default_image: "ghcr.io/srl-labs/firewall:latest",
        default_ports: &["eth1", "eth2", "eth3"],
        interface_pattern: "eth{n:1}",
        description: "Network security inspection and packet filter gateway",
        node_category: NodeCategory::Firewall,
    },
];

/// Containerlab inspect JSON container structure
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ContainerInspectInfo {
    #[serde(rename = "lab_name", default)]
    pub lab_name: String,
    #[serde(rename = "labPath", default)]
    pub lab_path: Option<String>,
    #[serde(default)]
    pub name: String,
    #[serde(rename = "container_id", default)]
    pub container_id: Option<String>,
    #[serde(default)]
    pub image: String,
    #[serde(default)]
    pub kind: String,
    #[serde(default)]
    pub state: String,
    #[serde(rename = "ipv4_address", default)]
    pub ipv4_address: Option<String>,
    #[serde(rename = "ipv6_address", default)]
    pub ipv6_address: Option<String>,
}
