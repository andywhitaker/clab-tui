use ratatui::style::Color;

#[derive(Debug, Clone)]
pub struct Theme {
    pub bg: Color,
    pub fg: Color,
    pub panel_bg: Color,
    pub border: Color,
    pub border_focused: Color,
    pub accent: Color,
    pub accent_alt: Color,
    pub success: Color,
    pub warning: Color,
    pub error: Color,
    pub info: Color,

    pub text_primary: Color,
    pub text_secondary: Color,
    pub text_muted: Color,

    pub grid_color: Color,
    pub link_color: Color,
    pub link_selected: Color,
    pub wiring_active: Color,

    pub node_bg: Color,
    pub node_selected_bg: Color,
    pub node_selected: Color,
    pub node_router: Color,
    pub node_switch: Color,
    pub node_host: Color,
    pub node_firewall: Color,
}

impl Default for Theme {
    fn default() -> Self {
        Self::tokyo_night()
    }
}

impl Theme {
    pub fn tokyo_night() -> Self {
        Self {
            bg: Color::Rgb(26, 27, 38),                // #1a1b26
            fg: Color::Rgb(192, 202, 245),             // #c0caf5
            panel_bg: Color::Rgb(36, 40, 59),          // #24283b
            border: Color::Rgb(65, 72, 104),           // #414868
            border_focused: Color::Rgb(122, 162, 247), // #7aa2f7
            accent: Color::Rgb(122, 162, 247),         // #7aa2f7
            accent_alt: Color::Rgb(187, 154, 247),     // #bb9af7
            success: Color::Rgb(158, 206, 106),        // #9ece6a
            warning: Color::Rgb(224, 175, 104),        // #e0af68
            error: Color::Rgb(247, 118, 142),          // #f7768e
            info: Color::Rgb(125, 207, 255),           // #7dcfff

            text_primary: Color::Rgb(192, 202, 245),
            text_secondary: Color::Rgb(169, 177, 214),
            text_muted: Color::Rgb(86, 95, 137),

            grid_color: Color::Rgb(50, 54, 80),
            link_color: Color::Rgb(122, 162, 247),
            link_selected: Color::Rgb(224, 175, 104),
            wiring_active: Color::Rgb(158, 206, 106),

            node_bg: Color::Rgb(36, 40, 59),
            node_selected_bg: Color::Rgb(46, 52, 78),
            node_selected: Color::Rgb(224, 175, 104),
            node_router: Color::Rgb(125, 207, 255),
            node_switch: Color::Rgb(115, 218, 202),
            node_host: Color::Rgb(187, 154, 247),
            node_firewall: Color::Rgb(247, 118, 142),
        }
    }
}
