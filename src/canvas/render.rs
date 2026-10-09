use crate::canvas::link::OrthogonalRouter;
use crate::canvas::state::{CanvasMode, CanvasState};
use crate::ui::theme::Theme;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};

pub struct CanvasRenderer;

impl CanvasRenderer {
    /// Render the entire canvas into the Ratatui buffer
    pub fn render(canvas: &CanvasState, area: Rect, buf: &mut Buffer, theme: &Theme) {
        if area.width < 4 || area.height < 4 {
            return;
        }

        // 1. Render subtle background grid dots
        Self::render_grid(canvas, area, buf, theme);

        // 2. Render links (orthogonal wires)
        Self::render_links(canvas, area, buf, theme);

        // 3. Render nodes
        Self::render_nodes(canvas, area, buf, theme);

        // 4. Render interface labels (pill badges) near link perimeters
        Self::render_link_labels(canvas, area, buf, theme);

        // 5. Render active wiring line if in wiring mode
        if canvas.mode == CanvasMode::Wiring {
            Self::render_wiring_preview(canvas, area, buf, theme);
        }

        // 6. Render marquee box selection if active
        if canvas.selection_box.is_some() {
            Self::render_marquee_box(canvas, area, buf, theme);
        }

        // 7. Render viewport status overlay (bottom-right: zoom & coordinates)
        Self::render_overlay(canvas, area, buf, theme);
    }

    fn render_grid(canvas: &CanvasState, area: Rect, buf: &mut Buffer, theme: &Theme) {
        let grid_spacing = 4.0;
        let grid_style = Style::default().fg(theme.grid_color);

        let min_x = 0;
        let max_x = area.width as i32;
        let min_y = 0;
        let max_y = area.height as i32;

        let start_canvas = canvas.screen_to_canvas(0, 0);
        let end_canvas = canvas.screen_to_canvas(area.width, area.height);

        let mut cy = (start_canvas.1 / grid_spacing).floor() * grid_spacing;
        while cy <= end_canvas.1 {
            let mut cx = (start_canvas.0 / grid_spacing).floor() * grid_spacing;
            while cx <= end_canvas.0 {
                let (sx, sy) = canvas.canvas_to_screen(cx, cy);
                if sx >= min_x && sx < max_x && sy >= min_y && sy < max_y {
                    let bx = area.x + sx as u16;
                    let by = area.y + sy as u16;
                    if bx < area.right() && by < area.bottom() {
                        buf[(bx, by)].set_char('·').set_style(grid_style);
                    }
                }
                cx += grid_spacing;
            }
            cy += grid_spacing;
        }
    }

    pub fn blend_chars(current: char, new_char: char) -> char {
        if current == ' ' || current == '·' || current == '▀' {
            return new_char;
        }
        if current == new_char {
            return new_char;
        }
        // Orthogonal crossing
        if (current == '─' && new_char == '│') || (current == '│' && new_char == '─') {
            return '┼';
        }
        // Diagonal crossing
        if (current == '╱' && new_char == '╲') || (current == '╲' && new_char == '╱') {
            return '╳';
        }
        if current == '╳' || new_char == '╳' {
            return '╳';
        }
        if current == '┼' || new_char == '┼' {
            return '┼';
        }
        // Intersection between orthogonal and diagonal
        if (current == '─' || current == '│') && (new_char == '╱' || new_char == '╲') {
            return '┼';
        }
        if (current == '╱' || current == '╲') && (new_char == '─' || new_char == '│') {
            return '┼';
        }
        // Preserve node borders/corners
        match current {
            '┐' | '┘' | '┌' | '└' | '╭' | '╮' | '╯' | '╰' => current,
            _ => new_char,
        }
    }

    fn draw_char_cell(buf: &mut Buffer, area: Rect, x: i32, y: i32, ch: char, style: Style) {
        if x < 0 || (x as u16) >= area.width || y < 0 || (y as u16) >= area.height {
            return;
        }
        let bx = area.x + x as u16;
        let by = area.y + y as u16;
        let cell = &mut buf[(bx, by)];
        let current_char = cell.symbol().chars().next().unwrap_or(' ');
        let blended = Self::blend_chars(current_char, ch);
        cell.set_char(blended).set_style(style);
    }

