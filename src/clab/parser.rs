use crate::clab::model::{LabTopology, LinkDefinition, NodeDefinition, KIND_TEMPLATES};
use std::collections::{BTreeMap, HashSet};
use std::fs;
use std::path::Path;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum TopologyError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("YAML serialization/deserialization error: {0}")]
    Yaml(#[from] serde_yaml::Error),
    #[error("Validation error: {0}")]
    Validation(String),
}

/// Parser and validator for Containerlab topologies
pub struct TopologyParser;

impl TopologyParser {
    /// Parse topology from a YAML string
    pub fn parse_str(yaml_str: &str) -> Result<LabTopology, TopologyError> {
        let topo: LabTopology = serde_yaml::from_str(yaml_str)?;
        Self::validate(&topo)?;
        Ok(topo)
    }

    /// Parse topology from a YAML string (alias for parse_str)
    pub fn from_yaml_str(yaml_str: &str) -> Result<LabTopology, TopologyError> {
        Self::parse_str(yaml_str)
    }

    /// Parse topology from a file path
    pub fn parse_file<P: AsRef<Path>>(path: P) -> Result<LabTopology, TopologyError> {
        let content = fs::read_to_string(path)?;
        Self::parse_str(&content)
    }

    /// Serialize topology to YAML string
    pub fn serialize(topo: &LabTopology) -> Result<String, TopologyError> {
        Self::validate(topo)?;
        let yaml_str = serde_yaml::to_string(topo)?;
        Ok(yaml_str)
    }

    /// Save topology to file
    pub fn save_file<P: AsRef<Path>>(topo: &LabTopology, path: P) -> Result<(), TopologyError> {
        let yaml_str = Self::serialize(topo)?;
        fs::write(path, yaml_str)?;
        Ok(())
    }

    /// Validate topology consistency and Containerlab constraints
    pub fn validate(topo: &LabTopology) -> Result<(), TopologyError> {
        let trimmed_name = topo.name.trim();
        if trimmed_name.is_empty() {
            return Err(TopologyError::Validation(
                "Topology name cannot be empty".to_string(),
            ));
        }

        if trimmed_name.contains('/')
            || trimmed_name.contains('\\')
            || trimmed_name.contains("..")
            || trimmed_name.starts_with('.')
            || trimmed_name.starts_with('-')
        {
            return Err(TopologyError::Validation(format!(
                "Topology name '{}' contains path traversal, separator characters, or starts with '.' or '-'",
                topo.name
            )));
        }

        if !trimmed_name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-' || c == '.')
        {
            return Err(TopologyError::Validation(format!(
                "Topology name '{}' contains invalid characters. Must be alphanumeric, hyphen, underscore, or period.",
                topo.name
            )));
        }

        // Validate node definitions
        for node_name in topo.topology.nodes.keys() {
            let trimmed_node = node_name.trim();
            if trimmed_node.is_empty() {
                return Err(TopologyError::Validation(
                    "Node name cannot be empty".to_string(),
                ));
            }
            if trimmed_node.contains(':')
                || trimmed_node.contains('/')
                || trimmed_node.contains('\\')
            {
                return Err(TopologyError::Validation(format!(
                    "Node name '{}' contains forbidden characters (':', '/', '\\')",
                    node_name
                )));
            }
            if !trimmed_node
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
            {
                return Err(TopologyError::Validation(format!(
                    "Node name '{}' contains invalid characters. Must be alphanumeric, hyphen, or underscore.",
                    node_name
                )));
            }
        }

        let node_names: HashSet<&str> = topo.topology.nodes.keys().map(|s| s.as_str()).collect();

        // Track used ports: (node_name, port_name)
        let mut used_ports: HashSet<(String, String)> = HashSet::new();

        for (i, link) in topo.topology.links.iter().enumerate() {
            let (src_node, src_port) = link.source().ok_or_else(|| {
                TopologyError::Validation(format!(
                    "Invalid link #{}: bad source endpoint '{}'",
                    i + 1,
                    link.endpoints[0]
                ))
            })?;

            let (tgt_node, tgt_port) = link.target().ok_or_else(|| {
                TopologyError::Validation(format!(
                    "Invalid link #{}: bad target endpoint '{}'",
                    i + 1,
                    link.endpoints[1]
                ))
            })?;

            if src_port.trim().is_empty() || tgt_port.trim().is_empty() {
                return Err(TopologyError::Validation(format!(
                    "Link #{}: interface name cannot be empty",
                    i + 1
                )));
            }

            if src_port.contains(':') || tgt_port.contains(':') {
                return Err(TopologyError::Validation(format!(
                    "Link #{}: interface name cannot contain ':'",
                    i + 1
                )));
            }

            if !node_names.contains(src_node.as_str()) {
                return Err(TopologyError::Validation(format!(
                    "Link #{}: source node '{}' does not exist in topology",
                    i + 1,
                    src_node
                )));
            }

            if !node_names.contains(tgt_node.as_str()) {
                return Err(TopologyError::Validation(format!(
                    "Link #{}: target node '{}' does not exist in topology",
                    i + 1,
                    tgt_node
                )));
            }

            if src_node == tgt_node && src_port == tgt_port {
                return Err(TopologyError::Validation(format!(
                    "Link #{}: self-loop on '{}:{}' is not permitted",
                    i + 1,
                    src_node,
                    src_port
                )));
            }

            let src_key = (src_node.clone(), src_port.clone());
            if !used_ports.insert(src_key) {
                return Err(TopologyError::Validation(format!(
                    "Port '{}:{}' is connected more than once",
                    src_node, src_port
                )));
            }

            let tgt_key = (tgt_node.clone(), tgt_port.clone());
            if !used_ports.insert(tgt_key) {
                return Err(TopologyError::Validation(format!(
                    "Port '{}:{}' is connected more than once",
                    tgt_node, tgt_port
                )));
            }
        }

        Ok(())
    }

