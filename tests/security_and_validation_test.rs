use clab_tui::canvas::node::CanvasNode;
use clab_tui::canvas::state::CanvasState;
use clab_tui::clab::client::ClabClient;
use clab_tui::clab::model::{LinkDefinition, NodeDefinition};
use clab_tui::clab::parser::{TopologyError, TopologyParser};
use clab_tui::graphics::kitty::KittyGraphRenderer;
use clab_tui::ui::theme::Theme;
use clab_tui::ui::widgets::log_viewer::LogViewer;
use clab_tui::ui::widgets::toast::ToastManager;
use std::time::Duration;

#[test]
fn test_security_path_traversal_in_topology_name() {
    let mut topo = TopologyParser::create_sample_topology();

    let invalid_names = [
        "../../etc/passwd",
        "/etc/shadow",
        "..",
        ".hidden_lab",
        r"C:\Windows\System32",
        "lab/sublab",
        "lab\\sublab",
        "lab name with spaces",
        "lab;reboot",
        "lab|curl",
        "lab`whoami`",
        "lab$HOME",
    ];

    for name in invalid_names {
        topo.name = name.to_string();
        let res = TopologyParser::validate(&topo);
        assert!(
            res.is_err(),
            "Topology name '{}' should have failed security validation",
            name
        );
    }
}

#[test]
fn test_security_valid_topology_names() {
    let mut topo = TopologyParser::create_sample_topology();

    let valid_names = [
        "clab-quickstart",
        "leaf_spine_01",
        "lab-v1.2",
        "MyLab-2026",
        "datacenter-spine-leaf",
    ];

    for name in valid_names {
        topo.name = name.to_string();
        assert!(
            TopologyParser::validate(&topo).is_ok(),
            "Valid topology name '{}' was rejected",
            name
        );
    }
}

#[test]
fn test_security_node_name_validation() {
    let mut topo = TopologyParser::create_sample_topology();

    // Node name with colon
    let bad_node = NodeDefinition {
        kind: Some("linux".to_string()),
        ..Default::default()
    };
    topo.topology
        .nodes
        .insert("node:bad".to_string(), bad_node.clone());

    let err = TopologyParser::validate(&topo).unwrap_err();
    assert!(matches!(err, TopologyError::Validation(_)));

    // Node name with path traversal
    topo.topology.nodes.remove("node:bad");
    topo.topology
        .nodes
        .insert("../escape".to_string(), bad_node.clone());
    let err = TopologyParser::validate(&topo).unwrap_err();
    assert!(matches!(err, TopologyError::Validation(_)));

    // Empty node name
    topo.topology.nodes.remove("../escape");
    topo.topology.nodes.insert("".to_string(), bad_node);
    let err = TopologyParser::validate(&topo).unwrap_err();
    assert!(matches!(err, TopologyError::Validation(_)));
}

#[test]
fn test_security_port_name_validation() {
    let mut topo = TopologyParser::create_sample_topology();

    // Port with colon
    topo.topology.links.push(LinkDefinition {
        endpoints: ["srl1:e1:1".to_string(), "host1:eth1".to_string()],
        labels: None,
        vars: None,
    });
    assert!(TopologyParser::validate(&topo).is_err());

    // Port with empty string
    let mut topo2 = TopologyParser::create_sample_topology();
    topo2.topology.links.push(LinkDefinition {
        endpoints: ["srl1:".to_string(), "host1:eth1".to_string()],
        labels: None,
        vars: None,
    });
    assert!(TopologyParser::validate(&topo2).is_err());
}

#[test]
fn test_security_identifier_validation() {
    // Valid identifiers
    assert!(ClabClient::validate_identifier("srl1", "Node").is_ok());
    assert!(ClabClient::validate_identifier("eth1-1", "Interface").is_ok());
    assert!(ClabClient::validate_identifier("GigabitEthernet0/0/0/0", "Interface").is_ok());

    // Invalid identifiers (flag injection or empty)
    assert!(ClabClient::validate_identifier("-it", "Node").is_err());
    assert!(ClabClient::validate_identifier("--privileged", "Node").is_err());
    assert!(ClabClient::validate_identifier("", "Node").is_err());
    assert!(ClabClient::validate_identifier("node;reboot", "Node").is_err());
}

