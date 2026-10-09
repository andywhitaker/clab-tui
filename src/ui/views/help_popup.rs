use crate::ui::theme::Theme;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};

pub struct HelpPopup;

impl HelpPopup {
    pub fn render(area: Rect, buf: &mut Buffer, theme: &Theme) {
        let width = 76.min(area.width.saturating_sub(4));
        let height = 34.min(area.height.saturating_sub(2));

        let x = area.x + (area.width.saturating_sub(width)) / 2;
        let y = area.y + (area.height.saturating_sub(height)) / 2;
        let popup_rect = Rect::new(x, y, width, height);

        let border_style = Style::default().fg(theme.accent).bg(theme.panel_bg);

        // Fill background & draw border
        for py in popup_rect.y..popup_rect.bottom() {
            for px in popup_rect.x..popup_rect.right() {
                let ch = if py == popup_rect.y && px == popup_rect.x {
                    '╭'
                } else if py == popup_rect.y && px == popup_rect.right() - 1 {
                    '╮'
                } else if py == popup_rect.bottom() - 1 && px == popup_rect.x {
                    '╰'
                } else if py == popup_rect.bottom() - 1 && px == popup_rect.right() - 1 {
                    '╯'
                } else if py == popup_rect.y || py == popup_rect.bottom() - 1 {
                    '─'
                } else if px == popup_rect.x || px == popup_rect.right() - 1 {
                    '│'
                } else {
                    ' '
                };

                buf[(px, py)].set_char(ch).set_style(border_style);
            }
        }

        // Title
        let title = " clab-tui Keyboard & Mouse Reference ";
        let title_style = Style::default()
            .fg(theme.accent)
            .bg(theme.panel_bg)
            .add_modifier(Modifier::BOLD);
        for (i, ch) in title.chars().enumerate() {
            let px = popup_rect.x + 3 + i as u16;
            if px < popup_rect.right() - 1 {
                buf[(px, popup_rect.y)].set_char(ch).set_style(title_style);
            }
        }

        let help_sections = [
            (
                "NAVIGATION & VIEWS",
                vec![
                    (
                        "1, 2, 3, 4, 5",
                        "Switch view: [1] Canvas, [2] Inspector, [3] Logs, [4] YAML, [5] CLI",
                    ),
                    ("Tab / BackTab", "Cycle focus between panels and nodes"),
                    ("q / Ctrl-C", "Quit clab-tui"),
                    ("? / F1", "Toggle this help popup"),
                ],
            ),
            (
                "CANVAS VISUAL DESIGNER",
                vec![
                    ("h, j, k, l / Arrows", "Move selected node on canvas grid"),
                    (
                        "Shift/Ctrl+Arrows / H,J,K,L",
                        "Pan canvas viewport (or drag empty canvas)",
                    ),
                    (
                        "v / b / s / Space",
                        "Toggle Visual / Box Select mode (drag to select)",
                    ),
                    (
                        "w",
                        "Enter port-to-port wiring mode (select source -> target)",
                    ),
                    (
                        "a",
                        "Add new node (Nokia SR Linux, Arista cEOS, Cisco, Linux, etc.)",
                    ),
                    (
                        "e",
                        "Edit selected node in Drawer or selected link in Modal",
                    ),
                    ("[, ]", "Cycle selected link previous / next"),
                    ("c", "Clone / duplicate selected node"),
                    ("x / Delete", "Delete selected node or selected wire"),
                    ("+ / - / 0", "Zoom in / Zoom out / Reset zoom (100%)"),
                    (
                        "r / R",
                        "Cycle link routing (Orthogonal, Direct/Diag, Octilinear)",
                    ),
                    (
                        "a -> n, e, d",
                        "Manage dynamic node profiles (Add, Edit, Delete)",
                    ),
                    (
                        "e / Ctrl-S in YAML",
                        "Interactive YAML text editor: type, edit, and apply",
                    ),
                ],
            ),
            (
                "CONTAINERLAB OPERATIONS",
                vec![
                    (
                        "d",
                        "Deploy topology (with -c cleanup / --reconfigure options)",
                    ),
                    ("D", "Destroy / Teardown lab (with --cleanup option)"),
                    (
                        "5 -> r / Enter",
                        "Interactive CLI Tree: configure flags & execute live",
                    ),
                    ("r / i", "Refresh container status / Reload inspect"),
                    ("Ctrl-S", "Save canvas topology directly to *.clab.yml file"),
                ],
            ),
            (
                "MOUSE SUPPORT",
                vec![
                    (
                        "Left Click",
                        "Select node or wire / Click port to start or finish link",
                    ),
                    ("Left Drag", "Drag node or drag empty canvas to pan"),
                    (
                        "Shift/Alt/Ctrl+Drag",
                        "Marquee box select multiple nodes (or use v/b/s mode)",
                    ),
                    ("Scroll Wheel", "Pan canvas vertically or zoom"),
                ],
            ),
        ];

        let mut row_y = popup_rect.y + 2;

        for (section_title, items) in help_sections {
            if row_y >= popup_rect.bottom() - 2 {
                break;
            }

            Self::draw_line(
                popup_rect.x + 2,
                row_y,
                section_title,
                Style::default()
                    .fg(theme.accent_alt)
                    .bg(theme.panel_bg)
                    .add_modifier(Modifier::BOLD | Modifier::UNDERLINED),
                buf,
                popup_rect.right(),
            );
            row_y += 1;

            for (keys, desc) in items {
                if row_y >= popup_rect.bottom() - 2 {
                    break;
                }

                let line = format!("  {:<22} : {}", keys, desc);
                Self::draw_line(
                    popup_rect.x + 2,
                    row_y,
                    &line,
                    Style::default().fg(theme.text_primary).bg(theme.panel_bg),
                    buf,
                    popup_rect.right(),
                );
                row_y += 1;
            }

            row_y += 1;
        }

        // Close instruction footer
        let footer_y = popup_rect.bottom() - 2;
        let footer = "Press Esc or ? to return to application";
        Self::draw_line(
            popup_rect.x + 3,
            footer_y,
            footer,
            Style::default().fg(theme.text_muted).bg(theme.panel_bg),
            buf,
            popup_rect.right(),
        );
    }

    fn draw_line(x: u16, y: u16, text: &str, style: Style, buf: &mut Buffer, right_bound: u16) {
        for (i, ch) in text.chars().enumerate() {
            let px = x + i as u16;
            if px < right_bound - 1 {
                buf[(px, y)].set_char(ch).set_style(style);
            }
        }
    }
}
