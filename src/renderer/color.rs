use alacritty_terminal::vte::ansi::{Color, NamedColor};

pub fn resolve_color(color: Color, default_fg: u32, default_bg: u32) -> u32 {
    match color {
        Color::Spec(rgb) => {
            ((rgb.r as u32) << 16) | ((rgb.g as u32) << 8) | (rgb.b as u32)
        }
        Color::Indexed(idx) => indexed_to_rgb(idx),
        Color::Named(named) => named_to_rgb(named, default_fg, default_bg),
    }
}

fn named_to_rgb(named: NamedColor, default_fg: u32, default_bg: u32) -> u32 {
    match named {
        // Tokyo Night standard ANSI colors
        NamedColor::Black => 0x15161e,
        NamedColor::Red => 0xf7768e,
        NamedColor::Green => 0x9ece6a,
        NamedColor::Yellow => 0xe0af68,
        NamedColor::Blue => 0x7aa2f7,
        NamedColor::Magenta => 0xbb9af7,
        NamedColor::Cyan => 0x7dcfff,
        NamedColor::White => 0xa9b1d6,

        // Bright ANSI colors
        NamedColor::BrightBlack => 0x414868,
        NamedColor::BrightRed => 0xf7768e,
        NamedColor::BrightGreen => 0x9ece6a,
        NamedColor::BrightYellow => 0xe0af68,
        NamedColor::BrightBlue => 0x7aa2f7,
        NamedColor::BrightMagenta => 0xbb9af7,
        NamedColor::BrightCyan => 0x7dcfff,
        NamedColor::BrightWhite => 0xc0caf5,

        // Dim ANSI colors
        NamedColor::DimBlack => 0x0e0f15,
        NamedColor::DimRed => 0xa54e5f,
        NamedColor::DimGreen => 0x698a47,
        NamedColor::DimYellow => 0x967545,
        NamedColor::DimBlue => 0x516ca5,
        NamedColor::DimMagenta => 0x7d67a5,
        NamedColor::DimCyan => 0x538aa8,
        NamedColor::DimWhite => 0x70768e,

        NamedColor::Foreground => default_fg,
        NamedColor::Background => default_bg,
        NamedColor::Cursor => 0x7aa2f7,
        NamedColor::DimForeground => 0x565f89,
        NamedColor::BrightForeground => 0xc0caf5,
    }
}

fn indexed_to_rgb(idx: u8) -> u32 {
    match idx {
        // Standard 0..15 ANSI
        0 => 0x15161e,
        1 => 0xf7768e,
        2 => 0x9ece6a,
        3 => 0xe0af68,
        4 => 0x7aa2f7,
        5 => 0xbb9af7,
        6 => 0x7dcfff,
        7 => 0xa9b1d6,
        8 => 0x414868,
        9 => 0xf7768e,
        10 => 0x9ece6a,
        11 => 0xe0af68,
        12 => 0x7aa2f7,
        13 => 0xbb9af7,
        14 => 0x7dcfff,
        15 => 0xc0caf5,

        // 16..231: 6x6x6 color cube
        16..=231 => {
            let i = idx - 16;
            let r = (i / 36) % 6;
            let g = (i / 6) % 6;
            let b = i % 6;
            let to_val = |v: u8| if v == 0 { 0 } else { 55 + v * 40 };
            ((to_val(r) as u32) << 16) | ((to_val(g) as u32) << 8) | (to_val(b) as u32)
        }

        // 232..255: grayscale ramp
        232..=255 => {
            let gray = 8 + (idx - 232) * 10;
            ((gray as u32) << 16) | ((gray as u32) << 8) | (gray as u32)
        }
    }
}
