use clab_tui::canvas::state::CanvasState;
use clab_tui::clab::model::{LabTopology, NodeCategory, NodeDefinition};
use clab_tui::clab::parser::TopologyParser;
use clab_tui::graphics::kitty::{GraphicsDetector, GraphicsProtocol, KittyGraphRenderer};
use clab_tui::ui::theme::Theme;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;

fn create_full_categories_topology() -> LabTopology {
    let mut topo = TopologyParser::create_sample_topology();

    // Add a switch node (ceos)
    let mut switch_node = NodeDefinition {
        kind: Some("ceos".to_string()),
        image: Some("ceos:latest".to_string()),
        mgmt_ipv4: Some("172.20.20.15".to_string()),
        ..Default::default()
    };
    switch_node.set_canvas_pos(55.0, 25.0);
    topo.topology.nodes.insert("leaf1".to_string(), switch_node);

    // Add a firewall node (sros or generic firewall)
    let mut fw_node = NodeDefinition {
        kind: Some("checkpoint".to_string()),
        image: Some("checkpoint:latest".to_string()),
        mgmt_ipv4: Some("172.20.20.50".to_string()),
        ..Default::default()
    };
    fw_node.set_canvas_pos(35.0, 18.0);
    topo.topology.nodes.insert("fw1".to_string(), fw_node);

    topo
}

#[test]
fn test_kitty_all_categories_raster_rendering() {
    let topo = create_full_categories_topology();
    let mut canvas = CanvasState::new();
    canvas.load_from_topology(&topo);
    let theme = Theme::tokyo_night();

    // Verify all 4 categories exist in canvas
    let categories: Vec<NodeCategory> = canvas.nodes.values().map(|n| n.category).collect();
    assert!(categories.contains(&NodeCategory::Router));
    assert!(categories.contains(&NodeCategory::Switch));
    assert!(categories.contains(&NodeCategory::Host));
    assert!(categories.contains(&NodeCategory::Firewall));

    // Render high-res image (800x600)
    let img = KittyGraphRenderer::render_image(&canvas, 800, 600, &theme);
    assert_eq!(img.width(), 800);
    assert_eq!(img.height(), 600);

    // Verify that key Tokyo night and category colors exist in pixels
    let mut found_router_cyan = false;
    let mut found_switch_teal = false;
    let mut found_host_purple = false;
    let mut found_fw_coral = false;
    let mut found_link_blue = false;
    let mut found_white_text = false;

    for pixel in img.pixels() {
        let [r, g, b, _] = pixel.0;
        // Router cyan: (125, 207, 255)
        if r == 125 && g == 207 && b == 255 {
            found_router_cyan = true;
        }
        // Switch teal: (115, 218, 202)
        if r == 115 && g == 218 && b == 202 {
            found_switch_teal = true;
        }
        // Host purple: (187, 154, 247)
        if r == 187 && g == 154 && b == 247 {
            found_host_purple = true;
        }
        // Firewall coral: (247, 118, 142)
        if r == 247 && g == 118 && b == 142 {
            found_fw_coral = true;
        }
        // Link blue: (122, 162, 247)
        if r == 122 && g == 162 && b == 247 {
            found_link_blue = true;
        }
        // White text / arrow core: (255, 255, 255)
        if r == 255 && g == 255 && b == 255 {
            found_white_text = true;
        }
    }

    assert!(
        found_router_cyan,
        "Router SVG iconography cyan color should be present"
    );
    assert!(
        found_switch_teal,
        "Switch SVG iconography teal color should be present"
    );
    assert!(
        found_host_purple,
        "Host SVG iconography purple color should be present"
    );
    assert!(
        found_fw_coral,
        "Firewall SVG iconography coral color should be present"
    );
    assert!(
        found_link_blue,
        "Straight link line blue color should be present"
    );
    assert!(
        found_white_text,
        "White text / arrowhead elements should be present"
    );
}

