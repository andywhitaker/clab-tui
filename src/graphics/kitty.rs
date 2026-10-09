use crate::canvas::link::OrthogonalRouter;
use crate::canvas::render::CanvasRenderer;
use crate::canvas::state::{CanvasMode, CanvasState};
use crate::ui::theme::Theme;
use base64::engine::general_purpose::STANDARD as BASE64;
use base64::Engine;
use image::{ImageBuffer, Rgba};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::widgets::Widget;
use ratatui_image::picker::{Picker, ProtocolType};
use ratatui_image::{Image, Resize};
use std::env;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GraphicsProtocol {
    Kitty,
    Sixel,
    BrailleUnicodeFallback,
}

pub struct GraphicsDetector;

impl GraphicsDetector {
    /// Detect if the terminal supports the Kitty graphics protocol
    pub fn detect_kitty_support() -> bool {
        // 1. Direct Kitty window ID check
        if env::var("KITTY_WINDOW_ID").is_ok() {
            return true;
        }

        // 2. Terminal program check (Kitty, Ghostty, WezTerm support Kitty protocol)
        if let Ok(term_program) = env::var("TERM_PROGRAM") {
            let lower = term_program.to_lowercase();
            if lower.contains("kitty") || lower.contains("ghostty") || lower.contains("wezterm") {
                return true;
            }
        }

        // 3. TERM check
        if let Ok(term) = env::var("TERM") {
            let lower = term.to_lowercase();
            if lower.contains("kitty") || lower.contains("ghostty") || lower.contains("wezterm") {
                return true;
            }
        }

        // 4. Ghostty environment check
        if env::var("GHOSTTY_RESOURCES_DIR").is_ok() {
            return true;
        }

        false
    }

    /// Determine active protocol (defaults to Kitty if supported, else Braille/Unicode fallback)
    pub fn get_preferred_protocol() -> GraphicsProtocol {
        if Self::detect_kitty_support() {
            GraphicsProtocol::Kitty
        } else {
            GraphicsProtocol::BrailleUnicodeFallback
        }
    }
}

// ----------------------------------------------------------------------------
// 5x7 Crisp Bitmap Font for High-Resolution Raster Rendering (ASCII 32..=126)
// ----------------------------------------------------------------------------
const FONT_5X7: [[u8; 7]; 95] = [
    // 32 ' '
    [0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00],
    // 33 '!'
    [0x04, 0x04, 0x04, 0x04, 0x04, 0x00, 0x04],
    // 34 '"'
    [0x0A, 0x0A, 0x0A, 0x00, 0x00, 0x00, 0x00],
    // 35 '#'
    [0x0A, 0x0A, 0x1F, 0x0A, 0x1F, 0x0A, 0x0A],
    // 36 '$'
    [0x04, 0x0F, 0x14, 0x0E, 0x05, 0x1E, 0x04],
    // 37 '%'
    [0x19, 0x19, 0x02, 0x04, 0x08, 0x13, 0x13],
    // 38 '&'
    [0x0C, 0x12, 0x14, 0x08, 0x15, 0x12, 0x0D],
    // 39 '\''
    [0x04, 0x04, 0x02, 0x00, 0x00, 0x00, 0x00],
    // 40 '('
    [0x02, 0x04, 0x08, 0x08, 0x08, 0x04, 0x02],
    // 41 ')'
    [0x08, 0x04, 0x02, 0x02, 0x02, 0x04, 0x08],
    // 42 '*'
    [0x00, 0x04, 0x15, 0x0E, 0x15, 0x04, 0x00],
    // 43 '+'
    [0x00, 0x04, 0x04, 0x1F, 0x04, 0x04, 0x00],
    // 44 ','
    [0x00, 0x00, 0x00, 0x00, 0x04, 0x04, 0x08],
    // 45 '-'
    [0x00, 0x00, 0x00, 0x1F, 0x00, 0x00, 0x00],
    // 46 '.'
    [0x00, 0x00, 0x00, 0x00, 0x00, 0x0C, 0x0C],
    // 47 '/'
    [0x01, 0x02, 0x04, 0x08, 0x10, 0x00, 0x00],
    // 48 '0'
    [0x0E, 0x11, 0x13, 0x15, 0x19, 0x11, 0x0E],
    // 49 '1'
    [0x04, 0x0C, 0x04, 0x04, 0x04, 0x04, 0x0E],
    // 50 '2'
    [0x0E, 0x11, 0x01, 0x02, 0x04, 0x08, 0x1F],
    // 51 '3'
    [0x1F, 0x02, 0x04, 0x02, 0x01, 0x11, 0x0E],
    // 52 '4'
    [0x02, 0x06, 0x0A, 0x12, 0x1F, 0x02, 0x02],
    // 53 '5'
    [0x1F, 0x10, 0x1E, 0x01, 0x01, 0x11, 0x0E],
    // 54 '6'
    [0x06, 0x08, 0x10, 0x1E, 0x11, 0x11, 0x0E],
    // 55 '7'
    [0x1F, 0x01, 0x02, 0x04, 0x08, 0x08, 0x08],
    // 56 '8'
    [0x0E, 0x11, 0x11, 0x0E, 0x11, 0x11, 0x0E],
    // 57 '9'
    [0x0E, 0x11, 0x11, 0x0F, 0x01, 0x02, 0x0C],
    // 58 ':'
    [0x00, 0x0C, 0x0C, 0x00, 0x0C, 0x0C, 0x00],
    // 59 ';'
    [0x00, 0x0C, 0x0C, 0x00, 0x0C, 0x04, 0x08],
    // 60 '<'
    [0x02, 0x04, 0x08, 0x10, 0x08, 0x04, 0x02],
    // 61 '='
    [0x00, 0x1F, 0x00, 0x1F, 0x00, 0x00, 0x00],
    // 62 '>'
    [0x08, 0x04, 0x02, 0x01, 0x02, 0x04, 0x08],
    // 63 '?'
    [0x0E, 0x11, 0x01, 0x02, 0x04, 0x00, 0x04],
    // 64 '@'
    [0x0E, 0x11, 0x01, 0x0D, 0x15, 0x15, 0x0E],
    // 65 'A'
    [0x0E, 0x11, 0x11, 0x1F, 0x11, 0x11, 0x11],
    // 66 'B'
    [0x1E, 0x11, 0x11, 0x1E, 0x11, 0x11, 0x1E],
    // 67 'C'
    [0x0E, 0x11, 0x10, 0x10, 0x10, 0x11, 0x0E],
    // 68 'D'
    [0x1C, 0x12, 0x11, 0x11, 0x11, 0x12, 0x1C],
    // 69 'E'
    [0x1F, 0x10, 0x10, 0x1E, 0x10, 0x10, 0x1F],
    // 70 'F'
    [0x1F, 0x10, 0x10, 0x1E, 0x10, 0x10, 0x10],
    // 71 'G'
    [0x0E, 0x11, 0x10, 0x17, 0x11, 0x11, 0x0F],
    // 72 'H'
    [0x11, 0x11, 0x11, 0x1F, 0x11, 0x11, 0x11],
    // 73 'I'
    [0x0E, 0x04, 0x04, 0x04, 0x04, 0x04, 0x0E],
    // 74 'J'
    [0x07, 0x02, 0x02, 0x02, 0x02, 0x12, 0x0C],
    // 75 'K'
    [0x11, 0x12, 0x14, 0x18, 0x14, 0x12, 0x11],
    // 76 'L'
    [0x10, 0x10, 0x10, 0x10, 0x10, 0x10, 0x1F],
    // 77 'M'
    [0x11, 0x1B, 0x15, 0x15, 0x11, 0x11, 0x11],
    // 78 'N'
    [0x11, 0x11, 0x19, 0x15, 0x13, 0x11, 0x11],
    // 79 'O'
    [0x0E, 0x11, 0x11, 0x11, 0x11, 0x11, 0x0E],
    // 80 'P'
    [0x1E, 0x11, 0x11, 0x1E, 0x10, 0x10, 0x10],
    // 81 'Q'
    [0x0E, 0x11, 0x11, 0x11, 0x15, 0x12, 0x0D],
    // 82 'R'
    [0x1E, 0x11, 0x11, 0x1E, 0x14, 0x12, 0x11],
    // 83 'S'
    [0x0F, 0x10, 0x10, 0x0E, 0x01, 0x01, 0x1E],
    // 84 'T'
    [0x1F, 0x04, 0x04, 0x04, 0x04, 0x04, 0x04],
    // 85 'U'
    [0x11, 0x11, 0x11, 0x11, 0x11, 0x11, 0x0E],
    // 86 'V'
    [0x11, 0x11, 0x11, 0x11, 0x11, 0x0A, 0x04],
    // 87 'W'
    [0x11, 0x11, 0x11, 0x15, 0x15, 0x15, 0x0A],
    // 88 'X'
    [0x11, 0x11, 0x0A, 0x04, 0x0A, 0x11, 0x11],
    // 89 'Y'
    [0x11, 0x11, 0x11, 0x0A, 0x04, 0x04, 0x04],
    // 90 'Z'
    [0x1F, 0x01, 0x02, 0x04, 0x08, 0x10, 0x1F],
    // 91 '['
    [0x0E, 0x08, 0x08, 0x08, 0x08, 0x08, 0x0E],
    // 92 '\\'
    [0x10, 0x08, 0x04, 0x02, 0x01, 0x00, 0x00],
    // 93 ']'
    [0x0E, 0x02, 0x02, 0x02, 0x02, 0x02, 0x0E],
    // 94 '^'
    [0x04, 0x0A, 0x11, 0x00, 0x00, 0x00, 0x00],
    // 95 '_'
    [0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x1F],
    // 96 '`'
    [0x08, 0x04, 0x02, 0x00, 0x00, 0x00, 0x00],
    // 97 'a'
    [0x00, 0x00, 0x0E, 0x01, 0x0F, 0x11, 0x0F],
    // 98 'b'
    [0x10, 0x10, 0x16, 0x19, 0x11, 0x11, 0x1E],
    // 99 'c'
    [0x00, 0x00, 0x0E, 0x11, 0x10, 0x11, 0x0E],
    // 100 'd'
    [0x01, 0x01, 0x0D, 0x13, 0x11, 0x11, 0x0F],
    // 101 'e'
    [0x00, 0x00, 0x0E, 0x11, 0x1F, 0x10, 0x0E],
    // 102 'f'
    [0x06, 0x09, 0x08, 0x1C, 0x08, 0x08, 0x08],
    // 103 'g'
    [0x00, 0x0F, 0x11, 0x11, 0x0F, 0x01, 0x0E],
    // 104 'h'
    [0x10, 0x10, 0x16, 0x19, 0x11, 0x11, 0x11],
    // 105 'i'
    [0x04, 0x00, 0x0C, 0x04, 0x04, 0x04, 0x0E],
    // 106 'j'
    [0x02, 0x00, 0x06, 0x02, 0x02, 0x12, 0x0C],
    // 107 'k'
    [0x10, 0x10, 0x12, 0x14, 0x18, 0x14, 0x12],
    // 108 'l'
    [0x0C, 0x04, 0x04, 0x04, 0x04, 0x04, 0x0E],
    // 109 'm'
    [0x00, 0x00, 0x1A, 0x15, 0x15, 0x11, 0x11],
    // 110 'n'
    [0x00, 0x00, 0x16, 0x19, 0x11, 0x11, 0x11],
    // 111 'o'
    [0x00, 0x00, 0x0E, 0x11, 0x11, 0x11, 0x0E],
    // 112 'p'
    [0x00, 0x00, 0x1E, 0x11, 0x1E, 0x10, 0x10],
    // 113 'q'
    [0x00, 0x00, 0x0D, 0x13, 0x0F, 0x01, 0x01],
    // 114 'r'
    [0x00, 0x00, 0x16, 0x19, 0x10, 0x10, 0x10],
    // 115 's'
    [0x00, 0x00, 0x0F, 0x10, 0x0E, 0x01, 0x1E],
    // 116 't'
    [0x08, 0x08, 0x1C, 0x08, 0x08, 0x09, 0x06],
    // 117 'u'
    [0x00, 0x00, 0x11, 0x11, 0x11, 0x13, 0x0D],
    // 118 'v'
    [0x00, 0x00, 0x11, 0x11, 0x11, 0x0A, 0x04],
    // 119 'w'
    [0x00, 0x00, 0x11, 0x11, 0x15, 0x15, 0x0A],
    // 120 'x'
    [0x00, 0x00, 0x11, 0x0A, 0x04, 0x0A, 0x11],
    // 121 'y'
    [0x00, 0x00, 0x11, 0x11, 0x0F, 0x01, 0x0E],
    // 122 'z'
    [0x00, 0x00, 0x1F, 0x02, 0x04, 0x08, 0x1F],
    // 123 '{'
    [0x02, 0x04, 0x04, 0x08, 0x04, 0x04, 0x02],
    // 124 '|'
    [0x04, 0x04, 0x04, 0x04, 0x04, 0x04, 0x04],
    // 125 '}'
    [0x08, 0x04, 0x04, 0x02, 0x04, 0x04, 0x08],
    // 126 '~'
    [0x00, 0x08, 0x15, 0x02, 0x00, 0x00, 0x00],
];