    fn draw_corner_char(buf: &mut Buffer, area: Rect, x: i32, y: i32, ch: char, style: Style) {
        if x < 0 || (x as u16) >= area.width || y < 0 || (y as u16) >= area.height {
            return;
        }
        let bx = area.x + x as u16;
        let by = area.y + y as u16;
        let cell = &mut buf[(bx, by)];
        cell.set_char(ch).set_style(style);
    }

    pub fn draw_line_segment(
        buf: &mut Buffer,
        area: Rect,
        sx1: i32,
        sy1: i32,
        sx2: i32,
        sy2: i32,
        style: Style,
    ) {
        if sy1 == sy2 {
            // Horizontal segment
            let min_x = sx1.min(sx2);
            let max_x = sx1.max(sx2);
            for x in min_x..=max_x {
                Self::draw_char_cell(buf, area, x, sy1, '─', style);
            }
        } else if sx1 == sx2 {
            // Vertical segment
            let min_y = sy1.min(sy2);
            let max_y = sy1.max(sy2);
            for y in min_y..=max_y {
                Self::draw_char_cell(buf, area, sx1, y, '│', style);
            }
        } else {
            // Diagonal / Oblique segment using Bresenham line algorithm
            let dx = (sx2 - sx1).abs();
            let dy = -(sy2 - sy1).abs();
            let step_x = if sx1 < sx2 { 1 } else { -1 };
            let step_y = if sy1 < sy2 { 1 } else { -1 };
            let mut err = dx + dy;
            let mut cx = sx1;
            let mut cy = sy1;

            let diag_ch = if (sx2 - sx1) * (sy2 - sy1) > 0 {
                '╲'
            } else {
                '╱'
            };

            let mut last_ch = diag_ch;

            loop {
                if cx == sx2 && cy == sy2 {
                    Self::draw_char_cell(buf, area, cx, cy, last_ch, style);
                    break;
                }
                let e2 = 2 * err;
                let step_in_x = e2 >= dy;
                let step_in_y = e2 <= dx;

                let ch = if step_in_x && step_in_y {
                    diag_ch
                } else if step_in_x {
                    '─'
                } else {
                    '│'
                };
                last_ch = ch;

                Self::draw_char_cell(buf, area, cx, cy, ch, style);

                if step_in_x {
                    err += dy;
                    cx += step_x;
                }
                if step_in_y {
                    err += dx;
                    cy += step_y;
                }
            }
        }
    }

    fn render_links(canvas: &CanvasState, area: Rect, buf: &mut Buffer, theme: &Theme) {
        for (i, link) in canvas.links.iter().enumerate() {
            let is_selected = canvas.selected_link == Some(i);
            let style = if is_selected {
                Style::default()
                    .fg(theme.link_selected)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(theme.link_color)
            };

            // Render each straight/diagonal segment
            for win in link.waypoints.windows(2) {
                let (p1, p2) = (win[0], win[1]);
                let (sx1, sy1) = canvas.canvas_to_screen(p1.0, p1.1);
                let (sx2, sy2) = canvas.canvas_to_screen(p2.0, p2.1);
                Self::draw_line_segment(buf, area, sx1, sy1, sx2, sy2, style);
            }

            // Render corner glyphs at waypoints
            if link.waypoints.len() >= 3 {
                for j in 1..link.waypoints.len() - 1 {
                    let prev = link.waypoints[j - 1];
                    let curr = link.waypoints[j];
                    let next = link.waypoints[j + 1];

                    let (sx, sy) = canvas.canvas_to_screen(curr.0, curr.1);
                    let corner_ch = OrthogonalRouter::get_glyph_at(Some(prev), curr, Some(next));
                    Self::draw_corner_char(buf, area, sx, sy, corner_ch, style);
                }
            }
        }
    }