#[test]
fn test_kitty_escape_sequence_chunked_and_placed() {
    let topo = TopologyParser::create_sample_topology();
    let mut canvas = CanvasState::new();
    canvas.load_from_topology(&topo);
    let theme = Theme::tokyo_night();

    let img = KittyGraphRenderer::render_image(&canvas, 400, 300, &theme);

    // Standard sequence
    let std_seq = KittyGraphRenderer::generate_kitty_escape_sequence(&img).unwrap();
    assert!(std_seq.starts_with("\x1b_Gf=100,a=T,m=0;"));
    assert!(std_seq.ends_with("\x1b\\"));

    // Placed sequence
    let placed_seq =
        KittyGraphRenderer::generate_kitty_escape_sequence_placed(&img, 10, 5, 80, 24).unwrap();
    assert!(placed_seq.starts_with("\x1b_Ga=T,f=100,c=80,r=24,X=10,Y=5,m=0;"));
    assert!(placed_seq.ends_with("\x1b\\"));

    // Chunked sequences
    let chunks = KittyGraphRenderer::generate_kitty_escape_sequence_chunked(&img, 2048).unwrap();
    assert!(
        chunks.len() > 1,
        "PNG should be split across multiple 2048-byte chunks"
    );
    for (idx, chunk) in chunks.iter().enumerate() {
        assert!(chunk.starts_with("\x1b_G"));
        assert!(chunk.ends_with("\x1b\\"));
        if idx < chunks.len() - 1 {
            assert!(chunk.contains("m=1;"));
        } else {
            assert!(chunk.contains("m=0;"));
        }
    }

    // Direct transmission to writer
    let mut output = Vec::new();
    KittyGraphRenderer::transmit_kitty_image(&mut output, &img).unwrap();
    assert!(!output.is_empty());
    assert!(output.starts_with(b"\x1b_Gf=100,a=T,m=0;"));
}

#[test]
fn test_kitty_render_to_buffer_pill_badges_and_icons() {
    let topo = create_full_categories_topology();
    let mut canvas = CanvasState::new();
    canvas.load_from_topology(&topo);
    let theme = Theme::tokyo_night();

    let area = Rect::new(0, 0, 120, 45);
    let mut buf = Buffer::empty(area);

    KittyGraphRenderer::render_to_buffer(&canvas, area, &mut buf, &theme);

    // Collect buffer string
    let mut full_text = String::new();
    for y in 0..area.height {
        for x in 0..area.width {
            full_text.push_str(buf[(x, y)].symbol());
        }
        full_text.push('\n');
    }

    // 1. Half-block character '▀' must be present for raster display
    assert!(
        full_text.contains('▀'),
        "Half-blocks should be blitted across buffer"
    );

    // 2. Node titles must be present
    assert!(full_text.contains("srl1"));
    assert!(full_text.contains("host1"));
    assert!(full_text.contains("leaf1"));
    assert!(full_text.contains("fw1"));

    // 3. SVG iconography symbols must be present
    assert!(
        full_text.contains('⨁'),
        "Router multidirectional icon should be present"
    );
    assert!(
        full_text.contains('⇆'),
        "Switch opposing arrows icon should be present"
    );
    assert!(
        full_text.contains('🖳'),
        "Host server icon should be present"
    );
    assert!(
        full_text.contains('🛡'),
        "Firewall shield icon should be present"
    );

    // 4. Interface pill badges must be present
    assert!(
        full_text.contains("[e1-1]"),
        "Source interface pill badge [e1-1] should be present"
    );
    assert!(
        full_text.contains("[eth1]"),
        "Target interface pill badge [eth1] should be present"
    );

    // 5. Kind pill badges must be present
    assert!(full_text.contains("[nokia_srlinux]") || full_text.contains("[srl]"));
    assert!(full_text.contains("[linux]"));

    // 6. Straight rendered lines must be present
    assert!(
        full_text.contains('─'),
        "Horizontal straight line segments should be present"
    );
    assert!(
        full_text.contains('│'),
        "Vertical straight line segments should be present"
    );
    assert!(
        full_text.contains('╭') || full_text.contains('╰'),
        "Waypoint corner connectors should be present"
    );
}