#[test]
fn test_inspect_json_parsing_variations() {
    // 1. Empty / null representation
    let empty = ClabClient::parse_inspect_json("{}").unwrap();
    assert!(empty.is_empty());

    let empty_arr = ClabClient::parse_inspect_json("[]").unwrap();
    assert!(empty_arr.is_empty());

    // 2. Map-based JSON output (containerlab standard)
    let map_json = r#"{
        "leaf-spine-demo": [
            {
                "lab_name": "leaf-spine-demo",
                "labPath": "/etc/clab/leaf-spine-demo/topo.yml",
                "name": "clab-leaf-spine-demo-srl1",
                "container_id": "abc123456789",
                "image": "ghcr.io/nokia/srlinux:latest",
                "kind": "srl",
                "state": "running",
                "ipv4_address": "172.20.20.11/24",
                "ipv6_address": "2001:172:20:20::11/64"
            }
        ]
    }"#;
    let list = ClabClient::parse_inspect_json(map_json).unwrap();
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].name, "clab-leaf-spine-demo-srl1");
    assert_eq!(list[0].kind, "srl");
    assert_eq!(list[0].ipv4_address.as_deref(), Some("172.20.20.11/24"));

    // 3. Array-based JSON output
    let arr_json = r#"[
        {
            "lab_name": "lab1",
            "name": "clab-lab1-host1",
            "image": "alpine:latest",
            "kind": "linux",
            "state": "running"
        }
    ]"#;
    let list_arr = ClabClient::parse_inspect_json(arr_json).unwrap();
    assert_eq!(list_arr.len(), 1);
    assert_eq!(list_arr[0].name, "clab-lab1-host1");
}

#[test]
fn test_kitty_graphics_raster_generation() {
    let topo = TopologyParser::create_sample_topology();
    let mut canvas = CanvasState::new();
    canvas.load_from_topology(&topo);
    let theme = Theme::tokyo_night();

    // Render image buffer
    let img = KittyGraphRenderer::render_image(&canvas, 400, 300, &theme);
    assert_eq!(img.width(), 400);
    assert_eq!(img.height(), 300);

    // Escape sequence generation
    let escape_seq = KittyGraphRenderer::generate_kitty_escape_sequence(&img).unwrap();
    assert!(escape_seq.starts_with("\x1b_Gf=100,a=T,m=0;"));
    assert!(escape_seq.ends_with("\x1b\\"));
}

#[test]
fn test_toast_manager_lifecycle() {
    let mut mgr = ToastManager::new();
    mgr.info("Info message");
    mgr.warning("Warning message");
    mgr.error("Error message");

    // Before tick
    assert_eq!(mgr.toasts.len(), 3);

    // Simulate expiration
    for toast in &mut mgr.toasts {
        toast.duration = Duration::from_millis(1);
    }
    std::thread::sleep(Duration::from_millis(5));
    mgr.tick();

    // Expired toasts cleaned up
    assert!(mgr.toasts.is_empty());
}

#[test]
fn test_log_viewer_filtering_and_scroll() {
    let mut viewer = LogViewer::new();
    viewer.push_line("INFO[0000] Starting lab provisioning");
    viewer.push_line("WARN[0001] Container restart delayed");
    viewer.push_line("ERROR[0002] Port already allocated");

    assert_eq!(viewer.lines.len(), 3);
    assert_eq!(viewer.filtered_lines().len(), 3);

    // Filter by ERROR
    viewer.filter = "ERROR".to_string();
    assert_eq!(viewer.filtered_lines().len(), 1);
    assert_eq!(
        viewer.filtered_lines()[0],
        "ERROR[0002] Port already allocated"
    );

    // Scroll operations
    viewer.scroll_up(5);
    assert_eq!(viewer.scroll_offset, 0);

    viewer.scroll_down(5, 10);
    assert_eq!(viewer.scroll_offset, 0);

    viewer.clear();
    assert!(viewer.lines.is_empty());
}

#[test]
fn test_canvas_node_geometry_and_hit_test() {
    let def = NodeDefinition {
        kind: Some("srl".to_string()),
        ..Default::default()
    };
    let node = CanvasNode::new("srl1", &def, 10.0, 10.0);

    // Bounding box hit test
    assert!(node.contains_point(15.0, 12.0));
    assert!(!node.contains_point(5.0, 5.0));
    assert!(!node.contains_point(40.0, 40.0));

    // Port anchor search
    assert!(node.find_port_near(10.0, 11.5, 2.0).is_some());
    assert!(node.find_port_near(0.0, 0.0, 1.0).is_none());
}