    pub fn render_link_labels(canvas: &CanvasState, area: Rect, buf: &mut Buffer, theme: &Theme) {
        for (i, link) in canvas.links.iter().enumerate() {
            if link.waypoints.len() < 2 {
                continue;
            }

            let is_selected = canvas.selected_link == Some(i);
            let pill_style = if is_selected {
                Style::default()
                    .fg(theme.link_selected)
                    .bg(theme.panel_bg)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default()
                    .fg(Color::Rgb(115, 218, 202))
                    .bg(Color::Rgb(26, 27, 38))
                    .add_modifier(Modifier::BOLD)
            };

            let Some((src_badge, tgt_badge)) = canvas.link_badge_positions(i) else {
                continue;
            };

            // Source interface pill badge
            let badge_src = format!("[{}]", link.source_port);
            if src_badge.y >= 0 && (src_badge.y as u16) < area.height {
                for (c_idx, ch) in badge_src.chars().enumerate() {
                    let cell_x = src_badge.x + c_idx as i32;
                    if cell_x >= 0 && (cell_x as u16) < area.width {
                        let px = area.x + cell_x as u16;
                        let py = area.y + src_badge.y as u16;
                        if px < area.right()
                            && py < area.bottom()
                            && px < buf.area.right()
                            && py < buf.area.bottom()
                        {
                            buf[(px, py)].set_char(ch).set_style(pill_style);
                        }
                    }
                }
            }

            // Target interface pill badge
            let badge_tgt = format!("[{}]", link.target_port);
            if tgt_badge.y >= 0 && (tgt_badge.y as u16) < area.height {
                for (c_idx, ch) in badge_tgt.chars().enumerate() {
                    let cell_x = tgt_badge.x + c_idx as i32;
                    if cell_x >= 0 && (cell_x as u16) < area.width {
                        let px = area.x + cell_x as u16;
                        let py = area.y + tgt_badge.y as u16;
                        if px < area.right()
                            && py < area.bottom()
                            && px < buf.area.right()
                            && py < buf.area.bottom()
                        {
                            buf[(px, py)].set_char(ch).set_style(pill_style);
                        }
                    }
                }
            }
        }
    }

    fn render_nodes(canvas: &CanvasState, area: Rect, buf: &mut Buffer, theme: &Theme) {
        for (name, node) in &canvas.nodes {
            let is_selected = canvas.selected_nodes.contains(name)
                || canvas.selected_node.as_deref() == Some(name.as_str());

            let (sx, sy) = canvas.canvas_to_screen(node.x, node.y);
            let w = (node.width * canvas.zoom).round() as i32;
            let h = (node.height * canvas.zoom).round() as i32;

            if sx + w < 0 || sx >= area.width as i32 || sy + h < 0 || sy >= area.height as i32 {
                continue;
            }

            let box_style = if is_selected {
                Style::default()
                    .fg(theme.node_selected)
                    .bg(theme.node_selected_bg)
                    .add_modifier(Modifier::BOLD)
            } else {
                let fg_color = match node.category {
                    crate::clab::model::NodeCategory::Router => theme.node_router,
                    crate::clab::model::NodeCategory::Switch => theme.node_switch,
                    crate::clab::model::NodeCategory::Host => theme.node_host,
                    crate::clab::model::NodeCategory::Firewall => theme.node_firewall,
                };
                Style::default().fg(fg_color).bg(theme.node_bg)
            };

            // Draw box background and borders
            for row in 0..h {
                let cy = sy + row;
                if cy < 0 || cy >= area.height as i32 {
                    continue;
                }
                let by = area.y + cy as u16;

                for col in 0..w {
                    let cx = sx + col;
                    if cx < 0 || cx >= area.width as i32 {
                        continue;
                    }
                    let bx = area.x + cx as u16;

                    let ch = if row == 0 && col == 0 {
                        '╭'
                    } else if row == 0 && col == w - 1 {
                        '╮'
                    } else if row == h - 1 && col == 0 {
                        '╰'
                    } else if row == h - 1 && col == w - 1 {
                        '╯'
                    } else if row == 0 || row == h - 1 {
                        '─'
                    } else if col == 0 || col == w - 1 {
                        '│'
                    } else {
                        ' '
                    };

                    buf[(bx, by)].set_char(ch).set_style(box_style);
                }
            }

            // Draw header inside node: Icon + Name
            let header_y = sy + 1;
            if header_y >= 0 && header_y < area.height as i32 {
                let by = area.y + header_y as u16;
                let title = format!("{} {}", node.icon(), name);
                let title_style = box_style.add_modifier(Modifier::BOLD);
                let start_x = sx + 2;

                for (idx, ch) in title.chars().enumerate() {
                    let cx = start_x + idx as i32;
                    if cx > sx && cx < sx + w - 1 && cx >= 0 && cx < area.width as i32 {
                        let bx = area.x + cx as u16;
                        buf[(bx, by)].set_char(ch).set_style(title_style);
                    }
                }
            }

            // Draw Kind / Image subtitle inside node
            let sub_y = sy + 2;
            if sub_y >= 0 && sub_y < area.height as i32 && h >= 4 {
                let by = area.y + sub_y as u16;
                let sub = format!("[{}]", node.kind);
                let sub_style = Style::default()
                    .fg(theme.text_muted)
                    .bg(box_style.bg.unwrap_or(Color::Reset));
                let start_x = sx + 2;

                for (idx, ch) in sub.chars().enumerate() {
                    let cx = start_x + idx as i32;
                    if cx > sx && cx < sx + w - 1 && cx >= 0 && cx < area.width as i32 {
                        let bx = area.x + cx as u16;
                        buf[(bx, by)].set_char(ch).set_style(sub_style);
                    }
                }
            }
        }
    }