#[test]
fn test_kitty_render_ratatui_image_widget() {
    let topo = create_full_categories_topology();
    let mut canvas = CanvasState::new();
    canvas.load_from_topology(&topo);
    let theme = Theme::tokyo_night();

    let area = Rect::new(0, 0, 80, 24);

    // 1. Test Halfblocks protocol via ratatui-image
    let mut buf_hb = Buffer::empty(area);
    let res_hb =
        KittyGraphRenderer::render_ratatui_image(&canvas, area, &mut buf_hb, &theme, false);
    assert!(
        res_hb.is_ok(),
        "ratatui-image halfblocks render should succeed"
    );

    let mut full_text_hb = String::new();
    for y in 0..area.height {
        for x in 0..area.width {
            full_text_hb.push_str(buf_hb[(x, y)].symbol());
        }
        full_text_hb.push('\n');
    }
    assert!(
        full_text_hb.contains('▀'),
        "Halfblocks widget should blit '▀' characters"
    );

    // 2. Test Kitty graphics protocol via ratatui-image
    let mut buf_kitty = Buffer::empty(area);
    let res_kitty =
        KittyGraphRenderer::render_ratatui_image(&canvas, area, &mut buf_kitty, &theme, true);
    assert!(
        res_kitty.is_ok(),
        "ratatui-image kitty protocol render should succeed"
    );

    // In Kitty protocol, cell (0, 0) contains the escape sequence prefix \x1b_G
    let first_cell_symbol = buf_kitty[(0, 0)].symbol();
    assert!(
        first_cell_symbol.contains("\x1b_G"),
        "Kitty protocol widget should transmit Kitty escape sequence in first cell"
    );
}

#[test]
fn test_kitty_render_with_protocol_dispatch() {
    let topo = create_full_categories_topology();
    let mut canvas = CanvasState::new();
    canvas.load_from_topology(&topo);
    let theme = Theme::tokyo_night();

    let area = Rect::new(0, 0, 100, 30);
    let mut buf = Buffer::empty(area);

    // In a test runner (where stdout is not a live terminal), render_with_protocol
    // dispatches seamlessly to high-res buffer rasterization
    KittyGraphRenderer::render_with_protocol(
        &canvas,
        area,
        &mut buf,
        &theme,
        GraphicsProtocol::Kitty,
    );

    let mut full_text = String::new();
    for y in 0..area.height {
        for x in 0..area.width {
            full_text.push_str(buf[(x, y)].symbol());
        }
        full_text.push('\n');
    }

    assert!(full_text.contains('▀'));
    assert!(full_text.contains("srl1"));
    assert!(full_text.contains('⨁'));
    assert!(full_text.contains('─'));
    assert!(full_text.contains("[e1-1]"));
}

#[test]
fn test_graphics_detector_and_protocol() {
    // Protocol preferred should return either Kitty or BrailleUnicodeFallback
    let proto = GraphicsDetector::get_preferred_protocol();
    assert!(proto == GraphicsProtocol::Kitty || proto == GraphicsProtocol::BrailleUnicodeFallback);
}

#[test]
fn test_kitty_diagonal_raster_antialiased_lines() {
    use image::{ImageBuffer, Rgba};

    let mut img: ImageBuffer<Rgba<u8>, Vec<u8>> =
        ImageBuffer::from_pixel(100, 100, Rgba([0, 0, 0, 255]));

    let line_col = Rgba([122, 162, 247, 255]);

    // Draw diagonal line from (10, 10) to (80, 80)
    KittyGraphRenderer::draw_line_px(&mut img, 10, 10, 80, 80, line_col, 2);

    // Center pixel of the line must have the full color
    let center_px = img.get_pixel(45, 45);
    assert_eq!(center_px[0], 122);
    assert_eq!(center_px[1], 162);
    assert_eq!(center_px[2], 247);

    // Adjacent pixels along the antialiased falloff boundary must be blended smoothly
    // (i.e. partially transparent or blended between 0 and 255)
    let mut found_antialiased_pixel = false;
    for y in 43..=47 {
        for x in 43..=47 {
            let px = img.get_pixel(x, y);
            // Non-zero, but not full line color
            if px[0] > 0 && px[0] < 122 {
                found_antialiased_pixel = true;
            }
        }
    }
    assert!(
        found_antialiased_pixel,
        "Subpixel antialiasing must produce smooth fractional color transitions on diagonal lines"
    );
}

