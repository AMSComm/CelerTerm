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
    NewTab,
    CloseTab,
    NewWorkspace,
    PreviousWorkspace,
    NextWorkspace,
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
    // 0. Command (Logo) shortcuts for Tab and Workspace management
    if mods.logo && !mods.ctrl && !mods.alt {
        if mods.shift {
            if let Key::Character(ch) = key {
                match ch.as_str() {
                    "N" | "n" => return Some(KeyAction::NewWorkspace),
                    "{" | "[" => return Some(KeyAction::PreviousWorkspace),
                    "}" | "]" => return Some(KeyAction::NextWorkspace),
                    _ => {}
                }
            }
        } else {
            if let Key::Character(ch) = key {
                match ch.as_str() {
                    "t" | "T" => return Some(KeyAction::NewTab),
                    "w" | "W" => return Some(KeyAction::CloseTab),
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
                    _ => {}
                }
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
        // Physical key takes priority on macOS to bypass dead key transformations (e.g. Option+Q producing "œ")
        if let Some(code) = physical_key
            && let Some(ch) = key_code_to_ascii(code)
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

fn key_code_to_ascii(code: KeyCode) -> Option<char> {
    match code {
        KeyCode::KeyA => Some('a'),
        KeyCode::KeyB => Some('b'),
        KeyCode::KeyC => Some('c'),
        KeyCode::KeyD => Some('d'),
        KeyCode::KeyE => Some('e'),
        KeyCode::KeyF => Some('f'),
        KeyCode::KeyG => Some('g'),
        KeyCode::KeyH => Some('h'),
        KeyCode::KeyI => Some('i'),
        KeyCode::KeyJ => Some('j'),
        KeyCode::KeyK => Some('k'),
        KeyCode::KeyL => Some('l'),
        KeyCode::KeyM => Some('m'),
        KeyCode::KeyN => Some('n'),
        KeyCode::KeyO => Some('o'),
        KeyCode::KeyP => Some('p'),
        KeyCode::KeyQ => Some('q'),
        KeyCode::KeyR => Some('r'),
        KeyCode::KeyS => Some('s'),
        KeyCode::KeyT => Some('t'),
        KeyCode::KeyU => Some('u'),
        KeyCode::KeyV => Some('v'),
        KeyCode::KeyW => Some('w'),
        KeyCode::KeyX => Some('x'),
        KeyCode::KeyY => Some('y'),
        KeyCode::KeyZ => Some('z'),
        KeyCode::Digit0 => Some('0'),
        KeyCode::Digit1 => Some('1'),
        KeyCode::Digit2 => Some('2'),
        KeyCode::Digit3 => Some('3'),
        KeyCode::Digit4 => Some('4'),
        KeyCode::Digit5 => Some('5'),
        KeyCode::Digit6 => Some('6'),
        KeyCode::Digit7 => Some('7'),
        KeyCode::Digit8 => Some('8'),
        KeyCode::Digit9 => Some('9'),
        _ => None,
    }
}
