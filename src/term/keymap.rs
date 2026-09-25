use winit::keyboard::{Key, NamedKey};

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
}

pub fn translate_key(key: &Key, mods: Modifiers, option_as_alt: bool) -> Option<KeyAction> {
    // 0. Command (Logo) shortcuts for Tab management
    if mods.logo && !mods.ctrl && !mods.alt {
        if let Key::Character(ch) = key
            && let Ok(num) = ch.parse::<usize>()
            && (1..=9).contains(&num)
        {
            return Some(KeyAction::SelectTab(num));
        }

        if let Key::Named(named) = key {
            match named {
                NamedKey::ArrowLeft => return Some(KeyAction::PreviousTab),
                NamedKey::ArrowRight => return Some(KeyAction::NextTab),
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

    // 3. Named keys (Arrows, Enter, Backspace, etc.)
    if let Key::Named(named) = key {
        match named {
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