    /// List currently used interfaces for each node
    pub fn get_used_interfaces(topo: &LabTopology) -> BTreeMap<String, HashSet<String>> {
        let mut map: BTreeMap<String, HashSet<String>> = BTreeMap::new();
        for node in topo.topology.nodes.keys() {
            map.insert(node.clone(), HashSet::new());
        }

        for link in &topo.topology.links {
            if let Some((src_node, src_port)) = link.source() {
                map.entry(src_node).or_default().insert(src_port);
            }
            if let Some((tgt_node, tgt_port)) = link.target() {
                map.entry(tgt_node).or_default().insert(tgt_port);
            }
        }

        map
    }

    /// List available interfaces for a given node based on kind defaults and used interfaces
    pub fn get_available_interfaces(topo: &LabTopology, node_name: &str) -> Vec<String> {
        let node = match topo.topology.nodes.get(node_name) {
            Some(n) => n,
            None => return Vec::new(),
        };

        let used = Self::get_used_interfaces(topo);
        let used_set = used.get(node_name);

        let kind_str = node.kind.as_deref().unwrap_or("linux");
        let template = KIND_TEMPLATES.iter().find(|t| t.matches_kind(kind_str));

        let candidate_ports: Vec<String> = if let Some(tmpl) = template {
            tmpl.default_ports.iter().map(|s| s.to_string()).collect()
        } else {
            vec![
                "eth1".to_string(),
                "eth2".to_string(),
                "eth3".to_string(),
                "eth4".to_string(),
            ]
        };

        let mut available = Vec::new();
        for port in candidate_ports {
            if let Some(set) = used_set {
                if !set.contains(&port) {
                    available.push(port);
                }
            } else {
                available.push(port);
            }
        }

        // If all default ports are used, propose next sequential port
        if available.is_empty() {
            let next_index = used_set.map(|s| s.len() + 1).unwrap_or(1);
            if kind_str == "nokia_srlinux" || kind_str == "srl" {
                available.push(format!("e1-{}", next_index));
            } else {
                available.push(format!("eth{}", next_index));
            }
        }

        available
    }

    /// Creates a sample default leaf-spine topology
    pub fn create_sample_topology() -> LabTopology {
        let mut nodes = BTreeMap::new();

        let mut srl1 = NodeDefinition {
            kind: Some("nokia_srlinux".to_string()),
            image: Some("ghcr.io/nokia/srlinux:latest".to_string()),
            mgmt_ipv4: Some("172.20.20.11".to_string()),
            ..Default::default()
        };
        srl1.set_canvas_pos(15.0, 10.0);
        nodes.insert("srl1".to_string(), srl1);

        let mut srl2 = NodeDefinition {
            kind: Some("nokia_srlinux".to_string()),
            image: Some("ghcr.io/nokia/srlinux:latest".to_string()),
            mgmt_ipv4: Some("172.20.20.12".to_string()),
            ..Default::default()
        };
        srl2.set_canvas_pos(55.0, 10.0);
        nodes.insert("srl2".to_string(), srl2);

        let mut host1 = NodeDefinition {
            kind: Some("linux".to_string()),
            image: Some("ghcr.io/srl-labs/network-multitool:latest".to_string()),
            mgmt_ipv4: Some("172.20.20.21".to_string()),
            ..Default::default()
        };
        host1.set_canvas_pos(15.0, 25.0);
        nodes.insert("host1".to_string(), host1);

        let mut host2 = NodeDefinition {
            kind: Some("linux".to_string()),
            image: Some("ghcr.io/srl-labs/network-multitool:latest".to_string()),
            mgmt_ipv4: Some("172.20.20.22".to_string()),
            ..Default::default()
        };
        host2.set_canvas_pos(55.0, 25.0);
        nodes.insert("host2".to_string(), host2);

        let links = vec![
            LinkDefinition::new("srl1", "e1-1", "srl2", "e1-1"),
            LinkDefinition::new("host1", "eth1", "srl1", "e1-2"),
            LinkDefinition::new("host2", "eth1", "srl2", "e1-2"),
        ];

        LabTopology {
            name: "leaf-spine-demo".to_string(),
            prefix: None,
            mgmt: Some(crate::clab::model::MgmtConfig::default()),
            topology: crate::clab::model::TopologyData {
                defaults: None,
                kinds: None,
                nodes,
                links,
            },
        }
    }
}