    pub fn render_wiring_preview(
        canvas: &CanvasState,
        area: Rect,
        buf: &mut Buffer,
        theme: &Theme,
    ) {
        if let Some((src_name, src_port)) = &canvas.wiring_source {
            if let Some(node) = canvas.nodes.get(src_name) {
                let ((px, py), _) = if let Some(port) = node.get_port(src_port) {
                    ((port.x, port.y), port.direction)
                } else {
                    node.perimeter_connection_towards(
                        canvas.wiring_cursor.0,
                        canvas.wiring_cursor.1,
                    )
                };
                let (sx1, sy1) = canvas.canvas_to_screen(px, py);
                let (sx2, sy2) =
                    canvas.canvas_to_screen(canvas.wiring_cursor.0, canvas.wiring_cursor.1);

                let wiring_style = Style::default()
                    .fg(theme.wiring_active)
                    .add_modifier(Modifier::RAPID_BLINK | Modifier::BOLD);

                match canvas.routing_style {
                    crate::canvas::link::RoutingStyle::Direct => {
                        // Direct diagonal rubber-band line straight to cursor
                        Self::draw_line_segment(buf, area, sx1, sy1, sx2, sy2, wiring_style);
                    }
                    crate::canvas::link::RoutingStyle::Octilinear => {
                        // 45-degree octilinear rubber-band line
                        let dx = sx2 - sx1;
                        let dy = sy2 - sy1;
                        if dx == 0 || dy == 0 || dx.abs() == dy.abs() {
                            Self::draw_line_segment(buf, area, sx1, sy1, sx2, sy2, wiring_style);
                        } else if dx.abs() > dy.abs() {
                            let mid_x = sx1 + dy.abs() * dx.signum();
                            Self::draw_line_segment(buf, area, sx1, sy1, mid_x, sy2, wiring_style);
                            Self::draw_line_segment(buf, area, mid_x, sy2, sx2, sy2, wiring_style);
                            let corner_ch = OrthogonalRouter::get_glyph_at(
                                Some((sx1 as f64, sy1 as f64)),
                                (mid_x as f64, sy2 as f64),
                                Some((sx2 as f64, sy2 as f64)),
                            );
                            Self::draw_corner_char(buf, area, mid_x, sy2, corner_ch, wiring_style);
                        } else {
                            let mid_y = sy1 + dx.abs() * dy.signum();
                            Self::draw_line_segment(buf, area, sx1, sy1, sx2, mid_y, wiring_style);
                            Self::draw_line_segment(buf, area, sx2, mid_y, sx2, sy2, wiring_style);
                            let corner_ch = OrthogonalRouter::get_glyph_at(
                                Some((sx1 as f64, sy1 as f64)),
                                (sx2 as f64, mid_y as f64),
                                Some((sx2 as f64, sy2 as f64)),
                            );
                            Self::draw_corner_char(buf, area, sx2, mid_y, corner_ch, wiring_style);
                        }
                    }
                    crate::canvas::link::RoutingStyle::Orthogonal => {
                        // Clean orthogonal straight rubber-band lines:
                        // Horizontal from sx1 to sx2, corner at (sx2, sy1), then vertical from sy1 to sy2
                        let min_x = sx1.min(sx2);
                        let max_x = sx1.max(sx2);
                        for x in min_x..=max_x {
                            Self::draw_char_cell(buf, area, x, sy1, '─', wiring_style);
                        }

                        let min_y = sy1.min(sy2);
                        let max_y = sy1.max(sy2);
                        for y in min_y..=max_y {
                            Self::draw_char_cell(buf, area, sx2, y, '│', wiring_style);
                        }

                        if sx1 != sx2 && sy1 != sy2 {
                            let corner_ch = OrthogonalRouter::get_glyph_at(
                                Some((px, py)),
                                (canvas.wiring_cursor.0, py),
                                Some((canvas.wiring_cursor.0, canvas.wiring_cursor.1)),
                            );
                            Self::draw_corner_char(buf, area, sx2, sy1, corner_ch, wiring_style);
                        }
                    }
                }

                // Target cursor endpoint marker
                Self::draw_char_cell(buf, area, sx2, sy2, '●', wiring_style);
            }
        }
    }

