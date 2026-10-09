use crate::clab::model::ContainerInspectInfo;
use crate::ui::theme::Theme;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};

#[derive(Debug, Clone, Default)]
pub struct InspectViewState {
    pub containers: Vec<ContainerInspectInfo>,
    pub selected_index: usize,
    pub is_loading: bool,
    pub filter: String,
}

impl InspectViewState {
    pub fn select_next(&mut self) {
        if !self.containers.is_empty() {
            self.selected_index = (self.selected_index + 1) % self.containers.len();
        }
    }

    pub fn select_prev(&mut self) {
        if !self.containers.is_empty() {
            if self.selected_index == 0 {
                self.selected_index = self.containers.len() - 1;
            } else {
                self.selected_index -= 1;
            }
        }
    }

    pub fn selected_container(&self) -> Option<&ContainerInspectInfo> {
        self.containers.get(self.selected_index)
    }
}

pub struct InspectView;

impl InspectView {
    pub fn render(state: &InspectViewState, area: Rect, buf: &mut Buffer, theme: &Theme) {
        if area.width < 10 || area.height < 4 {
            return;
        }

        let bg_style = Style::default().bg(theme.panel_bg);
        for y in area.y..area.bottom() {
            for x in area.x..area.right() {
                buf[(x, y)].set_char(' ').set_style(bg_style);
            }
        }

        // Header bar
        let loading_tag = if state.is_loading {
            " [Refreshing...]"
        } else {
            ""
        };
        let header = format!(
            " Active Lab Containers ({}){} [r Refresh]",
            state.containers.len(),
            loading_tag
        );
        for (i, ch) in header.chars().enumerate() {
            let px = area.x + 1 + i as u16;
            if px < area.right() {
                buf[(px, area.y)].set_char(ch).set_style(
                    Style::default()
                        .fg(theme.accent)
                        .bg(theme.panel_bg)
                        .add_modifier(Modifier::BOLD),
                );
            }
        }

        // Table column headers
        let col_y = area.y + 2;
        let col_header = format!(
            "  {:<26} {:<10} {:<10} {:<20} {:<16} {:<12}",
            "CONTAINER NAME", "KIND", "STATE", "IMAGE", "IPV4 ADDR", "CONTAINER ID"
        );
        for (i, ch) in col_header.chars().take(area.width as usize).enumerate() {
            let px = area.x + i as u16;
            buf[(px, col_y)].set_char(ch).set_style(
                Style::default()
                    .fg(theme.text_secondary)
                    .bg(theme.panel_bg)
                    .add_modifier(Modifier::BOLD | Modifier::UNDERLINED),
            );
        }

        let divider_y = area.y + 3;
        for x in area.x..area.right() {
            buf[(x, divider_y)]
                .set_char('─')
                .set_style(Style::default().fg(theme.border).bg(theme.panel_bg));
        }

        // Empty state check
        if state.containers.is_empty() {
            let empty_msg = "No running Containerlab containers found. Press 'd' to deploy topology or 'r' to refresh.";
            let empty_style = Style::default().fg(theme.text_muted).bg(theme.panel_bg);
            let msg_y = area.y + 5;
            for (i, ch) in empty_msg.chars().enumerate() {
                let px = area.x + 4 + i as u16;
                if px < area.right() {
                    buf[(px, msg_y)].set_char(ch).set_style(empty_style);
                }
            }
            return;
        }

        // Rows
        let table_start_y = area.y + 4;
        let max_rows = (area.height.saturating_sub(6)) as usize;

        for (idx, container) in state.containers.iter().take(max_rows).enumerate() {
            let row_y = table_start_y + idx as u16;
            if row_y >= area.bottom() - 2 {
                break;
            }

            let is_sel = idx == state.selected_index;
            let status_icon = if container.state.to_lowercase().contains("running") {
                "●"
            } else {
                "○"
            };

            let row_style = if is_sel {
                Style::default()
                    .fg(theme.accent)
                    .bg(theme.bg)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(theme.text_primary).bg(theme.panel_bg)
            };

            let prefix = if is_sel { "▶ " } else { "  " };
            let short_id = container
                .container_id
                .as_deref()
                .map(|id| if id.len() > 10 { &id[..10] } else { id })
                .unwrap_or("-");

            let short_image = if container.image.len() > 18 {
                &container.image[..18]
            } else {
                &container.image
            };

            let ipv4 = container.ipv4_address.as_deref().unwrap_or("-");

            let line = format!(
                "{}{:<24} {:<10} {} {:<8} {:<20} {:<16} {:<12}",
                prefix,
                container.name,
                container.kind,
                status_icon,
                container.state,
                short_image,
                ipv4,
                short_id
            );

            for (i, ch) in line.chars().take(area.width as usize).enumerate() {
                let px = area.x + i as u16;
                buf[(px, row_y)].set_char(ch).set_style(row_style);
            }
        }

        // Bottom detail bar for selected container
        if let Some(sel) = state.selected_container() {
            let detail_y = area.bottom() - 2;
            let detail_line = format!(
                "Selected: {} | Lab: {} | IPv4: {} | IPv6: {}",
                sel.name,
                sel.lab_name,
                sel.ipv4_address.as_deref().unwrap_or("none"),
                sel.ipv6_address.as_deref().unwrap_or("none")
            );
            let detail_style = Style::default().fg(theme.text_secondary).bg(theme.panel_bg);
            for (i, ch) in detail_line.chars().take(area.width as usize).enumerate() {
                let px = area.x + 2 + i as u16;
                if px < area.right() {
                    buf[(px, detail_y)].set_char(ch).set_style(detail_style);
                }
            }
        }
    }
}