#[test]
fn test_kitty_render_to_buffer_diagonal_lines() {
    let topo = create_full_categories_topology();
    let mut canvas = CanvasState::new();
    canvas.load_from_topology(&topo);
    // Add a diagonal cross link between srl1 (15, 10) and host2 (55, 25)
    canvas.add_link("srl1", "e1-3", "host2", "eth2");
    // Switch routing style to Direct diagonal
    canvas.set_routing_style(clab_tui::canvas::link::RoutingStyle::Direct);

    let theme = Theme::tokyo_night();
    let area = Rect::new(0, 0, 120, 45);
    let mut buf = Buffer::empty(area);

    KittyGraphRenderer::render_to_buffer(&canvas, area, &mut buf, &theme);

    let mut full_text = String::new();
    for y in 0..area.height {
        for x in 0..area.width {
            full_text.push_str(buf[(x, y)].symbol());
        }
        full_text.push('\n');
    }

    // Both raster half-blocks and diagonal characters must be present
    assert!(
        full_text.contains('▀'),
        "Half-blocks should be blitted across buffer"
    );
    assert!(
        full_text.contains('╲') || full_text.contains('╱'),
        "Appropriate diagonal characters should be overlaid for diagonal links in render_to_buffer"
    );
}

#[test]
fn test_kitty_wiring_preview_direct_and_octilinear() {
    let topo = create_full_categories_topology();
    let mut canvas = CanvasState::new();
    canvas.load_from_topology(&topo);
    canvas.mode = clab_tui::canvas::state::CanvasMode::Wiring;
    canvas.wiring_source = Some(("srl1".to_string(), "e1-1".to_string()));
    canvas.wiring_cursor = (50.0, 30.0);
    canvas.routing_style = clab_tui::canvas::link::RoutingStyle::Direct;

    let theme = Theme::tokyo_night();

    // Render image with active direct diagonal wiring preview
    let img_direct = KittyGraphRenderer::render_image(&canvas, 800, 600, &theme);
    assert_eq!(img_direct.width(), 800);
    assert_eq!(img_direct.height(), 600);

    // Verify gold wire color exists in raster
    let mut found_wire_gold = false;
    for pixel in img_direct.pixels() {
        let [r, g, b, _] = pixel.0;
        // Wire gold: (224, 175, 104)
        if r == 224 && g == 175 && b == 104 {
            found_wire_gold = true;
            break;
        }
    }
    assert!(
        found_wire_gold,
        "Direct diagonal wiring preview should render gold pixels in raster"
    );

    // Test octilinear wiring preview
    canvas.routing_style = clab_tui::canvas::link::RoutingStyle::Octilinear;
    let img_octi = KittyGraphRenderer::render_image(&canvas, 800, 600, &theme);
    let mut found_octi_gold = false;
    for pixel in img_octi.pixels() {
        let [r, g, b, _] = pixel.0;
        if r == 224 && g == 175 && b == 104 {
            found_octi_gold = true;
            break;
        }
    }
    assert!(
        found_octi_gold,
        "Octilinear wiring preview should render gold pixels in raster"
    );
}

#[test]
fn test_kitty_draw_line_px_zero_and_degenerate_image() {
    use image::{ImageBuffer, Rgba};
    let mut empty_img: ImageBuffer<Rgba<u8>, Vec<u8>> = ImageBuffer::new(0, 0);
    // Should safely return without panic
    KittyGraphRenderer::draw_line_px(&mut empty_img, 0, 0, 10, 10, Rgba([255, 255, 255, 255]), 2);

    let mut degenerate_img: ImageBuffer<Rgba<u8>, Vec<u8>> = ImageBuffer::new(0, 50);
    KittyGraphRenderer::draw_line_px(
        &mut degenerate_img,
        0,
        0,
        10,
        10,
        Rgba([255, 255, 255, 255]),
        2,
    );
}