fn get_glyph(ch: char) -> [u8; 7] {
    let c = ch as usize;
    if (32..=126).contains(&c) {
        FONT_5X7[c - 32]
    } else {
        [0, 0, 0, 0, 0, 0, 0]
    }
}

// ----------------------------------------------------------------------------
// High-Resolution 2D Raster Drawing Utilities
// ----------------------------------------------------------------------------

#[inline]
fn blend_pixel(dest: &mut Rgba<u8>, src: Rgba<u8>) {
    if src[3] == 0 {
        return;
    }
    if src[3] == 255 {
        *dest = src;
        return;
    }
    let sa = src[3] as u32;
    let da = dest[3] as u32;
    let inv_sa = 255 - sa;
    let out_a = sa + (da * inv_sa) / 255;
    if out_a == 0 {
        return;
    }
    let r = ((src[0] as u32 * sa) + (dest[0] as u32 * da * inv_sa / 255)) / out_a;
    let g = ((src[1] as u32 * sa) + (dest[1] as u32 * da * inv_sa / 255)) / out_a;
    let b = ((src[2] as u32 * sa) + (dest[2] as u32 * da * inv_sa / 255)) / out_a;
    *dest = Rgba([r as u8, g as u8, b as u8, out_a as u8]);
}

#[inline]
fn put_pixel_safe(img: &mut ImageBuffer<Rgba<u8>, Vec<u8>>, x: i32, y: i32, color: Rgba<u8>) {
    if x >= 0 && x < img.width() as i32 && y >= 0 && y < img.height() as i32 {
        if color[3] == 255 {
            img.put_pixel(x as u32, y as u32, color);
        } else {
            let px = img.get_pixel_mut(x as u32, y as u32);
            blend_pixel(px, color);
        }
    }
}

fn fill_rect(
    img: &mut ImageBuffer<Rgba<u8>, Vec<u8>>,
    x: i32,
    y: i32,
    w: i32,
    h: i32,
    color: Rgba<u8>,
) {
    let x_start = x.max(0);
    let y_start = y.max(0);
    let x_end = (x + w).min(img.width() as i32);
    let y_end = (y + h).min(img.height() as i32);

    for py in y_start..y_end {
        for px in x_start..x_end {
            put_pixel_safe(img, px, py, color);
        }
    }
}

