use crate::renderer::color::parse_hex_color;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PaletteColor {
    pub name: &'static str,
    pub hex: &'static str,
    pub u32_val: u32,
}

pub const WORKSPACE_ACCENT_PALETTE: &[PaletteColor] = &[
    PaletteColor { name: "Tokyo Blue",     hex: "#7aa2f7", u32_val: 0x7aa2f7 },
    PaletteColor { name: "Tokyo Cyan",     hex: "#7dcfff", u32_val: 0x7dcfff },
    PaletteColor { name: "Tokyo Sky",      hex: "#89ddff", u32_val: 0x89ddff },
    PaletteColor { name: "Tokyo Teal",     hex: "#73daca", u32_val: 0x73daca },
    PaletteColor { name: "Tokyo Mint",     hex: "#41a6b5", u32_val: 0x41a6b5 },
    PaletteColor { name: "Tokyo Green",    hex: "#9ece6a", u32_val: 0x9ece6a },
    PaletteColor { name: "Tokyo Emerald",  hex: "#73ca92", u32_val: 0x73ca92 },
    PaletteColor { name: "Tokyo Lime",     hex: "#b9f27c", u32_val: 0xb9f27c },
    PaletteColor { name: "Tokyo Yellow",   hex: "#e0af68", u32_val: 0xe0af68 },
    PaletteColor { name: "Tokyo Amber",    hex: "#ffc777", u32_val: 0xffc777 },
    PaletteColor { name: "Tokyo Orange",   hex: "#ff9e64", u32_val: 0xff9e64 },
    PaletteColor { name: "Tokyo Coral",    hex: "#ff757f", u32_val: 0xff757f },
    PaletteColor { name: "Tokyo Red",      hex: "#f7768e", u32_val: 0xf7768e },
    PaletteColor { name: "Tokyo Crimson",  hex: "#db4b4b", u32_val: 0xdb4b4b },
    PaletteColor { name: "Tokyo Pink",     hex: "#f678be", u32_val: 0xf678be },
    PaletteColor { name: "Tokyo Magenta",  hex: "#bb9af7", u32_val: 0xbb9af7 },
    PaletteColor { name: "Tokyo Purple",   hex: "#9d7cd8", u32_val: 0x9d7cd8 },
    PaletteColor { name: "Tokyo Lavender", hex: "#b4f9f8", u32_val: 0xb4f9f8 },
    PaletteColor { name: "Tokyo Indigo",   hex: "#657b83", u32_val: 0x657b83 },
    PaletteColor { name: "Tokyo Slate",    hex: "#a9b1d6", u32_val: 0xa9b1d6 },
];

pub const WORKSPACE_BACKGROUND_PALETTE: &[PaletteColor] = &[
    PaletteColor { name: "Tokyo Night",   hex: "#1a1b26", u32_val: 0x1a1b26 },
    PaletteColor { name: "Tokyo Storm",   hex: "#24283b", u32_val: 0x24283b },
    PaletteColor { name: "Deep Night",    hex: "#16161e", u32_val: 0x16161e },
    PaletteColor { name: "Abyss Dark",    hex: "#0f1017", u32_val: 0x0f1017 },
    PaletteColor { name: "Navy Midnight", hex: "#131a2a", u32_val: 0x131a2a },
    PaletteColor { name: "Cyan Midnight", hex: "#112128", u32_val: 0x112128 },
    PaletteColor { name: "Forest Night",  hex: "#14231e", u32_val: 0x14231e },
    PaletteColor { name: "Plum Midnight", hex: "#1f172a", u32_val: 0x1f172a },
    PaletteColor { name: "Charcoal Night",hex: "#1c1d27", u32_val: 0x1c1d27 },
    PaletteColor { name: "Espresso Night",hex: "#221c1a", u32_val: 0x221c1a },
];

pub fn normalize_hex(input: &str) -> Option<String> {
    let s = input.trim().trim_start_matches('#');
    if s.len() == 6 && s.chars().all(|c| c.is_ascii_hexdigit()) {
        Some(format!("#{}", s.to_lowercase()))
    } else {
        None
    }
}

pub fn hex_to_u32(hex: &str, default: u32) -> u32 {
    parse_hex_color(hex, default)
}