#[test]
fn test_kitty_render_to_buffer_diagonal_pill_badge_placement() {
    use clab_tui::canvas::link::CanvasLink;

    let mut canvas = CanvasState::new();
    let theme = Theme::tokyo_night();
    let area = Rect::new(0, 0, 80, 40);
    let mut buf = Buffer::empty(area);

    // Link heading down-left: from (50, 10) to (20, 30)
    let link_down_left = CanvasLink {
        source_node: "n1".to_string(),
        source_port: "e1".to_string(),
        target_node: "n2".to_string(),
        target_port: "e2".to_string(),
        waypoints: vec![(50.0, 10.0), (20.0, 30.0)],
        routing_style: clab_tui::canvas::link::RoutingStyle::Direct,
    };
    canvas.links.push(link_down_left);

    KittyGraphRenderer::render_to_buffer(&canvas, area, &mut buf, &theme);

    // For link going left (sx1 < sx0), source badge [e1] must be placed to the left (x < 50)
    let mut found_left_badge = false;
    for x in 40..50 {
        if buf[(x, 11)].symbol() == "[" || buf[(x, 11)].symbol() == "e" {
            found_left_badge = true;
            break;
        }
    }
    assert!(
        found_left_badge,
        "Source badge for down-left wire must be placed to the left of the anchor (x < 50)"
    );
}

#[test]
fn test_kitty_render_to_buffer_link_background_color() {
    use clab_tui::canvas::link::CanvasLink;
    use ratatui::style::Color;

    let mut canvas = CanvasState::new();
    let theme = Theme::tokyo_night();
    let area = Rect::new(0, 0, 40, 20);
    let mut buf = Buffer::empty(area);

    canvas.links.push(CanvasLink {
        source_node: "a".to_string(),
        source_port: "p1".to_string(),
        target_node: "b".to_string(),
        target_port: "p2".to_string(),
        waypoints: vec![(5.0, 5.0), (25.0, 5.0)],
        routing_style: clab_tui::canvas::link::RoutingStyle::Orthogonal,
    });

    KittyGraphRenderer::render_to_buffer(&canvas, area, &mut buf, &theme);

    // Overlaid straight link cells must have the background color #1a1b26 (Rgb(26, 27, 38))
    let cell = &buf[(10, 5)];
    assert_eq!(cell.symbol(), "─");
    assert_eq!(cell.style().bg, Some(Color::Rgb(26, 27, 38)));
}

#[test]
fn test_extreme_zoom_and_pan_with_diagonal_links() {
    let mut canvas = CanvasState::new();
    let theme = Theme::tokyo_night();
    let area = Rect::new(0, 0, 100, 50);

    canvas.links.push(clab_tui::canvas::link::CanvasLink {
        source_node: "n1".to_string(),
        source_port: "p1".to_string(),
        target_node: "n2".to_string(),
        target_port: "p2".to_string(),
        waypoints: vec![(10.0, 10.0), (30.0, 30.0), (60.0, 30.0)],
        routing_style: clab_tui::canvas::link::RoutingStyle::Octilinear,
    });

    // Test minimum zoom (50%) and rapid panning
    canvas.zoom = 0.5;
    canvas.offset_x = -200.0;
    canvas.offset_y = 150.0;
    let mut buf_min = Buffer::empty(area);
    KittyGraphRenderer::render_to_buffer(&canvas, area, &mut buf_min, &theme);
    let img_min = KittyGraphRenderer::render_image(&canvas, 800, 600, &theme);
    assert_eq!(img_min.width(), 800);
    assert_eq!(img_min.height(), 600);

    // Test maximum zoom (250%) and large positive offset
    canvas.zoom = 2.5;
    canvas.offset_x = 500.0;
    canvas.offset_y = -300.0;
    let mut buf_max = Buffer::empty(area);
    KittyGraphRenderer::render_to_buffer(&canvas, area, &mut buf_max, &theme);
    let img_max = KittyGraphRenderer::render_image(&canvas, 800, 600, &theme);
    assert_eq!(img_max.width(), 800);
    assert_eq!(img_max.height(), 600);
}
