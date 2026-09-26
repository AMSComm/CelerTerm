use winit::keyboard::{Key, KeyCode, NamedKey};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Modifiers {
    pub alt: bool,
    pub ctrl: bool,
    pub shift: bool,
    pub logo: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KeyAction {
    Bytes(Vec<u8>),
    Text(String),
    SelectTab(usize),
    PreviousTab,
    NextTab,
    MoveTabLeft,
    MoveTabRight,
    NewTab,
    CloseTab,
    Quit,
    NewWorkspace,
    PreviousWorkspace,
    NextWorkspace,
    ToggleWorkspaceModal,
    ReloadConfig,
    CheckForUpdates,
    Paste,
    Copy,
    ClearScreen,
    IncreaseFontSize,
    DecreaseFontSize,
    ResetFontSize,
}

pub fn translate_key(key: &Key, mods: Modifiers, option_as_alt: bool) -> Option<KeyAction> {
    translate_key_event(key, None, mods, option_as_alt)
}

pub fn translate_key_event(
    key: &Key,
    physical_key: Option<KeyCode>,
    mods: Modifiers,
    option_as_alt: bool,
) -> Option<KeyAction> {
    // 0. Command (Logo) or Ctrl+Shift shortcuts for Tab and Workspace management
    if mods.logo && !mods.ctrl && !mods.alt {
        if mods.shift {
            if let Key::Character(ch) = key {
                match ch.as_str() {
                    "N" | "n" => return Some(KeyAction::NewWorkspace),
                    "{" | "[" => return Some(KeyAction::PreviousWorkspace),
                    "}" | "]" => return Some(KeyAction::NextWorkspace),
                    "P" | "p" | "O" | "o" => return Some(KeyAction::ToggleWorkspaceModal),
                    "R" | "r" => return Some(KeyAction::ReloadConfig),
                    "U" | "u" => return Some(KeyAction::CheckForUpdates),
                    _ => {}
                }
            }
        } else {
            if let Key::Character(ch) = key {
                match ch.as_str() {
                    "t" | "T" => return Some(KeyAction::NewTab),
                    "w" | "W" => return Some(KeyAction::CloseTab),
                    "q" | "Q" => return Some(KeyAction::Quit),
                    "c" | "C" => return Some(KeyAction::Copy),
                    "v" | "V" => return Some(KeyAction::Paste),
                    "k" | "K" => return Some(KeyAction::ClearScreen),
                    "=" | "+" => return Some(KeyAction::IncreaseFontSize),
                    "-" | "_" => return Some(KeyAction::DecreaseFontSize),
                    "0" => return Some(KeyAction::ResetFontSize),
                    _ => {
                        if let Ok(num) = ch.parse::<usize>()
                            && (1..=9).contains(&num)
                        {
                            return Some(KeyAction::SelectTab(num));
                        }
                    }
                }
            }

            if let Key::Named(named) = key {
                match named {
                    NamedKey::ArrowLeft => return Some(KeyAction::PreviousTab),
                    NamedKey::ArrowRight => return Some(KeyAction::NextTab),
                    NamedKey::ArrowUp => return Some(KeyAction::MoveTabLeft),
                    NamedKey::ArrowDown => return Some(KeyAction::MoveTabRight),
                    _ => {}
                }
            }
        }
    }

    // 0.1 Control + Shift combinations (cross-platform shortcuts)
    if mods.ctrl && mods.shift && !mods.alt && !mods.logo {
        if let Key::Character(ch) = key {
            match ch.as_str() {
                "P" | "p" | "O" | "o" => return Some(KeyAction::ToggleWorkspaceModal),
                "R" | "r" => return Some(KeyAction::ReloadConfig),
                "N" | "n" => return Some(KeyAction::NewWorkspace),
                "U" | "u" => return Some(KeyAction::CheckForUpdates),
                _ => {}
            }
        }

        if let Key::Named(named) = key {
            match named {
                NamedKey::ArrowUp => return Some(KeyAction::MoveTabLeft),
                NamedKey::ArrowDown => return Some(KeyAction::MoveTabRight),
                _ => {}
            }
        }
    }

    // 1. Control + Key combinations
    if mods.ctrl && !mods.alt && !mods.logo
        && let Key::Character(ch) = key
        && let Some(first_char) = ch.chars().next()
    {
        let ascii = first_char.to_ascii_lowercase();
        if ascii.is_ascii_lowercase() {
            let byte = (ascii as u8) - b'a' + 1;
            return Some(KeyAction::Bytes(vec![byte]));
        }
        match ascii {
            '@' => return Some(KeyAction::Bytes(vec![0x00])),
            '[' => return Some(KeyAction::Bytes(vec![0x1b])),
            '\\' => return Some(KeyAction::Bytes(vec![0x1c])),
            ']' => return Some(KeyAction::Bytes(vec![0x1d])),
            '^' => return Some(KeyAction::Bytes(vec![0x1e])),
            '_' => return Some(KeyAction::Bytes(vec![0x1f])),
            '?' => return Some(KeyAction::Bytes(vec![0x7f])),
            _ => {}
        }
    }

    // 2. Alt / Option combinations when option_as_alt is true
    if mods.alt && option_as_alt && !mods.ctrl && !mods.logo {
        // Physical key takes priority on macOS to bypass dead key transformations (e.g. Option+Q producing "œ", Option+E producing "´")
        if let Some(code) = physical_key
            && let Some(ch) = key_code_to_ascii(code, mods.shift)
        {
            return Some(KeyAction::Bytes(vec![0x1b, ch as u8]));
        }

        match key {
            Key::Character(s) => {
                let mut bytes = vec![0x1b];
                bytes.extend_from_slice(s.as_bytes());
                return Some(KeyAction::Bytes(bytes));
            }
            Key::Named(NamedKey::Backspace) => {
                return Some(KeyAction::Bytes(vec![0x1b, 0x7f]));
            }
            _ => {}
        }
    }

    // 3. Named keys (Arrows, Enter, Backspace, Space, etc.)
    if let Key::Named(named) = key {
        match named {
            NamedKey::Space => {
                return Some(KeyAction::Bytes(vec![b' ']));
            }
            NamedKey::ArrowUp => {
                return Some(KeyAction::Bytes(b"\x1b[A".to_vec()));
            }
            NamedKey::ArrowDown => {
                return Some(KeyAction::Bytes(b"\x1b[B".to_vec()));
            }
            NamedKey::ArrowRight => {
                return Some(KeyAction::Bytes(b"\x1b[C".to_vec()));
            }
            NamedKey::ArrowLeft => {
                return Some(KeyAction::Bytes(b"\x1b[D".to_vec()));
            }
            NamedKey::Enter => {
                return Some(KeyAction::Bytes(b"\r".to_vec()));
            }
            NamedKey::Backspace => {
                return Some(KeyAction::Bytes(vec![0x7f]));
            }
            NamedKey::Tab => {
                if mods.shift {
                    return Some(KeyAction::Bytes(b"\x1b[Z".to_vec()));
                } else {
                    return Some(KeyAction::Bytes(b"\t".to_vec()));
                }
            }
            NamedKey::Escape => {
                return Some(KeyAction::Bytes(vec![0x1b]));
            }
            NamedKey::Home => {
                return Some(KeyAction::Bytes(b"\x1b[H".to_vec()));
            }
            NamedKey::End => {
                return Some(KeyAction::Bytes(b"\x1b[F".to_vec()));
            }
            NamedKey::PageUp => {
                return Some(KeyAction::Bytes(b"\x1b[5~".to_vec()));
            }
            NamedKey::PageDown => {
                return Some(KeyAction::Bytes(b"\x1b[6~".to_vec()));
            }
            NamedKey::Delete => {
                return Some(KeyAction::Bytes(b"\x1b[3~".to_vec()));
            }
            _ => {}
        }
    }

    // 4. Standard text fallback
    if let Key::Character(s) = key {
        return Some(KeyAction::Text(s.to_string()));
    }

    None
}

fn key_code_to_ascii(code: KeyCode, shift: bool) -> Option<char> {
    match code {
        KeyCode::KeyA => Some(if shift { 'A' } else { 'a' }),
        KeyCode::KeyB => Some(if shift { 'B' } else { 'b' }),
        KeyCode::KeyC => Some(if shift { 'C' } else { 'c' }),
        KeyCode::KeyD => Some(if shift { 'D' } else { 'd' }),
        KeyCode::KeyE => Some(if shift { 'E' } else { 'e' }),
        KeyCode::KeyF => Some(if shift { 'F' } else { 'f' }),
        KeyCode::KeyG => Some(if shift { 'G' } else { 'g' }),
        KeyCode::KeyH => Some(if shift { 'H' } else { 'h' }),
        KeyCode::KeyI => Some(if shift { 'I' } else { 'i' }),
        KeyCode::KeyJ => Some(if shift { 'J' } else { 'j' }),
        KeyCode::KeyK => Some(if shift { 'K' } else { 'k' }),
        KeyCode::KeyL => Some(if shift { 'L' } else { 'l' }),
        KeyCode::KeyM => Some(if shift { 'M' } else { 'm' }),
        KeyCode::KeyN => Some(if shift { 'N' } else { 'n' }),
        KeyCode::KeyO => Some(if shift { 'O' } else { 'o' }),
        KeyCode::KeyP => Some(if shift { 'P' } else { 'p' }),
        KeyCode::KeyQ => Some(if shift { 'Q' } else { 'q' }),
        KeyCode::KeyR => Some(if shift { 'R' } else { 'r' }),
        KeyCode::KeyS => Some(if shift { 'S' } else { 's' }),
        KeyCode::KeyT => Some(if shift { 'T' } else { 't' }),
        KeyCode::KeyU => Some(if shift { 'U' } else { 'u' }),
        KeyCode::KeyV => Some(if shift { 'V' } else { 'v' }),
        KeyCode::KeyW => Some(if shift { 'W' } else { 'w' }),
        KeyCode::KeyX => Some(if shift { 'X' } else { 'x' }),
        KeyCode::KeyY => Some(if shift { 'Y' } else { 'y' }),
        KeyCode::KeyZ => Some(if shift { 'Z' } else { 'z' }),
        KeyCode::Digit0 => Some(if shift { ')' } else { '0' }),
        KeyCode::Digit1 => Some(if shift { '!' } else { '1' }),
        KeyCode::Digit2 => Some(if shift { '@' } else { '2' }),
        KeyCode::Digit3 => Some(if shift { '#' } else { '3' }),
        KeyCode::Digit4 => Some(if shift { '$' } else { '4' }),
        KeyCode::Digit5 => Some(if shift { '%' } else { '5' }),
        KeyCode::Digit6 => Some(if shift { '^' } else { '6' }),
        KeyCode::Digit7 => Some(if shift { '&' } else { '7' }),
        KeyCode::Digit8 => Some(if shift { '*' } else { '8' }),
        KeyCode::Digit9 => Some(if shift { '(' } else { '9' }),
        KeyCode::Minus => Some(if shift { '_' } else { '-' }),
        KeyCode::Equal => Some(if shift { '+' } else { '=' }),
        KeyCode::BracketLeft => Some(if shift { '{' } else { '[' }),
        KeyCode::BracketRight => Some(if shift { '}' } else { ']' }),
        KeyCode::Semicolon => Some(if shift { ':' } else { ';' }),
        KeyCode::Quote => Some(if shift { '"' } else { '\'' }),
        KeyCode::Comma => Some(if shift { '<' } else { ',' }),
        KeyCode::Period => Some(if shift { '>' } else { '.' }),
        KeyCode::Slash => Some(if shift { '?' } else { '/' }),
        KeyCode::Backslash => Some(if shift { '|' } else { '\\' }),
        KeyCode::Backquote => Some(if shift { '~' } else { '`' }),
        _ => None,
    }
}