    pub fn render_marquee_box(canvas: &CanvasState, area: Rect, buf: &mut Buffer, theme: &Theme) {
        let Some((p1, p2)) = canvas.selection_box else {
            return;
        };

        let (sx1, sy1) = canvas.canvas_to_screen(p1.0, p1.1);
        let (sx2, sy2) = canvas.canvas_to_screen(p2.0, p2.1);

        let min_x = sx1.min(sx2);
        let max_x = sx1.max(sx2);
        let min_y = sy1.min(sy2);
        let max_y = sy1.max(sy2);

        let box_style = Style::default().fg(theme.accent);

        for x in min_x..=max_x {
            if min_y >= 0 && (min_y as u16) < area.height {
                let ch = if x == min_x {
                    '┌'
                } else if x == max_x {
                    '┐'
                } else {
                    '┄'
                };
                Self::draw_corner_char(buf, area, x, min_y, ch, box_style);
            }
            if max_y >= 0 && (max_y as u16) < area.height && max_y != min_y {
                let ch = if x == min_x {
                    '└'
                } else if x == max_x {
                    '┘'
                } else {
                    '┄'
                };
                Self::draw_corner_char(buf, area, x, max_y, ch, box_style);
            }
        }

        for y in (min_y + 1)..max_y {
            if y >= 0 && (y as u16) < area.height {
                Self::draw_corner_char(buf, area, min_x, y, '┆', box_style);
                if max_x != min_x {
                    Self::draw_corner_char(buf, area, max_x, y, '┆', box_style);
                }
            }
        }
    }

    pub fn render_overlay(canvas: &CanvasState, area: Rect, buf: &mut Buffer, theme: &Theme) {
        let zoom_pct = (canvas.zoom * 100.0).round() as u32;
        let mode_str = match canvas.mode {
            CanvasMode::Normal => "MODE: NORMAL",
            CanvasMode::Wiring => "MODE: WIRING (Click / Select target port, Esc cancel)",
            CanvasMode::DraggingNode => "MODE: MOVE",
            CanvasMode::PanningCanvas => "MODE: PANNING",
            CanvasMode::InspectorDrawer => "MODE: INSPECTOR",
            CanvasMode::BoxSelection => "MODE: SELECT",
        };

        let selected_info = if canvas.selected_nodes.len() > 1 {
            format!("Selected: {} nodes", canvas.selected_nodes.len())
        } else if let Some(n) = &canvas.selected_node {
            format!("Node: {}", n)
        } else if let Some(l) = canvas.selected_link {
            format!("Link: #{}", l + 1)
        } else {
            "No selection".to_string()
        };

        let style_str = match canvas.routing_style {
            crate::canvas::link::RoutingStyle::Orthogonal => "Orthogonal",
            crate::canvas::link::RoutingStyle::Direct => "Direct (Diag)",
            crate::canvas::link::RoutingStyle::Octilinear => "Octilinear (45°)",
        };

        let status = format!(
            "{} | {} | Route: {} [r] | Zoom: {}%",
            mode_str, selected_info, style_str, zoom_pct
        );
        let status_style = Style::default().fg(theme.text_secondary).bg(theme.panel_bg);

        let y = area.bottom().saturating_sub(1);
        let start_x = area.x + 2;

        for (i, ch) in status.chars().enumerate() {
            let x = start_x + i as u16;
            if x < area.right() {
                buf[(x, y)].set_char(ch).set_style(status_style);
            }
        }
    }
}
