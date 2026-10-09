use clab_tui::clab::model::LinkDefinition;
use clab_tui::clab::parser::{TopologyError, TopologyParser};

#[test]
fn test_parse_and_serialize_roundtrip() {
    let sample = TopologyParser::create_sample_topology();
    let yaml_str = TopologyParser::serialize(&sample).expect("Serialization failed");

    let parsed = TopologyParser::parse_str(&yaml_str).expect("Parsing failed");
    assert_eq!(parsed.name, sample.name);
    assert_eq!(parsed.topology.nodes.len(), sample.topology.nodes.len());
    assert_eq!(parsed.topology.links.len(), sample.topology.links.len());
}

#[test]
fn test_parse_demo_example_file() {
    let demo_path = std::path::Path::new("examples/demo.clab.yml");
    let topo = TopologyParser::parse_file(demo_path).expect("Failed to parse demo.clab.yml");
    assert_eq!(topo.name, "clab-demo");
    assert_eq!(topo.topology.nodes.len(), 8);
    assert_eq!(topo.topology.links.len(), 9);
}

#[test]
fn test_parse_official_srl_leaf_spine() {
    let yaml = r#"
name: srl02
mgmt:
  network: clab-mgmt
  ipv4-subnet: 172.20.20.0/24

topology:
  kinds:
    srl:
      image: ghcr.io/nokia/srlinux:latest
    linux:
      image: ghcr.io/srl-labs/network-multitool:latest

  nodes:
    spine1:
      kind: srl
      type: ixr6
      mgmt-ipv4: 172.20.20.101
      labels:
        clab-tui-x: "20.0"
        clab-tui-y: "10.0"
    spine2:
      kind: srl
      type: ixr6
      mgmt-ipv4: 172.20.20.102
      labels:
        clab-tui-x: "60.0"
        clab-tui-y: "10.0"
    leaf1:
      kind: srl
      mgmt-ipv4: 172.20.20.11
      labels:
        clab-tui-x: "10.0"
        clab-tui-y: "25.0"
    leaf2:
      kind: srl
      mgmt-ipv4: 172.20.20.12
      labels:
        clab-tui-x: "40.0"
        clab-tui-y: "25.0"
    client1:
      kind: linux
      mgmt-ipv4: 172.20.20.21
      labels:
        clab-tui-x: "10.0"
        clab-tui-y: "40.0"

  links:
    - endpoints: ["leaf1:e1-1", "spine1:e1-1"]
    - endpoints: ["leaf1:e1-2", "spine2:e1-1"]
    - endpoints: ["leaf2:e1-1", "spine1:e1-2"]
    - endpoints: ["leaf2:e1-2", "spine2:e1-2"]
    - endpoints: ["client1:eth1", "leaf1:e1-3"]
"#;

    let topo = TopologyParser::parse_str(yaml).expect("Failed to parse official topology");
    assert_eq!(topo.name, "srl02");
    assert_eq!(topo.topology.nodes.len(), 5);
    assert_eq!(topo.topology.links.len(), 5);

    let spine1 = &topo.topology.nodes["spine1"];
    assert_eq!(spine1.canvas_x(), Some(20.0));
    assert_eq!(spine1.canvas_y(), Some(10.0));
    assert_eq!(spine1.mgmt_ipv4.as_deref(), Some("172.20.20.101"));
}

#[test]
fn test_parse_multi_vendor_topology() {
    let yaml = r#"
name: multi-vendor-lab
topology:
  nodes:
    srl1:
      kind: srl
      image: ghcr.io/nokia/srlinux:latest
    ceos1:
      kind: ceos
      image: ceos:4.30.0F
    crpd1:
      kind: crpd
      image: crpd:23.2R1
    host1:
      kind: linux
      image: alpine:latest
  links:
    - endpoints: ["srl1:e1-1", "ceos1:eth1"]
    - endpoints: ["ceos1:eth2", "crpd1:eth1"]
    - endpoints: ["crpd1:eth2", "host1:eth1"]
"#;

    let topo = TopologyParser::parse_str(yaml).expect("Failed to parse multi-vendor topology");
    assert_eq!(topo.topology.nodes.len(), 4);
    assert_eq!(topo.topology.links.len(), 3);
}

#[test]
fn test_validation_empty_name() {
    let mut topo = TopologyParser::create_sample_topology();
    topo.name = "   ".to_string();
    let err = TopologyParser::validate(&topo).unwrap_err();
    assert!(matches!(err, TopologyError::Validation(_)));
}

#[test]
fn test_validation_missing_node_in_link() {
    let mut topo = TopologyParser::create_sample_topology();
    topo.topology
        .links
        .push(LinkDefinition::new("nonexistent", "eth1", "srl1", "e1-4"));
    let err = TopologyParser::validate(&topo).unwrap_err();
    assert!(matches!(err, TopologyError::Validation(_)));
}

#[test]
fn test_validation_duplicate_port_connection() {
    let mut topo = TopologyParser::create_sample_topology();
    // srl1:e1-1 is already connected in sample topology
    topo.topology
        .links
        .push(LinkDefinition::new("srl1", "e1-1", "host1", "eth2"));
    let err = TopologyParser::validate(&topo).unwrap_err();
    assert!(matches!(err, TopologyError::Validation(_)));
}

#[test]
fn test_validation_self_loop() {
    let mut topo = TopologyParser::create_sample_topology();
    topo.topology
        .links
        .push(LinkDefinition::new("srl1", "e1-3", "srl1", "e1-3"));
    let err = TopologyParser::validate(&topo).unwrap_err();
    assert!(matches!(err, TopologyError::Validation(_)));
}

#[test]
fn test_available_and_used_interfaces() {
    let topo = TopologyParser::create_sample_topology();
    let used = TopologyParser::get_used_interfaces(&topo);

    // srl1 uses e1-1 and e1-2 in sample
    assert!(used["srl1"].contains("e1-1"));
    assert!(used["srl1"].contains("e1-2"));

    // Available interfaces should have e1-3 and e1-4
    let avail = TopologyParser::get_available_interfaces(&topo, "srl1");
    assert!(avail.contains(&"e1-3".to_string()));
    assert!(avail.contains(&"e1-4".to_string()));
    assert!(!avail.contains(&"e1-1".to_string()));
}