fn fill_rounded_rect(
    img: &mut ImageBuffer<Rgba<u8>, Vec<u8>>,
    x: i32,
    y: i32,
    w: i32,
    h: i32,
    r: i32,
    color: Rgba<u8>,
) {
    let r = r.min(w / 2).min(h / 2);
    if r <= 0 {
        fill_rect(img, x, y, w, h, color);
        return;
    }
    // Main inner horizontal and vertical blocks
    fill_rect(img, x + r, y, w - 2 * r, h, color);
    fill_rect(img, x, y + r, r, h - 2 * r, color);
    fill_rect(img, x + w - r, y + r, r, h - 2 * r, color);

    // 4 corners: circle quadrants
    let corners = [
        (x + r, y + r),
        (x + w - r - 1, y + r),
        (x + r, y + h - r - 1),
        (x + w - r - 1, y + h - r - 1),
    ];
    let r2 = r * r;
    for dy in 0..=r {
        for dx in 0..=r {
            if dx * dx + dy * dy <= r2 {
                put_pixel_safe(img, corners[0].0 - dx, corners[0].1 - dy, color);
                put_pixel_safe(img, corners[1].0 + dx, corners[1].1 - dy, color);
                put_pixel_safe(img, corners[2].0 - dx, corners[2].1 + dy, color);
                put_pixel_safe(img, corners[3].0 + dx, corners[3].1 + dy, color);
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn stroke_rounded_rect(
    img: &mut ImageBuffer<Rgba<u8>, Vec<u8>>,
    x: i32,
    y: i32,
    w: i32,
    h: i32,
    r: i32,
    thickness: i32,
    color: Rgba<u8>,
) {
    let r = r.min(w / 2).min(h / 2);
    // Straight edges
    fill_rect(img, x + r, y, w - 2 * r, thickness, color);
    fill_rect(img, x + r, y + h - thickness, w - 2 * r, thickness, color);
    fill_rect(img, x, y + r, thickness, h - 2 * r, color);
    fill_rect(img, x + w - thickness, y + r, thickness, h - 2 * r, color);

    // 4 corners
    let corners = [
        (x + r, y + r),
        (x + w - r - 1, y + r),
        (x + r, y + h - r - 1),
        (x + w - r - 1, y + h - r - 1),
    ];
    let r_outer2 = r * r;
    let r_inner = (r - thickness).max(0);
    let r_inner2 = r_inner * r_inner;
    for dy in 0..=r {
        for dx in 0..=r {
            let d2 = dx * dx + dy * dy;
            if d2 <= r_outer2 && d2 >= r_inner2 {
                put_pixel_safe(img, corners[0].0 - dx, corners[0].1 - dy, color);
                put_pixel_safe(img, corners[1].0 + dx, corners[1].1 - dy, color);
                put_pixel_safe(img, corners[2].0 - dx, corners[2].1 + dy, color);
                put_pixel_safe(img, corners[3].0 + dx, corners[3].1 + dy, color);
            }
        }
    }
}

fn fill_circle(
    img: &mut ImageBuffer<Rgba<u8>, Vec<u8>>,
    cx: i32,
    cy: i32,
    r: i32,
    color: Rgba<u8>,
) {
    let r2 = r * r;
    for dy in -r..=r {
        for dx in -r..=r {
            if dx * dx + dy * dy <= r2 {
                put_pixel_safe(img, cx + dx, cy + dy, color);
            }
        }
    }
}

fn stroke_circle(
    img: &mut ImageBuffer<Rgba<u8>, Vec<u8>>,
    cx: i32,
    cy: i32,
    r: i32,
    thickness: i32,
    color: Rgba<u8>,
) {
    let r_outer2 = r * r;
    let r_inner = (r - thickness).max(0);
    let r_inner2 = r_inner * r_inner;
    for dy in -r..=r {
        for dx in -r..=r {
            let d2 = dx * dx + dy * dy;
            if d2 <= r_outer2 && d2 >= r_inner2 {
                put_pixel_safe(img, cx + dx, cy + dy, color);
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn draw_straight_line(
    img: &mut ImageBuffer<Rgba<u8>, Vec<u8>>,
    x1: i32,
    y1: i32,
    x2: i32,
    y2: i32,
    color: Rgba<u8>,
    thickness: i32,
    glow: bool,
) {
    if glow {
        let glow_color = Rgba([color[0], color[1], color[2], 70]);
        let t_glow = thickness + 2;
        if y1 == y2 {
            let min_x = x1.min(x2) - 1;
            let max_x = x1.max(x2) + 1;
            fill_rect(
                img,
                min_x,
                y1 - t_glow / 2,
                max_x - min_x + 1,
                t_glow,
                glow_color,
            );
        } else if x1 == x2 {
            let min_y = y1.min(y2) - 1;
            let max_y = y1.max(y2) + 1;
            fill_rect(
                img,
                x1 - t_glow / 2,
                min_y,
                t_glow,
                max_y - min_y + 1,
                glow_color,
            );
        } else {
            KittyGraphRenderer::draw_line_px(img, x1, y1, x2, y2, glow_color, t_glow);
        }
    }

    if y1 == y2 {
        let min_x = x1.min(x2);
        let max_x = x1.max(x2);
        fill_rect(
            img,
            min_x,
            y1 - thickness / 2,
            max_x - min_x + 1,
            thickness,
            color,
        );
    } else if x1 == x2 {
        let min_y = y1.min(y2);
        let max_y = y1.max(y2);
        fill_rect(
            img,
            x1 - thickness / 2,
            min_y,
            thickness,
            max_y - min_y + 1,
            color,
        );
    } else {
        KittyGraphRenderer::draw_line_px(img, x1, y1, x2, y2, color, thickness);
    }
}

fn draw_arrowhead_left(
    img: &mut ImageBuffer<Rgba<u8>, Vec<u8>>,
    tip_x: i32,
    tip_y: i32,
    size: i32,
    color: Rgba<u8>,
) {
    let size = size.max(1);
    for dx in 0..size {
        let span = (dx * 3) / 4;
        for dy in -span..=span {
            put_pixel_safe(img, tip_x + dx, tip_y + dy, color);
        }
    }
}

fn draw_arrowhead_right(
    img: &mut ImageBuffer<Rgba<u8>, Vec<u8>>,
    tip_x: i32,
    tip_y: i32,
    size: i32,
    color: Rgba<u8>,
) {
    let size = size.max(1);
    for dx in 0..size {
        let span = (dx * 3) / 4;
        for dy in -span..=span {
            put_pixel_safe(img, tip_x - dx, tip_y + dy, color);
        }
    }
}

fn draw_arrowhead_up(
    img: &mut ImageBuffer<Rgba<u8>, Vec<u8>>,
    tip_x: i32,
    tip_y: i32,
    size: i32,
    color: Rgba<u8>,
) {
    let size = size.max(1);
    for dy in 0..size {
        let span = (dy * 3) / 4;
        for dx in -span..=span {
            put_pixel_safe(img, tip_x + dx, tip_y + dy, color);
        }
    }
}

fn draw_arrowhead_down(
    img: &mut ImageBuffer<Rgba<u8>, Vec<u8>>,
    tip_x: i32,
    tip_y: i32,
    size: i32,
    color: Rgba<u8>,
) {
    let size = size.max(1);
    for dy in 0..size {
        let span = (dy * 3) / 4;
        for dx in -span..=span {
            put_pixel_safe(img, tip_x + dx, tip_y - dy, color);
        }
    }
}

fn draw_text(
    img: &mut ImageBuffer<Rgba<u8>, Vec<u8>>,
    x: i32,
    y: i32,
    text: &str,
    color: Rgba<u8>,
    scale: i32,
) {
    let scale = scale.max(1);
    let mut cur_x = x;
    for ch in text.chars() {
        let glyph = get_glyph(ch);
        for (row, &row_bits) in glyph.iter().enumerate() {
            for col in 0..5i32 {
                if (row_bits & (1 << (4 - col))) != 0 {
                    fill_rect(
                        img,
                        cur_x + col * scale,
                        y + row as i32 * scale,
                        scale,
                        scale,
                        color,
                    );
                }
            }
        }
        cur_x += (5 + 1) * scale;
    }
}

fn draw_pill_badge(
    img: &mut ImageBuffer<Rgba<u8>, Vec<u8>>,
    cx: i32,
    cy: i32,
    text: &str,
    bg_color: Rgba<u8>,
    border_color: Rgba<u8>,
    text_color: Rgba<u8>,
) {
    let char_w = 6;
    let text_w = text.len() as i32 * char_w;
    let padding_x = 5;
    let w = text_w + padding_x * 2;
    let h = 13;
    let x = cx - w / 2;
    let y = cy - h / 2;
    let r = h / 2;

    fill_rounded_rect(img, x, y, w, h, r, bg_color);
    stroke_rounded_rect(img, x, y, w, h, r, 1, border_color);

    let tx = x + padding_x;
    let ty = y + 3;
    draw_text(img, tx, ty, text, text_color, 1);
}

// ----------------------------------------------------------------------------
// SVG-Style Iconography Renderers
// ----------------------------------------------------------------------------

fn draw_router_icon(
    img: &mut ImageBuffer<Rgba<u8>, Vec<u8>>,
    cx: i32,
    cy: i32,
    radius: i32,
    router_color: Rgba<u8>,
    _is_selected: bool,
) {
    let r = radius.max(6);
    // Dark disc fill
    fill_circle(img, cx, cy, r, Rgba([26, 43, 76, 255]));
    // Outer cyan ring
    stroke_circle(img, cx, cy, r, 2, router_color);

    // Center core
    let core_r = (r / 5).max(1);
    fill_circle(img, cx, cy, core_r, Rgba([255, 255, 255, 255]));

    let arrow_size = (r / 3).max(3);
    let stem_color = Rgba([224, 242, 254, 255]); // bright white-cyan

    // 4 Multidirectional arrows (Universal network router topology symbol)
    // Left arrow pointing Left
    let left_tip = cx - r + 2;
    draw_straight_line(
        img,
        cx - core_r - 1,
        cy,
        left_tip + arrow_size,
        cy,
        stem_color,
        2,
        false,
    );
    draw_arrowhead_left(img, left_tip, cy, arrow_size, stem_color);

    // Right arrow pointing Right
    let right_tip = cx + r - 2;
    draw_straight_line(
        img,
        cx + core_r + 1,
        cy,
        right_tip - arrow_size,
        cy,
        stem_color,
        2,
        false,
    );
    draw_arrowhead_right(img, right_tip, cy, arrow_size, stem_color);

    // Top arrow pointing Up
    let top_tip = cy - r + 2;
    draw_straight_line(
        img,
        cx,
        cy - core_r - 1,
        cx,
        top_tip + arrow_size,
        stem_color,
        2,
        false,
    );
    draw_arrowhead_up(img, cx, top_tip, arrow_size, stem_color);

    // Bottom arrow pointing Down
    let bot_tip = cy + r - 2;
    draw_straight_line(
        img,
        cx,
        cy + core_r + 1,
        cx,
        bot_tip - arrow_size,
        stem_color,
        2,
        false,
    );
    draw_arrowhead_down(img, cx, bot_tip, arrow_size, stem_color);
}

fn draw_switch_icon(
    img: &mut ImageBuffer<Rgba<u8>, Vec<u8>>,
    cx: i32,
    cy: i32,
    w: i32,
    h: i32,
    switch_color: Rgba<u8>,
    _is_selected: bool,
) {
    let w = w.max(14);
    let h = h.max(10);
    let x = cx - w / 2;
    let y = cy - h / 2;

    // Chassis body: dark teal fill
    fill_rounded_rect(img, x, y, w, h, 3, Rgba([19, 56, 58, 255]));
    // Border
    stroke_rounded_rect(img, x, y, w, h, 3, 2, switch_color);

    // Horizontal divider
    draw_straight_line(
        img,
        x + 2,
        cy,
        x + w - 2,
        cy,
        Rgba([41, 102, 104, 255]),
        1,
        false,
    );

    // Top layer switching arrow (pointing right)
    let top_y = cy - h / 4;
    let arrow_size = (h / 4).max(3);
    let right_tip = x + w - 4;
    let arrow_color = Rgba([180, 249, 248, 255]); // bright teal
    draw_straight_line(
        img,
        x + 4,
        top_y,
        right_tip - arrow_size,
        top_y,
        arrow_color,
        2,
        false,
    );
    draw_arrowhead_right(img, right_tip, top_y, arrow_size, arrow_color);

    // Bottom layer switching arrow (pointing left)
    let bot_y = cy + h / 4;
    let left_tip = x + 4;
    draw_straight_line(
        img,
        left_tip + arrow_size,
        bot_y,
        x + w - 4,
        bot_y,
        arrow_color,
        2,
        false,
    );
    draw_arrowhead_left(img, left_tip, bot_y, arrow_size, arrow_color);

    // Port LED indicators along top edge
    let num_leds = 4.min(w / 6);
    let led_spacing = (w - 6) / (num_leds + 1);
    for i in 0..num_leds {
        let lx = x + 4 + (i + 1) * led_spacing;
        let ly = y + 2;
        let led_col = if i % 2 == 0 {
            Rgba([158, 206, 106, 255]) // green
        } else {
            Rgba([115, 218, 202, 255]) // teal
        };
        put_pixel_safe(img, lx, ly, led_col);
    }
}

fn draw_firewall_icon(
    img: &mut ImageBuffer<Rgba<u8>, Vec<u8>>,
    cx: i32,
    cy: i32,
    w: i32,
    h: i32,
    fw_color: Rgba<u8>,
    _is_selected: bool,
) {
    let w = w.max(12);
    let h = h.max(10);
    let x = cx - w / 2;
    let y = cy - h / 2;

    // Dark crimson fill
    fill_rounded_rect(img, x, y, w, h, 2, Rgba([59, 28, 40, 255]));
    // Outer border
    stroke_rounded_rect(img, x, y, w, h, 2, 2, fw_color);

    // Staggered brickwall courses
    let num_courses = 3;
    let row_h = (h - 2) / num_courses;
    let mortar_color = Rgba([255, 158, 100, 255]); // orange/coral mortar

    for r in 1..num_courses {
        let my = y + 1 + r * row_h;
        draw_straight_line(img, x + 2, my, x + w - 2, my, mortar_color, 1, false);
    }

    // Vertical brick joints
    for r in 0..num_courses {
        let course_y1 = y + 1 + r * row_h;
        let course_y2 = (course_y1 + row_h).min(y + h - 2);
        if r % 2 == 0 {
            let j1 = x + w / 3;
            let j2 = x + 2 * w / 3;
            draw_straight_line(img, j1, course_y1, j1, course_y2, mortar_color, 1, false);
            draw_straight_line(img, j2, course_y1, j2, course_y2, mortar_color, 1, false);
        } else {
            let jm = x + w / 2;
            draw_straight_line(img, jm, course_y1, jm, course_y2, mortar_color, 1, false);
        }
    }

    // Central shield emblem
    let shield_w = 4;
    let shield_h = 5;
    fill_rect(
        img,
        cx - shield_w / 2,
        cy - shield_h / 2,
        shield_w,
        shield_h,
        Rgba([224, 175, 104, 255]),
    );
}

fn draw_host_icon(
    img: &mut ImageBuffer<Rgba<u8>, Vec<u8>>,
    cx: i32,
    cy: i32,
    w: i32,
    h: i32,
    host_color: Rgba<u8>,
    _is_selected: bool,
) {
    let w = w.max(14);
    let h = h.max(10);
    let x = cx - w / 2;
    let y = cy - h / 2;

    // Stacked chassis (2 horizontal server blades)
    let blade_h = (h - 3) / 2;
    for b in 0..2 {
        let by = y + b * (blade_h + 2);
        fill_rounded_rect(img, x, by, w, blade_h, 2, Rgba([40, 32, 61, 255]));
        stroke_rounded_rect(img, x, by, w, blade_h, 2, 1, host_color);

        // Drive bay slots
        let slot_color = Rgba([86, 95, 137, 255]);
        let slot_w = (w / 3).max(4);
        draw_straight_line(
            img,
            x + 3,
            by + blade_h / 2,
            x + 3 + slot_w,
            by + blade_h / 2,
            slot_color,
            1,
            false,
        );

        // Front panel LEDs
        let led_green = Rgba([158, 206, 106, 255]); // Power LED
        let led_amber = Rgba([224, 175, 104, 255]); // Activity LED
        put_pixel_safe(img, x + w - 5, by + blade_h / 2, led_green);
        put_pixel_safe(img, x + w - 3, by + blade_h / 2, led_amber);
    }
}

// ----------------------------------------------------------------------------
// Subpixel In-Card Iconography (for Ratatui Half-Block Rasterization)
// ----------------------------------------------------------------------------

fn draw_subpixel_router(img: &mut ImageBuffer<Rgba<u8>, Vec<u8>>, cx: i32, cy: i32, col: Rgba<u8>) {
    // 5x5 circular glyph with multi-arrow cross
    for dy in -2..=2 {
        for dx in -2..=2 {
            let d2 = dx * dx + dy * dy;
            if (2..=5).contains(&d2) {
                put_pixel_safe(img, cx + dx, cy + dy, col);
            }
        }
    }
    // Arrows / cross core
    put_pixel_safe(img, cx, cy, Rgba([255, 255, 255, 255]));
    put_pixel_safe(img, cx - 2, cy, Rgba([255, 255, 255, 255]));
    put_pixel_safe(img, cx + 2, cy, Rgba([255, 255, 255, 255]));
    put_pixel_safe(img, cx, cy - 2, Rgba([255, 255, 255, 255]));
    put_pixel_safe(img, cx, cy + 2, Rgba([255, 255, 255, 255]));
}

fn draw_subpixel_switch(img: &mut ImageBuffer<Rgba<u8>, Vec<u8>>, cx: i32, cy: i32, col: Rgba<u8>) {
    // 6x5 chassis with opposing arrows
    let x = cx - 3;
    let y = cy - 2;
    fill_rect(img, x, y, 6, 5, Rgba([19, 56, 58, 255]));
    for px in x..x + 6 {
        put_pixel_safe(img, px, y, col);
        put_pixel_safe(img, px, y + 4, col);
    }
    put_pixel_safe(img, x, y + 2, col);
    put_pixel_safe(img, x + 5, y + 2, col);

    // Top right arrow (x+1..=x+4)
    put_pixel_safe(img, x + 2, y + 1, Rgba([180, 249, 248, 255]));
    put_pixel_safe(img, x + 4, y + 1, Rgba([255, 255, 255, 255])); // arrowhead

    // Bottom left arrow
    put_pixel_safe(img, x + 3, y + 3, Rgba([180, 249, 248, 255]));
    put_pixel_safe(img, x + 1, y + 3, Rgba([255, 255, 255, 255])); // arrowhead
}

fn draw_subpixel_firewall(
    img: &mut ImageBuffer<Rgba<u8>, Vec<u8>>,
    cx: i32,
    cy: i32,
    col: Rgba<u8>,
) {
    let x = cx - 3;
    let y = cy - 2;
    fill_rect(img, x, y, 6, 5, Rgba([59, 28, 40, 255]));
    for px in x..x + 6 {
        put_pixel_safe(img, px, y, col);
        put_pixel_safe(img, px, y + 4, col);
    }
    let mortar = Rgba([255, 158, 100, 255]);
    for px in x + 1..x + 5 {
        put_pixel_safe(img, px, y + 2, mortar);
    }
    put_pixel_safe(img, x + 2, y + 1, mortar);
    put_pixel_safe(img, x + 4, y + 1, mortar);
    put_pixel_safe(img, x + 3, y + 3, mortar);
}

fn draw_subpixel_host(img: &mut ImageBuffer<Rgba<u8>, Vec<u8>>, cx: i32, cy: i32, col: Rgba<u8>) {
    let x = cx - 3;
    let y = cy - 2;
    // 2 chassis blades
    fill_rect(img, x, y, 6, 2, Rgba([40, 32, 61, 255]));
    fill_rect(img, x, y + 3, 6, 2, Rgba([40, 32, 61, 255]));

    put_pixel_safe(img, x, y, col);
    put_pixel_safe(img, x + 5, y, col);
    put_pixel_safe(img, x, y + 3, col);
    put_pixel_safe(img, x + 5, y + 3, col);

    // Green power LED & Amber activity LED
    put_pixel_safe(img, x + 4, y + 1, Rgba([158, 206, 106, 255]));
    put_pixel_safe(img, x + 4, y + 4, Rgba([224, 175, 104, 255]));
}

// ----------------------------------------------------------------------------
// Kitty Graph Renderer
// ----------------------------------------------------------------------------

pub struct KittyGraphRenderer;

impl KittyGraphRenderer {
    /// Render high-resolution raster image of the network canvas with SVG-style vector iconography
    pub fn render_image(
        canvas: &CanvasState,
        width_px: u32,
        height_px: u32,
        _theme: &Theme,
    ) -> ImageBuffer<Rgba<u8>, Vec<u8>> {
        let mut img: ImageBuffer<Rgba<u8>, Vec<u8>> =
            ImageBuffer::from_pixel(width_px, height_px, Rgba([26, 27, 38, 255])); // #1a1b26 background

        let scale_x = width_px as f64 / 120.0;
        let scale_y = height_px as f64 / 45.0;

        let to_screen_px = |cx: f64, cy: f64| -> (i32, i32) {
            let px = ((cx * canvas.zoom + canvas.offset_x) * scale_x).round() as i32;
            let py = ((cy * canvas.zoom + canvas.offset_y) * scale_y).round() as i32;
            (px, py)
        };

        // 1. Draw subtle background grid dots
        let grid_spacing = 4.0;
        let total_cols = ((width_px as f64 / scale_x).round() as u16).max(1);
        let total_rows = ((height_px as f64 / scale_y).round() as u16).max(1);
        let start_canvas = canvas.screen_to_canvas(0, 0);
        let end_canvas = canvas.screen_to_canvas(total_cols, total_rows);

        if start_canvas.0.is_finite()
            && start_canvas.1.is_finite()
            && end_canvas.0.is_finite()
            && end_canvas.1.is_finite()
        {
            let span_x = (end_canvas.0 - start_canvas.0).abs();
            let span_y = (end_canvas.1 - start_canvas.1).abs();
            if span_x < 2000.0 && span_y < 2000.0 {
                let mut cy = (start_canvas.1 / grid_spacing).floor() * grid_spacing;
                let mut loop_y = 0;
                while cy <= end_canvas.1 && loop_y < 1000 {
                    loop_y += 1;
                    let mut cx = (start_canvas.0 / grid_spacing).floor() * grid_spacing;
                    let mut loop_x = 0;
                    while cx <= end_canvas.0 && loop_x < 1000 {
                        loop_x += 1;
                        let (px, py) = to_screen_px(cx, cy);
                        if px >= 0 && px < width_px as i32 && py >= 0 && py < height_px as i32 {
                            img.put_pixel(px as u32, py as u32, Rgba([41, 46, 66, 255]));
                        }
                        cx += grid_spacing;
                    }
                    cy += grid_spacing;
                }
            }
        }

        // 2. Draw Links (Straight orthogonal lines with glow, junction dots & interface pill badges)
        for (i, link) in canvas.links.iter().enumerate() {
            let is_sel = canvas.selected_link == Some(i);
            let link_color = if is_sel {
                Rgba([224, 175, 104, 255]) // Gold
            } else {
                Rgba([122, 162, 247, 255]) // Tokyo Blue
            };

            for win in link.waypoints.windows(2) {
                let (p1, p2) = (win[0], win[1]);
                let (x1, y1) = to_screen_px(p1.0, p1.1);
                let (x2, y2) = to_screen_px(p2.0, p2.1);
                draw_straight_line(&mut img, x1, y1, x2, y2, link_color, 2, true);
            }

            // Waypoint corner junctions
            if link.waypoints.len() >= 3 {
                for wp in &link.waypoints[1..link.waypoints.len() - 1] {
                    let (jx, jy) = to_screen_px(wp.0, wp.1);
                    fill_circle(&mut img, jx, jy, 3, link_color);
                    fill_circle(&mut img, jx, jy, 1, Rgba([255, 255, 255, 255]));
                }
            }

            // Interface label pill badges at endpoints
            if link.waypoints.len() >= 2 {
                let pair = if link.source_node <= link.target_node {
                    (&link.source_node, &link.target_node)
                } else {
                    (&link.target_node, &link.source_node)
                };
                let parallel: Vec<usize> = canvas
                    .links
                    .iter()
                    .enumerate()
                    .filter(|(_, l)| {
                        let p = if l.source_node <= l.target_node {
                            (&l.source_node, &l.target_node)
                        } else {
                            (&l.target_node, &l.source_node)
                        };
                        p == pair
                    })
                    .map(|(idx, _)| idx)
                    .collect();
                let total = parallel.len();
                let rank = parallel.iter().position(|&idx| idx == i).unwrap_or(0);
                let badge_dist = 24 + if total > 1 { (rank as i32) * 16 } else { 0 };

                let (sx1, sy1) = to_screen_px(link.waypoints[0].0, link.waypoints[0].1);
                let (sx2, sy2) = to_screen_px(link.waypoints[1].0, link.waypoints[1].1);
                let (bx1, by1) = Self::compute_badge_pos(sx1, sy1, sx2, sy2, badge_dist);
                let src_label = format!("[{}]", link.source_port);
                draw_pill_badge(
                    &mut img,
                    bx1,
                    by1,
                    &src_label,
                    Rgba([31, 35, 53, 230]),
                    link_color,
                    Rgba([255, 255, 255, 255]),
                );

                let n = link.waypoints.len();
                let (tx1, ty1) = to_screen_px(link.waypoints[n - 2].0, link.waypoints[n - 2].1);
                let (tx2, ty2) = to_screen_px(link.waypoints[n - 1].0, link.waypoints[n - 1].1);
                let (bx2, by2) = Self::compute_badge_pos(tx2, ty2, tx1, ty1, badge_dist);
                let tgt_label = format!("[{}]", link.target_port);
                draw_pill_badge(
                    &mut img,
                    bx2,
                    by2,
                    &tgt_label,
                    Rgba([31, 35, 53, 230]),
                    link_color,
                    Rgba([255, 255, 255, 255]),
                );
            }
        }

        // 3. Active wiring preview
        if canvas.mode == CanvasMode::Wiring {
            if let Some((ref src_name, ref src_port)) = canvas.wiring_source {
                if let Some(node) = canvas.nodes.get(src_name) {
                    let ((px, py), _) = if let Some(port) = node.get_port(src_port) {
                        ((port.x, port.y), port.direction)
                    } else {
                        node.perimeter_connection_towards(
                            canvas.wiring_cursor.0,
                            canvas.wiring_cursor.1,
                        )
                    };
                    let (x1, y1) = to_screen_px(px, py);
                    let (x2, y2) = to_screen_px(canvas.wiring_cursor.0, canvas.wiring_cursor.1);
                    let wire_color = Rgba([224, 175, 104, 255]);
                    match canvas.routing_style {
                        crate::canvas::link::RoutingStyle::Direct => {
                            draw_straight_line(&mut img, x1, y1, x2, y2, wire_color, 2, true);
                        }
                        crate::canvas::link::RoutingStyle::Octilinear => {
                            let dx = x2 - x1;
                            let dy = y2 - y1;
                            if dx == 0 || dy == 0 || dx.abs() == dy.abs() {
                                draw_straight_line(&mut img, x1, y1, x2, y2, wire_color, 2, true);
                            } else if dx.abs() > dy.abs() {
                                let mid_x = x1 + dy.abs() * dx.signum();
                                draw_straight_line(
                                    &mut img, x1, y1, mid_x, y2, wire_color, 2, true,
                                );
                                draw_straight_line(
                                    &mut img, mid_x, y2, x2, y2, wire_color, 2, true,
                                );
                            } else {
                                let mid_y = y1 + dx.abs() * dy.signum();
                                draw_straight_line(
                                    &mut img, x1, y1, x2, mid_y, wire_color, 2, true,
                                );
                                draw_straight_line(
                                    &mut img, x2, mid_y, x2, y2, wire_color, 2, true,
                                );
                            }
                        }
                        crate::canvas::link::RoutingStyle::Orthogonal => {
                            draw_straight_line(&mut img, x1, y1, x2, y1, wire_color, 2, true);
                            draw_straight_line(&mut img, x2, y1, x2, y2, wire_color, 2, true);
                        }
                    }

                    // Pulsing cursor halo
                    stroke_circle(&mut img, x2, y2, 6, 1, Rgba([255, 215, 0, 160]));
                    fill_circle(&mut img, x2, y2, 3, Rgba([255, 215, 0, 255]));
                }
            }
        }

        // 4. Draw Nodes with SVG-style iconography, drop shadows, rounded cards, and badges
        for (name, node) in &canvas.nodes {
            let (nx, ny) = to_screen_px(node.x, node.y);
            let nw = ((node.width * canvas.zoom) * scale_x).round() as i32;
            let nh = ((node.height * canvas.zoom) * scale_y).round() as i32;

            let is_sel = canvas.selected_nodes.contains(name)
                || canvas.selected_node.as_deref() == Some(name.as_str());
            let (cat_color, cat_bg) = match node.category {
                crate::clab::model::NodeCategory::Router => {
                    (Rgba([125, 207, 255, 255]), Rgba([26, 43, 76, 255]))
                }
                crate::clab::model::NodeCategory::Switch => {
                    (Rgba([115, 218, 202, 255]), Rgba([19, 56, 58, 255]))
                }
                crate::clab::model::NodeCategory::Host => {
                    (Rgba([187, 154, 247, 255]), Rgba([40, 32, 61, 255]))
                }
                crate::clab::model::NodeCategory::Firewall => {
                    (Rgba([247, 118, 142, 255]), Rgba([59, 28, 40, 255]))
                }
            };

            let border_color = if is_sel {
                Rgba([224, 175, 104, 255]) // Gold
            } else {
                cat_color
            };
            let fill_color = if is_sel {
                Rgba([47, 53, 79, 255])
            } else {
                Rgba([36, 40, 59, 255])
            };

            // Soft drop shadow for 3D card elevation
            fill_rounded_rect(&mut img, nx + 3, ny + 4, nw, nh, 6, Rgba([10, 10, 16, 120]));

            // Card body fill
            fill_rounded_rect(&mut img, nx, ny, nw, nh, 6, fill_color);

            // Card border
            stroke_rounded_rect(&mut img, nx, ny, nw, nh, 6, 2, border_color);

            // If selected, glowing halo
            if is_sel {
                stroke_rounded_rect(
                    &mut img,
                    nx - 2,
                    ny - 2,
                    nw + 4,
                    nh + 4,
                    8,
                    1,
                    Rgba([224, 175, 104, 150]),
                );
            }

            // Top header pill background
            let header_h = (nh / 3).clamp(12, 20);
            fill_rounded_rect(&mut img, nx + 2, ny + 2, nw - 4, header_h, 4, cat_bg);

            // SVG-style Iconography
            let icon_r = (nh / 4).clamp(6, 16);
            let icon_cx = nx + icon_r + 6;
            let icon_cy = ny + nh / 2;

            match node.category {
                crate::clab::model::NodeCategory::Router => {
                    draw_router_icon(&mut img, icon_cx, icon_cy, icon_r, cat_color, is_sel);
                }
                crate::clab::model::NodeCategory::Switch => {
                    let sw = (icon_r * 2).max(14);
                    let sh = (icon_r * 7 / 5).max(10);
                    draw_switch_icon(&mut img, icon_cx, icon_cy, sw, sh, cat_color, is_sel);
                }
                crate::clab::model::NodeCategory::Firewall => {
                    let fw_w = (icon_r * 2 - 2).max(12);
                    let fw_h = (icon_r * 7 / 5).max(10);
                    draw_firewall_icon(&mut img, icon_cx, icon_cy, fw_w, fw_h, cat_color, is_sel);
                }
                crate::clab::model::NodeCategory::Host => {
                    let hw_w = (icon_r * 2).max(14);
                    let hw_h = (icon_r * 7 / 5).max(10);
                    draw_host_icon(&mut img, icon_cx, icon_cy, hw_w, hw_h, cat_color, is_sel);
                }
            }

            // Typography: Node Name
            let name_x = icon_cx + icon_r + 8;
            let name_y = ny + 4;
            let name_color = if is_sel {
                Rgba([224, 175, 104, 255])
            } else {
                Rgba([240, 240, 255, 255])
            };
            let font_scale = if nw > 100 && nh > 40 { 2 } else { 1 };
            draw_text(&mut img, name_x, name_y, name, name_color, font_scale);

            // Node Kind pill badge
            if nh >= 24 {
                let kind_y = name_y + 8 * font_scale + 2;
                let kind_w = (node.kind.len() as i32 * 6 + 10).max(20);
                let kind_pill_cx = name_x + kind_w / 2;
                draw_pill_badge(
                    &mut img,
                    kind_pill_cx,
                    kind_y + 6,
                    &node.kind,
                    Rgba([26, 27, 38, 200]),
                    border_color,
                    Rgba([169, 177, 214, 255]),
                );
            }
        }

        // 5. Draw marquee selection box if active
        if let Some((p1, p2)) = canvas.selection_box {
            let (sx1, sy1) = canvas.canvas_to_screen(p1.0, p1.1);
            let (sx2, sy2) = canvas.canvas_to_screen(p2.0, p2.1);
            let px1 = (sx1 as f64 * scale_x).round() as i32;
            let py1 = (sy1 as f64 * scale_y).round() as i32;
            let px2 = (sx2 as f64 * scale_x).round() as i32;
            let py2 = (sy2 as f64 * scale_y).round() as i32;
            let min_px = px1.min(px2);
            let max_px = px1.max(px2);
            let min_py = py1.min(py2);
            let max_py = py1.max(py2);
            let box_color = Rgba([122, 162, 247, 220]);
            Self::draw_line_px(&mut img, min_px, min_py, max_px, min_py, box_color, 1);
            Self::draw_line_px(&mut img, min_px, max_py, max_px, max_py, box_color, 1);
            Self::draw_line_px(&mut img, min_px, min_py, min_px, max_py, box_color, 1);
            Self::draw_line_px(&mut img, max_px, min_py, max_px, max_py, box_color, 1);
        }

        img
    }

    /// Calculate badge center along a line segment offset from the anchor point
    fn compute_badge_pos(x1: i32, y1: i32, x2: i32, y2: i32, offset: i32) -> (i32, i32) {
        let dx = x2 - x1;
        let dy = y2 - y1;
        let dist = ((dx * dx + dy * dy) as f64).sqrt().max(1.0);
        let actual_offset = (offset as f64).min(dist * 0.45);
        let bx = (x1 as f64 + (dx as f64 / dist) * actual_offset).round() as i32;
        let by = (y1 as f64 + (dy as f64 / dist) * actual_offset).round() as i32;
        (bx, by)
    }

    /// Draw a line on image with given thickness (smooth antialiased pixel Bresenham line)
    pub fn draw_line_px(
        img: &mut ImageBuffer<Rgba<u8>, Vec<u8>>,
        x1: i32,
        y1: i32,
        x2: i32,
        y2: i32,
        color: Rgba<u8>,
        thickness: i32,
    ) {
        let w = img.width() as i32;
        let h = img.height() as i32;
        if w <= 0 || h <= 0 {
            return;
        }

        let t = thickness.max(1);
        let pad = t + 2;

        let min_x = (x1.min(x2) - pad).clamp(0, w - 1);
        let max_x = (x1.max(x2) + pad).clamp(0, w - 1);
        let min_y = (y1.min(y2) - pad).clamp(0, h - 1);
        let max_y = (y1.max(y2) + pad).clamp(0, h - 1);

        if min_x > max_x || min_y > max_y {
            return;
        }

        let vx = (x2 - x1) as f32;
        let vy = (y2 - y1) as f32;
        let len_sq = vx * vx + vy * vy;
        let half_t = (t as f32) / 2.0;

        if len_sq < 1e-4 {
            for py in min_y..=max_y {
                for px in min_x..=max_x {
                    let d = (((px - x1) * (px - x1) + (py - y1) * (py - y1)) as f32).sqrt();
                    if d <= half_t {
                        put_pixel_safe(img, px, py, color);
                    } else if d < half_t + 1.0 {
                        let alpha = ((color[3] as f32) * (1.0 - (d - half_t))).round() as u8;
                        put_pixel_safe(img, px, py, Rgba([color[0], color[1], color[2], alpha]));
                    }
                }
            }
            return;
        }

        let scan_pad = half_t + 1.5;
        if vx.abs() >= vy.abs() {
            for px in min_x..=max_x {
                let t_val = ((px - x1) as f32 / vx).clamp(0.0, 1.0);
                let cy = y1 as f32 + t_val * vy;
                let py_start = ((cy - scan_pad).floor() as i32).max(min_y);
                let py_end = ((cy + scan_pad).ceil() as i32).min(max_y);
                for py in py_start..=py_end {
                    let wx = (px - x1) as f32;
                    let wy = (py - y1) as f32;
                    let proj = (wx * vx + wy * vy) / len_sq;
                    let t_clamp = proj.clamp(0.0, 1.0);
                    let close_x = x1 as f32 + t_clamp * vx;
                    let close_y = y1 as f32 + t_clamp * vy;
                    let dist =
                        ((px as f32 - close_x).powi(2) + (py as f32 - close_y).powi(2)).sqrt();

                    if dist <= half_t {
                        put_pixel_safe(img, px, py, color);
                    } else if dist < half_t + 1.0 {
                        let alpha = ((color[3] as f32) * (1.0 - (dist - half_t))).round() as u8;
                        put_pixel_safe(img, px, py, Rgba([color[0], color[1], color[2], alpha]));
                    }
                }
            }
        } else {
            for py in min_y..=max_y {
                let t_val = ((py - y1) as f32 / vy).clamp(0.0, 1.0);
                let cx = x1 as f32 + t_val * vx;
                let px_start = ((cx - scan_pad).floor() as i32).max(min_x);
                let px_end = ((cx + scan_pad).ceil() as i32).min(max_x);
                for px in px_start..=px_end {
                    let wx = (px - x1) as f32;
                    let wy = (py - y1) as f32;
                    let proj = (wx * vx + wy * vy) / len_sq;
                    let t_clamp = proj.clamp(0.0, 1.0);
                    let close_x = x1 as f32 + t_clamp * vx;
                    let close_y = y1 as f32 + t_clamp * vy;
                    let dist =
                        ((px as f32 - close_x).powi(2) + (py as f32 - close_y).powi(2)).sqrt();

                    if dist <= half_t {
                        put_pixel_safe(img, px, py, color);
                    } else if dist < half_t + 1.0 {
                        let alpha = ((color[3] as f32) * (1.0 - (dist - half_t))).round() as u8;
                        put_pixel_safe(img, px, py, Rgba([color[0], color[1], color[2], alpha]));
                    }
                }
            }
        }
    }

    /// Generate Kitty graphics escape sequence for direct transmission
    pub fn generate_kitty_escape_sequence(
        img: &ImageBuffer<Rgba<u8>, Vec<u8>>,
    ) -> Result<String, anyhow::Error> {
        let mut png_bytes = Vec::new();
        let mut cursor = std::io::Cursor::new(&mut png_bytes);
        img.write_to(&mut cursor, image::ImageFormat::Png)?;

        let b64 = BASE64.encode(&png_bytes);
        // Format: \x1b_Gf=100,a=T,m=0;{base64}\x1b\\
        Ok(format!("\x1b_Gf=100,a=T,m=0;{}\x1b\\", b64))
    }

    /// Generate Kitty graphics escape sequence placed at specific terminal cell coordinates
    pub fn generate_kitty_escape_sequence_placed(
        img: &ImageBuffer<Rgba<u8>, Vec<u8>>,
        col: u16,
        row: u16,
        width_cols: u16,
        height_rows: u16,
    ) -> Result<String, anyhow::Error> {
        let mut png_bytes = Vec::new();
        let mut cursor = std::io::Cursor::new(&mut png_bytes);
        img.write_to(&mut cursor, image::ImageFormat::Png)?;

        let b64 = BASE64.encode(&png_bytes);
        // Format with placement parameters: a=T (transmit and display), f=100 (PNG), c/r (cells), X/Y (pixel offset), m=0
        Ok(format!(
            "\x1b_Ga=T,f=100,c={},r={},X={},Y={},m=0;{}\x1b\\",
            width_cols, height_rows, col, row, b64
        ))
    }

    /// Generate chunked Kitty escape sequences if over transmission size limit (4096 bytes per chunk)
    pub fn generate_kitty_escape_sequence_chunked(
        img: &ImageBuffer<Rgba<u8>, Vec<u8>>,
        chunk_size: usize,
    ) -> Result<Vec<String>, anyhow::Error> {
        let mut png_bytes = Vec::new();
        let mut cursor = std::io::Cursor::new(&mut png_bytes);
        img.write_to(&mut cursor, image::ImageFormat::Png)?;

        let b64 = BASE64.encode(&png_bytes);
        let mut chunks = Vec::new();
        let chunk_len = chunk_size.clamp(512, 4096);
        let b64_bytes = b64.as_bytes();
        let total = b64_bytes.len();
        let mut offset = 0;

        while offset < total {
            let end = (offset + chunk_len).min(total);
            let slice = std::str::from_utf8(&b64_bytes[offset..end])?;
            let is_last = end == total;
            let m_val = if is_last { 0 } else { 1 };

            if offset == 0 {
                chunks.push(format!("\x1b_Gf=100,a=T,m={};{}\x1b\\", m_val, slice));
            } else {
                chunks.push(format!("\x1b_Gm={};{}\x1b\\", m_val, slice));
            }
            offset = end;
        }

        Ok(chunks)
    }

    /// Transmit Kitty graphics image directly to a writer (e.g. stdout)
    pub fn transmit_kitty_image<W: std::io::Write>(
        writer: &mut W,
        img: &ImageBuffer<Rgba<u8>, Vec<u8>>,
    ) -> Result<(), anyhow::Error> {
        let seq = Self::generate_kitty_escape_sequence(img)?;
        writer.write_all(seq.as_bytes())?;
        writer.flush()?;
        Ok(())
    }

    /// Render high-resolution canvas using ratatui-image protocol widget
    pub fn render_ratatui_image(
        canvas: &CanvasState,
        area: Rect,
        buf: &mut Buffer,
        theme: &Theme,
        use_kitty: bool,
    ) -> Result<(), anyhow::Error> {
        if area.width < 4 || area.height < 4 {
            return Ok(());
        }

        let width_px = (area.width as u32 * 2).clamp(80, 800);
        let height_px = (area.height as u32 * 4).clamp(80, 800);
        let img = Self::render_image(canvas, width_px, height_px, theme);
        let dyn_img = image::DynamicImage::ImageRgba8(img);

        let mut picker = Picker::halfblocks();
        if use_kitty {
            picker.set_protocol_type(ProtocolType::Kitty);
        } else {
            picker.set_protocol_type(ProtocolType::Halfblocks);
        }

        let proto = picker
            .new_protocol(
                dyn_img,
                ratatui::layout::Size::new(area.width, area.height),
                Resize::Fit(None),
            )
            .map_err(|e| anyhow::anyhow!("ratatui-image protocol error: {:?}", e))?;

        Image::new(&proto).allow_clipping(true).render(area, buf);
        Ok(())
    }

    /// Dispatch rendering: uses high-resolution buffer rasterization with half-block TrueColor glyphs.
    /// This eliminates terminal pipe buffer saturation, avoiding stdout lockup in interactive TTYs.
    pub fn render_with_protocol(
        canvas: &CanvasState,
        area: Rect,
        buf: &mut Buffer,
        theme: &Theme,
        _protocol: GraphicsProtocol,
    ) {
        Self::render_to_buffer(canvas, area, buf, theme);
    }

    /// Render high-resolution raster canvas into Ratatui buffer using half-blocks and enhanced iconography
    pub fn render_to_buffer(canvas: &CanvasState, area: Rect, buf: &mut Buffer, theme: &Theme) {
        if area.width < 4 || area.height < 4 {
            return;
        }

        let width_px = area.width as u32;
        let height_px = (area.height as u32) * 2;

        let mut img: ImageBuffer<Rgba<u8>, Vec<u8>> =
            ImageBuffer::from_pixel(width_px, height_px, Rgba([26, 27, 38, 255])); // #1a1b26 background

        // 1. Grid dots
        let grid_spacing = 4.0;
        let start_canvas = canvas.screen_to_canvas(0, 0);
        let end_canvas = canvas.screen_to_canvas(area.width, area.height);

        if start_canvas.0.is_finite()
            && start_canvas.1.is_finite()
            && end_canvas.0.is_finite()
            && end_canvas.1.is_finite()
        {
            let span_x = (end_canvas.0 - start_canvas.0).abs();
            let span_y = (end_canvas.1 - start_canvas.1).abs();
            if span_x < 2000.0 && span_y < 2000.0 {
                let mut cy = (start_canvas.1 / grid_spacing).floor() * grid_spacing;
                let mut loop_y = 0;
                while cy <= end_canvas.1 && loop_y < 1000 {
                    loop_y += 1;
                    let mut cx = (start_canvas.0 / grid_spacing).floor() * grid_spacing;
                    let mut loop_x = 0;
                    while cx <= end_canvas.0 && loop_x < 1000 {
                        loop_x += 1;
                        let px = (cx * canvas.zoom + canvas.offset_x).round() as i32;
                        let py = ((cy * canvas.zoom + canvas.offset_y) * 2.0).round() as i32;
                        if px >= 0 && px < width_px as i32 && py >= 0 && py < height_px as i32 {
                            img.put_pixel(px as u32, py as u32, Rgba([41, 46, 66, 255]));
                        }
                        cx += grid_spacing;
                    }
                    cy += grid_spacing;
                }
            }
        }

        // 2. Links (Straight lines in subpixels)
        for (i, link) in canvas.links.iter().enumerate() {
            let is_selected = canvas.selected_link == Some(i);
            let link_color = if is_selected {
                Rgba([224, 175, 104, 255])
            } else {
                Rgba([122, 162, 247, 255])
            };

            for win in link.waypoints.windows(2) {
                let (p1, p2) = (win[0], win[1]);
                let x1 = (p1.0 * canvas.zoom + canvas.offset_x).round() as i32;
                let y1 = ((p1.1 * canvas.zoom + canvas.offset_y) * 2.0).round() as i32;
                let x2 = (p2.0 * canvas.zoom + canvas.offset_x).round() as i32;
                let y2 = ((p2.1 * canvas.zoom + canvas.offset_y) * 2.0).round() as i32;
                draw_straight_line(&mut img, x1, y1, x2, y2, link_color, 1, false);
            }
        }

        // 3. Active wiring preview line in raster image
        if canvas.mode == CanvasMode::Wiring {
            if let Some((ref src_name, ref src_port)) = canvas.wiring_source {
                if let Some(node) = canvas.nodes.get(src_name) {
                    let ((px, py), _) = if let Some(port) = node.get_port(src_port) {
                        ((port.x, port.y), port.direction)
                    } else {
                        node.perimeter_connection_towards(
                            canvas.wiring_cursor.0,
                            canvas.wiring_cursor.1,
                        )
                    };
                    let x1 = (px * canvas.zoom + canvas.offset_x).round() as i32;
                    let y1 = ((py * canvas.zoom + canvas.offset_y) * 2.0).round() as i32;
                    let x2 =
                        (canvas.wiring_cursor.0 * canvas.zoom + canvas.offset_x).round() as i32;
                    let y2 = ((canvas.wiring_cursor.1 * canvas.zoom + canvas.offset_y) * 2.0)
                        .round() as i32;
                    let wire_color = Rgba([224, 175, 104, 255]);
                    match canvas.routing_style {
                        crate::canvas::link::RoutingStyle::Direct => {
                            draw_straight_line(&mut img, x1, y1, x2, y2, wire_color, 1, false);
                        }
                        crate::canvas::link::RoutingStyle::Octilinear => {
                            let dx = x2 - x1;
                            let dy = y2 - y1;
                            if dx == 0 || dy == 0 || dx.abs() == dy.abs() {
                                draw_straight_line(&mut img, x1, y1, x2, y2, wire_color, 1, false);
                            } else if dx.abs() > dy.abs() {
                                let mid_x = x1 + dy.abs() * dx.signum();
                                draw_straight_line(
                                    &mut img, x1, y1, mid_x, y2, wire_color, 1, false,
                                );
                                draw_straight_line(
                                    &mut img, mid_x, y2, x2, y2, wire_color, 1, false,
                                );
                            } else {
                                let mid_y = y1 + dx.abs() * dy.signum();
                                draw_straight_line(
                                    &mut img, x1, y1, x2, mid_y, wire_color, 1, false,
                                );
                                draw_straight_line(
                                    &mut img, x2, mid_y, x2, y2, wire_color, 1, false,
                                );
                            }
                        }
                        crate::canvas::link::RoutingStyle::Orthogonal => {
                            draw_straight_line(&mut img, x1, y1, x2, y1, wire_color, 1, false);
                            draw_straight_line(&mut img, x2, y1, x2, y2, wire_color, 1, false);
                        }
                    }

                    // Glowing cursor point
                    for dy in -1..=1 {
                        for dx in -1..=1 {
                            let px = x2 + dx;
                            let py = y2 + dy;
                            if px >= 0 && px < width_px as i32 && py >= 0 && py < height_px as i32 {
                                img.put_pixel(px as u32, py as u32, Rgba([255, 215, 0, 255]));
                            }
                        }
                    }
                }
            }
        }

        // 4. Nodes (filled cards with category borders and in-card subpixel iconography)
        for (name, node) in &canvas.nodes {
            let is_sel = canvas.selected_nodes.contains(name)
                || canvas.selected_node.as_deref() == Some(name.as_str());
            let (sx, sy) = canvas.canvas_to_screen(node.x, node.y);
            let w = (node.width * canvas.zoom).round() as i32;
            let h = (node.height * canvas.zoom).round() as i32;

            let nx = sx;
            let ny = sy * 2;
            let nw = w;
            let nh = h * 2;

            let border_color = if is_sel {
                Rgba([224, 175, 104, 255])
            } else {
                match node.category {
                    crate::clab::model::NodeCategory::Router => Rgba([125, 207, 255, 255]),
                    crate::clab::model::NodeCategory::Switch => Rgba([115, 218, 202, 255]),
                    crate::clab::model::NodeCategory::Host => Rgba([187, 154, 247, 255]),
                    crate::clab::model::NodeCategory::Firewall => Rgba([247, 118, 142, 255]),
                }
            };
            let fill_color = if is_sel {
                Rgba([40, 48, 70, 255])
            } else {
                Rgba([30, 34, 50, 255])
            };

            // Card body & border
            for py in ny..(ny + nh) {
                for px in nx..(nx + nw) {
                    if px >= 0 && px < width_px as i32 && py >= 0 && py < height_px as i32 {
                        let is_border =
                            px == nx || px == nx + nw - 1 || py == ny || py == ny + nh - 1;
                        let col = if is_border { border_color } else { fill_color };
                        img.put_pixel(px as u32, py as u32, col);
                    }
                }
            }

            // In-card subpixel iconography drawn on the left side
            if nw >= 14 && nh >= 6 {
                let icon_cx = nx + 4;
                let icon_cy = ny + nh / 2;
                match node.category {
                    crate::clab::model::NodeCategory::Router => {
                        draw_subpixel_router(&mut img, icon_cx, icon_cy, border_color);
                    }
                    crate::clab::model::NodeCategory::Switch => {
                        draw_subpixel_switch(&mut img, icon_cx, icon_cy, border_color);
                    }
                    crate::clab::model::NodeCategory::Firewall => {
                        draw_subpixel_firewall(&mut img, icon_cx, icon_cy, border_color);
                    }
                    crate::clab::model::NodeCategory::Host => {
                        draw_subpixel_host(&mut img, icon_cx, icon_cy, border_color);
                    }
                }
            }
        }

        // 5. Blit raster image into buffer using half-blocks ('▀')
        for row in 0..area.height {
            for col in 0..area.width {
                let top = img.get_pixel(col as u32, (row as u32) * 2);
                let bot = img.get_pixel(col as u32, (row as u32) * 2 + 1);
                let bx = area.x + col;
                let by = area.y + row;
                buf[(bx, by)].set_char('▀').set_style(
                    Style::default()
                        .fg(Color::Rgb(top[0], top[1], top[2]))
                        .bg(Color::Rgb(bot[0], bot[1], bot[2])),
                );
            }
        }

        // 6. Overlaid Straight Line Connectors & Interface Pill Badges on Links
        for (i, link) in canvas.links.iter().enumerate() {
            let is_selected = canvas.selected_link == Some(i);
            let link_fg = if is_selected {
                theme.link_selected
            } else {
                theme.link_color
            };
            let link_style = Style::default()
                .fg(link_fg)
                .bg(Color::Rgb(26, 27, 38))
                .add_modifier(if is_selected {
                    Modifier::BOLD
                } else {
                    Modifier::empty()
                });

            // Render each straight/diagonal line segment
            for win in link.waypoints.windows(2) {
                let (p1, p2) = (win[0], win[1]);
                let (sx1, sy1) = canvas.canvas_to_screen(p1.0, p1.1);
                let (sx2, sy2) = canvas.canvas_to_screen(p2.0, p2.1);
                CanvasRenderer::draw_line_segment(buf, area, sx1, sy1, sx2, sy2, link_style);
            }

            // Corner glyphs at waypoints
            if link.waypoints.len() >= 3 {
                for j in 1..link.waypoints.len() - 1 {
                    let prev = link.waypoints[j - 1];
                    let curr = link.waypoints[j];
                    let next = link.waypoints[j + 1];

                    let (sx, sy) = canvas.canvas_to_screen(curr.0, curr.1);
                    if sx >= 0 && (sx as u16) < area.width && sy >= 0 && (sy as u16) < area.height {
                        let bx = area.x + sx as u16;
                        let by = area.y + sy as u16;
                        let corner_ch =
                            OrthogonalRouter::get_glyph_at(Some(prev), curr, Some(next));
                        buf[(bx, by)].set_char(corner_ch).set_style(link_style);
                    }
                }
            }

            // Interface label pill badges along wire endpoints
            let pill_style = Style::default()
                .fg(Color::Rgb(115, 218, 202))
                .bg(Color::Rgb(26, 27, 38))
                .add_modifier(Modifier::BOLD);

            if link.waypoints.len() >= 2 {
                let Some((src_badge, tgt_badge)) = canvas.link_badge_positions(i) else {
                    continue;
                };

                // Source interface pill badge
                let badge_src = format!("[{}]", link.source_port);
                if src_badge.y >= 0 && (src_badge.y as u16) < area.height {
                    for (c_idx, ch) in badge_src.chars().enumerate() {
                        let cell_x = src_badge.x + c_idx as i32;
                        if cell_x >= 0 && (cell_x as u16) < area.width {
                            buf[(area.x + cell_x as u16, area.y + src_badge.y as u16)]
                                .set_char(ch)
                                .set_style(pill_style);
                        }
                    }
                }

                // Target interface pill badge
                let badge_tgt = format!("[{}]", link.target_port);
                if tgt_badge.y >= 0 && (tgt_badge.y as u16) < area.height {
                    for (c_idx, ch) in badge_tgt.chars().enumerate() {
                        let cell_x = tgt_badge.x + c_idx as i32;
                        if cell_x >= 0 && (cell_x as u16) < area.width {
                            buf[(area.x + cell_x as u16, area.y + tgt_badge.y as u16)]
                                .set_char(ch)
                                .set_style(pill_style);
                        }
                    }
                }
            }
        }

        // 7. Overlay Typography and Clean Node Cards (SVG iconography glyphs, pill badges, and contrast titles)
        for (name, node) in &canvas.nodes {
            let is_sel = canvas.selected_nodes.contains(name)
                || canvas.selected_node.as_deref() == Some(name.as_str());
            let (sx, sy) = canvas.canvas_to_screen(node.x, node.y);
            let w = (node.width * canvas.zoom).round() as i32;
            let h = (node.height * canvas.zoom).round() as i32;

            if sx + w < 0 || sx >= area.width as i32 || sy + h < 0 || sy >= area.height as i32 {
                continue;
            }

            let border_color = if is_sel {
                theme.accent
            } else {
                match node.category {
                    crate::clab::model::NodeCategory::Router => Color::Rgb(125, 207, 255),
                    crate::clab::model::NodeCategory::Switch => Color::Rgb(115, 218, 202),
                    crate::clab::model::NodeCategory::Host => Color::Rgb(187, 154, 247),
                    crate::clab::model::NodeCategory::Firewall => Color::Rgb(247, 118, 142),
                }
            };
            let card_bg = if is_sel {
                Color::Rgb(40, 48, 70)
            } else {
                Color::Rgb(30, 34, 50)
            };
            let border_style =
                Style::default()
                    .fg(border_color)
                    .bg(card_bg)
                    .add_modifier(if is_sel {
                        Modifier::BOLD
                    } else {
                        Modifier::empty()
                    });
            let interior_style = Style::default().bg(card_bg);

            // Draw crisp rounded card border and interior fill
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

                    let st = if ch == ' ' {
                        interior_style
                    } else {
                        border_style
                    };
                    buf[(bx, by)].set_char(ch).set_style(st);
                }
            }

            // Vector iconography glyph for header
            let icon_str = match node.category {
                crate::clab::model::NodeCategory::Router => "⨁", // Multidirectional router circular emblem
                crate::clab::model::NodeCategory::Switch => "⇆", // Opposing horizontal switching arrows
                crate::clab::model::NodeCategory::Host => "🖳",   // Stack chassis / host terminal
                crate::clab::model::NodeCategory::Firewall => "🛡", // Defensive security shield
            };

            // Header line inside node: Pill badge with Icon + Name
            let header_y = sy + 1;
            if header_y >= 0 && header_y < area.height as i32 {
                let by = area.y + header_y as u16;
                let title = format!("{} {}", icon_str, name);
                let title_style = Style::default()
                    .fg(if is_sel {
                        theme.accent
                    } else {
                        Color::Rgb(240, 240, 255)
                    })
                    .bg(card_bg)
                    .add_modifier(Modifier::BOLD);
                let start_x = sx + 2;

                for (idx, ch) in title.chars().enumerate() {
                    let cx = start_x + idx as i32;
                    if cx > sx && cx < sx + w - 1 && cx >= 0 && cx < area.width as i32 {
                        let bx = area.x + cx as u16;
                        buf[(bx, by)].set_char(ch).set_style(title_style);
                    }
                }
            }

            // Subtitle inside node: Kind pill badge (centered horizontally)
            let sub_y = sy + 2;
            if sub_y >= 0 && sub_y < area.height as i32 && h >= 4 {
                let by = area.y + sub_y as u16;
                let sub = format!("[{}]", node.kind);
                let sub_style = Style::default()
                    .fg(Color::Rgb(169, 177, 214))
                    .bg(Color::Rgb(26, 27, 38))
                    .add_modifier(Modifier::BOLD);
                let start_x = sx + ((w - sub.len() as i32) / 2).max(1);

                for (idx, ch) in sub.chars().enumerate() {
                    let cx = start_x + idx as i32;
                    if cx > sx && cx < sx + w - 1 && cx >= 0 && cx < area.width as i32 {
                        let bx = area.x + cx as u16;
                        buf[(bx, by)].set_char(ch).set_style(sub_style);
                    }
                }
            }
        }

        // 8. Wiring preview endpoint marker
        if canvas.mode == CanvasMode::Wiring {
            let (cx_s, cy_s) =
                canvas.canvas_to_screen(canvas.wiring_cursor.0, canvas.wiring_cursor.1);
            if cx_s >= 0 && (cx_s as u16) < area.width && cy_s >= 0 && (cy_s as u16) < area.height {
                let bx = area.x + cx_s as u16;
                let by = area.y + cy_s as u16;
                buf[(bx, by)].set_char('●').set_style(
                    Style::default()
                        .fg(theme.accent)
                        .add_modifier(Modifier::BOLD),
                );
            }
        }

        // 8b. Marquee box selection if active
        if canvas.selection_box.is_some() {
            CanvasRenderer::render_marquee_box(canvas, area, buf, theme);
        }

        // 9. Viewport status overlay
        CanvasRenderer::render_overlay(canvas, area, buf, theme);
    }
}
