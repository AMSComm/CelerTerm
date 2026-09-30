use std::collections::HashMap;
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
    CycleNextWindow,
    ToggleWorkspaceModal,
    CustomizeTabColor,
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
    translate_key_event_full(key, physical_key, mods, option_as_alt, false, None)
}

pub fn translate_key_event_full(
    key: &Key,
    physical_key: Option<KeyCode>,
    mods: Modifiers,
    option_as_alt: bool,
    app_cursor: bool,
    custom_bindings: Option<&HashMap<String, String>>,
) -> Option<KeyAction> {
    // -0.5 Custom Keybinding overrides from configuration (Highest priority)
    if let Some(bindings) = custom_bindings {
        for (spec, action_str) in bindings {
            if matches_key_spec(spec, key, physical_key, mods)
                && let Some(action) = parse_action_string(action_str, app_cursor)
            {
                return Some(action);
            }
        }
    }

    // 0. Command (Logo) or Ctrl+Shift shortcuts for Tab and Workspace management
    if mods.logo && !mods.ctrl && !mods.alt {
        if physical_key == Some(KeyCode::Backquote) {
            return Some(KeyAction::CycleNextWindow);
        }

        if mods.shift {
            if let Key::Character(ch) = key {
                match ch.as_str() {
                    "~" | "`" => return Some(KeyAction::CycleNextWindow),
                    "N" | "n" => return Some(KeyAction::NewWorkspace),
                    "{" | "[" => return Some(KeyAction::PreviousWorkspace),
                    "}" | "]" => return Some(KeyAction::NextWorkspace),
                    "P" | "p" | "O" | "o" => return Some(KeyAction::ToggleWorkspaceModal),
                    "T" | "t" | "K" | "k" => return Some(KeyAction::CustomizeTabColor),
                    "R" | "r" => return Some(KeyAction::ReloadConfig),
                    "U" | "u" => return Some(KeyAction::CheckForUpdates),
                    _ => {}
                }
            }
        } else {
            if let Key::Character(ch) = key {
                match ch.as_str() {
                    "`" | "~" => return Some(KeyAction::CycleNextWindow),
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

    // 0.05 Dedicated Escape key handling (handles NamedKey, KeyCode::Escape, and raw ESC character \x1b)
    let is_escape = matches!(key, Key::Named(NamedKey::Escape))
        || matches!(key, Key::Character(s) if s == "\x1b")
        || physical_key == Some(KeyCode::Escape);

    if is_escape && !mods.logo {
        if mods.alt {
            return Some(KeyAction::Bytes(vec![0x1b, 0x1b]));
        } else {
            return Some(KeyAction::Bytes(vec![0x1b]));
        }
    }

    // 0.1 Control + Shift combinations (cross-platform shortcuts)
    if mods.ctrl && mods.shift && !mods.alt && !mods.logo {
        if let Key::Character(ch) = key {
            match ch.as_str() {
                "P" | "p" | "O" | "o" => return Some(KeyAction::ToggleWorkspaceModal),
                "T" | "t" | "K" | "k" => return Some(KeyAction::CustomizeTabColor),
                "R" | "r" => return Some(KeyAction::ReloadConfig),
                "N" | "n" => return Some(KeyAction::NewWorkspace),
                "U" | "u" => return Some(KeyAction::CheckForUpdates),
                "C" | "c" => return Some(KeyAction::Copy),
                "V" | "v" => return Some(KeyAction::Paste),
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
        if let Some(code) = physical_key {
            match code {
                KeyCode::ArrowUp => return Some(KeyAction::MoveTabLeft),
                KeyCode::ArrowDown => return Some(KeyAction::MoveTabRight),
                _ => {}
            }
        }
    }

    // 1. Control + Key combinations (including Neovim Ctrl+6 / Ctrl+^ alternate buffer)
    if mods.ctrl && !mods.alt && !mods.logo {
        // Physical key check takes priority for special control characters
        if let Some(code) = physical_key {
            match code {
                KeyCode::Digit2 => return Some(KeyAction::Bytes(vec![0x00])), // Ctrl+2 -> NUL
                KeyCode::Digit3 => return Some(KeyAction::Bytes(vec![0x1b])), // Ctrl+3 -> ESC
                KeyCode::Digit4 => return Some(KeyAction::Bytes(vec![0x1c])), // Ctrl+4 -> FS
                KeyCode::Digit5 => return Some(KeyAction::Bytes(vec![0x1d])), // Ctrl+5 -> GS
                KeyCode::Digit6 => return Some(KeyAction::Bytes(vec![0x1e])), // Ctrl+6 -> RS (Neovim alternate buffer <C-^>)
                KeyCode::Digit7 => return Some(KeyAction::Bytes(vec![0x1f])), // Ctrl+7 -> US
                KeyCode::Digit8 => return Some(KeyAction::Bytes(vec![0x7f])), // Ctrl+8 -> DEL
                KeyCode::Space => return Some(KeyAction::Bytes(vec![0x00])),  // Ctrl+Space -> NUL
                KeyCode::Slash => return Some(KeyAction::Bytes(vec![0x1f])),  // Ctrl+/ -> US
                _ => {}
            }
        }

        if let Key::Character(ch) = key
            && let Some(first_char) = ch.chars().next()
        {
            let ascii = first_char.to_ascii_lowercase();
            if ascii.is_ascii_lowercase() {
                let byte = (ascii as u8) - b'a' + 1;
                return Some(KeyAction::Bytes(vec![byte]));
            }

            match ascii {
                '2' | '@' => return Some(KeyAction::Bytes(vec![0x00])),
                '3' | '[' => return Some(KeyAction::Bytes(vec![0x1b])),
                '4' | '\\' => return Some(KeyAction::Bytes(vec![0x1c])),
                '5' | ']' => return Some(KeyAction::Bytes(vec![0x1d])),
                '6' | '^' => return Some(KeyAction::Bytes(vec![0x1e])), // Ctrl+6 / Ctrl+^ -> RS
                '7' | '_' | '/' => return Some(KeyAction::Bytes(vec![0x1f])),
                '8' | '?' => return Some(KeyAction::Bytes(vec![0x7f])),
                ' ' => return Some(KeyAction::Bytes(vec![0x00])),
                _ => {}
            }
        }

        if matches!(key, Key::Named(NamedKey::Space)) {
            return Some(KeyAction::Bytes(vec![0x00]));
        }
    }

    // 1.5 Arrow keys with Ctrl / Shift modifiers (Word jumps and selection)
    let is_arrow_up = matches!(key, Key::Named(NamedKey::ArrowUp)) || physical_key == Some(KeyCode::ArrowUp);
    let is_arrow_down = matches!(key, Key::Named(NamedKey::ArrowDown)) || physical_key == Some(KeyCode::ArrowDown);
    let is_arrow_right = matches!(key, Key::Named(NamedKey::ArrowRight)) || physical_key == Some(KeyCode::ArrowRight);
    let is_arrow_left = matches!(key, Key::Named(NamedKey::ArrowLeft)) || physical_key == Some(KeyCode::ArrowLeft);

    if !mods.logo {
        if mods.ctrl && mods.shift {
            if is_arrow_right {
                return Some(KeyAction::Bytes(b"\x1b[1;6C".to_vec()));
            }
            if is_arrow_left {
                return Some(KeyAction::Bytes(b"\x1b[1;6D".to_vec()));
            }
        } else if mods.ctrl && !mods.alt {
            if is_arrow_up {
                return Some(KeyAction::Bytes(b"\x1b[1;5A".to_vec()));
            }
            if is_arrow_down {
                return Some(KeyAction::Bytes(b"\x1b[1;5B".to_vec()));
            }
            if is_arrow_right {
                return Some(KeyAction::Bytes(b"\x1b[1;5C".to_vec()));
            }
            if is_arrow_left {
                return Some(KeyAction::Bytes(b"\x1b[1;5D".to_vec()));
            }
        } else if mods.shift && !mods.alt && !mods.ctrl {
            if is_arrow_up {
                return Some(KeyAction::Bytes(b"\x1b[1;2A".to_vec()));
            }
            if is_arrow_down {
                return Some(KeyAction::Bytes(b"\x1b[1;2B".to_vec()));
            }
            if is_arrow_right {
                return Some(KeyAction::Bytes(b"\x1b[1;2C".to_vec()));
            }
            if is_arrow_left {
                return Some(KeyAction::Bytes(b"\x1b[1;2D".to_vec()));
            }
        }
    }

    // 2. Alt / Option combinations
    if mods.alt && !mods.ctrl && !mods.logo {
        // Physical key takes priority on macOS to bypass dead key transformations
        if let Some(code) = physical_key {
            match code {
                KeyCode::ArrowLeft => return Some(KeyAction::Bytes(b"\x1bb".to_vec())),
                KeyCode::ArrowRight => return Some(KeyAction::Bytes(b"\x1bf".to_vec())),
                KeyCode::ArrowUp => return Some(KeyAction::Bytes(b"\x1b[1;3A".to_vec())),
                KeyCode::ArrowDown => return Some(KeyAction::Bytes(b"\x1b[1;3B".to_vec())),
                KeyCode::Backspace => return Some(KeyAction::Bytes(vec![0x1b, 0x7f])),
                KeyCode::Delete => return Some(KeyAction::Bytes(b"\x1bd".to_vec())),
                KeyCode::Enter | KeyCode::NumpadEnter => return Some(KeyAction::Bytes(b"\x1b\r".to_vec())),
                _ => {}
            }
            if option_as_alt && let Some(ch) = key_code_to_ascii(code, mods.shift) {
                return Some(KeyAction::Bytes(vec![0x1b, ch as u8]));
            }
        }

        match key {
            Key::Named(NamedKey::ArrowLeft) => {
                return Some(KeyAction::Bytes(b"\x1bb".to_vec()));
            }
            Key::Named(NamedKey::ArrowRight) => {
                return Some(KeyAction::Bytes(b"\x1bf".to_vec()));
            }
            Key::Named(NamedKey::ArrowUp) => {
                return Some(KeyAction::Bytes(b"\x1b[1;3A".to_vec()));
            }
            Key::Named(NamedKey::ArrowDown) => {
                return Some(KeyAction::Bytes(b"\x1b[1;3B".to_vec()));
            }
            Key::Named(NamedKey::Backspace) => {
                return Some(KeyAction::Bytes(vec![0x1b, 0x7f]));
            }
            Key::Named(NamedKey::Delete) => {
                return Some(KeyAction::Bytes(b"\x1bd".to_vec()));
            }
            Key::Character(s) if option_as_alt => {
                let mut bytes = vec![0x1b];
                bytes.extend_from_slice(s.as_bytes());
                return Some(KeyAction::Bytes(bytes));
            }
            _ => {}
        }
    }

    // 2.1 Shift+Enter / Ctrl+Enter multiline newline support (e.g. for agy, claude, multiline prompts)
    if (mods.shift || mods.ctrl) && !mods.alt && !mods.logo {
        if let Some(code) = physical_key {
            if code == KeyCode::Enter || code == KeyCode::NumpadEnter {
                return Some(KeyAction::Bytes(b"\n".to_vec()));
            }
        }
        if matches!(key, Key::Named(NamedKey::Enter)) {
            return Some(KeyAction::Bytes(b"\n".to_vec()));
        }
        if matches!(key, Key::Character(s) if s == "\r" || s == "\n") {
            return Some(KeyAction::Bytes(b"\n".to_vec()));
        }
    }

    // 2.5 Function keys F1 - F12
    let fn_num = match key {
        Key::Named(NamedKey::F1) => Some(1),
        Key::Named(NamedKey::F2) => Some(2),
        Key::Named(NamedKey::F3) => Some(3),
        Key::Named(NamedKey::F4) => Some(4),
        Key::Named(NamedKey::F5) => Some(5),
        Key::Named(NamedKey::F6) => Some(6),
        Key::Named(NamedKey::F7) => Some(7),
        Key::Named(NamedKey::F8) => Some(8),
        Key::Named(NamedKey::F9) => Some(9),
        Key::Named(NamedKey::F10) => Some(10),
        Key::Named(NamedKey::F11) => Some(11),
        Key::Named(NamedKey::F12) => Some(12),
        _ => match physical_key {
            Some(KeyCode::F1) => Some(1),
            Some(KeyCode::F2) => Some(2),
            Some(KeyCode::F3) => Some(3),
            Some(KeyCode::F4) => Some(4),
            Some(KeyCode::F5) => Some(5),
            Some(KeyCode::F6) => Some(6),
            Some(KeyCode::F7) => Some(7),
            Some(KeyCode::F8) => Some(8),
            Some(KeyCode::F9) => Some(9),
            Some(KeyCode::F10) => Some(10),
            Some(KeyCode::F11) => Some(11),
            Some(KeyCode::F12) => Some(12),
            _ => None,
        },
    };

    if let Some(f) = fn_num {
        let bytes = match f {
            1 => b"\x1bOP".to_vec(),
            2 => b"\x1bOQ".to_vec(),
            3 => b"\x1bOR".to_vec(),
            4 => b"\x1bOS".to_vec(),
            5 => b"\x1b[15~".to_vec(),
            6 => b"\x1b[17~".to_vec(),
            7 => b"\x1b[18~".to_vec(),
            8 => b"\x1b[19~".to_vec(),
            9 => b"\x1b[20~".to_vec(),
            10 => b"\x1b[21~".to_vec(),
            11 => b"\x1b[23~".to_vec(),
            12 => b"\x1b[24~".to_vec(),
            _ => unreachable!(),
        };
        return Some(KeyAction::Bytes(bytes));
    }

    // 3. Navigation and Named keys (Home, End, Insert, Delete, PageUp, PageDown, Arrows, etc.)
    // Check both logical Key::Named and physical_key fallbacks
    if matches!(key, Key::Named(NamedKey::Home)) || physical_key == Some(KeyCode::Home) {
        if app_cursor {
            return Some(KeyAction::Bytes(b"\x1bOH".to_vec()));
        } else {
            return Some(KeyAction::Bytes(b"\x1b[H".to_vec()));
        }
    }

    if matches!(key, Key::Named(NamedKey::End)) || physical_key == Some(KeyCode::End) {
        if app_cursor {
            return Some(KeyAction::Bytes(b"\x1bOF".to_vec()));
        } else {
            return Some(KeyAction::Bytes(b"\x1b[F".to_vec()));
        }
    }

    if matches!(key, Key::Named(NamedKey::Insert)) || physical_key == Some(KeyCode::Insert) {
        return Some(KeyAction::Bytes(b"\x1b[2~".to_vec()));
    }

    if matches!(key, Key::Named(NamedKey::PageUp)) || physical_key == Some(KeyCode::PageUp) {
        return Some(KeyAction::Bytes(b"\x1b[5~".to_vec()));
    }

    if matches!(key, Key::Named(NamedKey::PageDown)) || physical_key == Some(KeyCode::PageDown) {
        return Some(KeyAction::Bytes(b"\x1b[6~".to_vec()));
    }

    if matches!(key, Key::Named(NamedKey::Delete)) || physical_key == Some(KeyCode::Delete) {
        if mods.alt && !mods.ctrl && !mods.logo {
            return Some(KeyAction::Bytes(b"\x1bd".to_vec()));
        }
        return Some(KeyAction::Bytes(b"\x1b[3~".to_vec()));
    }

    if is_arrow_up {
        if app_cursor {
            return Some(KeyAction::Bytes(b"\x1bOA".to_vec()));
        } else {
            return Some(KeyAction::Bytes(b"\x1b[A".to_vec()));
        }
    }

    if is_arrow_down {
        if app_cursor {
            return Some(KeyAction::Bytes(b"\x1bOB".to_vec()));
        } else {
            return Some(KeyAction::Bytes(b"\x1b[B".to_vec()));
        }
    }

    if is_arrow_right {
        if mods.alt && !mods.ctrl && !mods.logo {
            return Some(KeyAction::Bytes(b"\x1bf".to_vec()));
        }
        if app_cursor {
            return Some(KeyAction::Bytes(b"\x1bOC".to_vec()));
        } else {
            return Some(KeyAction::Bytes(b"\x1b[C".to_vec()));
        }
    }

    if is_arrow_left {
        if mods.alt && !mods.ctrl && !mods.logo {
            return Some(KeyAction::Bytes(b"\x1bb".to_vec()));
        }
        if app_cursor {
            return Some(KeyAction::Bytes(b"\x1bOD".to_vec()));
        } else {
            return Some(KeyAction::Bytes(b"\x1b[D".to_vec()));
        }
    }

    if matches!(key, Key::Named(NamedKey::Space)) || physical_key == Some(KeyCode::Space) {
        return Some(KeyAction::Bytes(vec![b' ']));
    }

    if matches!(key, Key::Named(NamedKey::Enter))
        || physical_key == Some(KeyCode::Enter)
        || physical_key == Some(KeyCode::NumpadEnter)
    {
        if mods.alt && !mods.ctrl && !mods.logo {
            return Some(KeyAction::Bytes(b"\x1b\r".to_vec()));
        }
        if mods.shift || mods.ctrl {
            return Some(KeyAction::Bytes(b"\n".to_vec()));
        }
        return Some(KeyAction::Bytes(b"\r".to_vec()));
    }

    if matches!(key, Key::Named(NamedKey::Backspace)) || physical_key == Some(KeyCode::Backspace) {
        if mods.alt && !mods.ctrl && !mods.logo {
            return Some(KeyAction::Bytes(vec![0x1b, 0x7f]));
        }
        return Some(KeyAction::Bytes(vec![0x7f]));
    }

    if matches!(key, Key::Named(NamedKey::Tab)) || physical_key == Some(KeyCode::Tab) {
        if mods.shift {
            return Some(KeyAction::Bytes(b"\x1b[Z".to_vec()));
        } else {
            return Some(KeyAction::Bytes(b"\t".to_vec()));
        }
    }

    // 4. Standard text fallback
    if let Key::Character(s) = key {
        if (mods.shift || mods.ctrl) && !mods.alt && !mods.logo && (s == "\r" || s == "\n") {
            return Some(KeyAction::Bytes(b"\n".to_vec()));
        }
        return Some(KeyAction::Text(s.to_string()));
    }

    None
}

pub fn parse_action_string(val: &str, app_cursor: bool) -> Option<KeyAction> {
    let trimmed = val.trim();
    if let Some(rest) = trimmed.strip_prefix("send_hex:") {
        return parse_hex_bytes(rest).map(KeyAction::Bytes);
    }
    if let Some(rest) = trimmed.strip_prefix("send_bytes:") {
        return Some(KeyAction::Bytes(parse_escaped_bytes(rest)));
    }
    if let Some(rest) = trimmed.strip_prefix("send_text:") {
        return Some(KeyAction::Text(rest.to_string()));
    }

    match trimmed.to_ascii_lowercase().as_str() {
        "new_tab" => Some(KeyAction::NewTab),
        "close_tab" => Some(KeyAction::CloseTab),
        "previous_tab" => Some(KeyAction::PreviousTab),
        "next_tab" => Some(KeyAction::NextTab),
        "new_workspace" => Some(KeyAction::NewWorkspace),
        "previous_workspace" => Some(KeyAction::PreviousWorkspace),
        "next_workspace" => Some(KeyAction::NextWorkspace),
        "cycle_next_window" => Some(KeyAction::CycleNextWindow),
        "toggle_workspace_modal" => Some(KeyAction::ToggleWorkspaceModal),
        "customize_tab_color" => Some(KeyAction::CustomizeTabColor),
        "reload_config" => Some(KeyAction::ReloadConfig),
        "check_for_updates" => Some(KeyAction::CheckForUpdates),
        "paste" => Some(KeyAction::Paste),
        "copy" => Some(KeyAction::Copy),
        "clear_screen" => Some(KeyAction::ClearScreen),
        "increase_font_size" => Some(KeyAction::IncreaseFontSize),
        "decrease_font_size" => Some(KeyAction::DecreaseFontSize),
        "reset_font_size" => Some(KeyAction::ResetFontSize),
        "move_tab_left" => Some(KeyAction::MoveTabLeft),
        "move_tab_right" => Some(KeyAction::MoveTabRight),
        "home" | "beginning_of_line" => {
            Some(KeyAction::Bytes(if app_cursor { b"\x1bOH".to_vec() } else { b"\x1b[H".to_vec() }))
        }
        "end" | "end_of_line" => {
            Some(KeyAction::Bytes(if app_cursor { b"\x1bOF".to_vec() } else { b"\x1b[F".to_vec() }))
        }
        _ => None,
    }
}

pub fn matches_key_spec(spec: &str, key: &Key, phys: Option<KeyCode>, mods: Modifiers) -> bool {
    let parts: Vec<&str> = spec.split('+').map(|s| s.trim()).filter(|s| !s.is_empty()).collect();
    if parts.is_empty() {
        return false;
    }
    let mut expected_logo = false;
    let mut expected_ctrl = false;
    let mut expected_alt = false;
    let mut expected_shift = false;
    let mut key_part = "";

    for (i, &part) in parts.iter().enumerate() {
        let p_lower = part.to_ascii_lowercase();
        if i < parts.len() - 1 {
            match p_lower.as_str() {
                "cmd" | "command" | "super" | "logo" => expected_logo = true,
                "ctrl" | "control" => expected_ctrl = true,
                "alt" | "opt" | "option" => expected_alt = true,
                "shift" => expected_shift = true,
                _ => key_part = part,
            }
        } else {
            key_part = part;
        }
    }

    if mods.logo != expected_logo
        || mods.ctrl != expected_ctrl
        || mods.alt != expected_alt
        || mods.shift != expected_shift
    {
        return false;
    }

    key_matches_str(key_part, key, phys)
}

fn key_matches_str(part: &str, key: &Key, phys: Option<KeyCode>) -> bool {
    let lower = part.to_ascii_lowercase();

    // 1. Check Named keys
    match lower.as_str() {
        "left" | "arrowleft" => return matches!(key, Key::Named(NamedKey::ArrowLeft)) || phys == Some(KeyCode::ArrowLeft),
        "right" | "arrowright" => return matches!(key, Key::Named(NamedKey::ArrowRight)) || phys == Some(KeyCode::ArrowRight),
        "up" | "arrowup" => return matches!(key, Key::Named(NamedKey::ArrowUp)) || phys == Some(KeyCode::ArrowUp),
        "down" | "arrowdown" => return matches!(key, Key::Named(NamedKey::ArrowDown)) || phys == Some(KeyCode::ArrowDown),
        "home" => return matches!(key, Key::Named(NamedKey::Home)) || phys == Some(KeyCode::Home),
        "end" => return matches!(key, Key::Named(NamedKey::End)) || phys == Some(KeyCode::End),
        "pageup" | "pgup" => return matches!(key, Key::Named(NamedKey::PageUp)) || phys == Some(KeyCode::PageUp),
        "pagedown" | "pgdn" => return matches!(key, Key::Named(NamedKey::PageDown)) || phys == Some(KeyCode::PageDown),
        "insert" | "ins" => return matches!(key, Key::Named(NamedKey::Insert)) || phys == Some(KeyCode::Insert),
        "delete" | "del" => return matches!(key, Key::Named(NamedKey::Delete)) || phys == Some(KeyCode::Delete),
        "backspace" | "bksp" => return matches!(key, Key::Named(NamedKey::Backspace)) || phys == Some(KeyCode::Backspace),
        "enter" | "return" => {
            return matches!(key, Key::Named(NamedKey::Enter))
                || phys == Some(KeyCode::Enter)
                || phys == Some(KeyCode::NumpadEnter)
        }
        "tab" => return matches!(key, Key::Named(NamedKey::Tab)) || phys == Some(KeyCode::Tab),
        "esc" | "escape" => {
            return matches!(key, Key::Named(NamedKey::Escape))
                || phys == Some(KeyCode::Escape)
                || matches!(key, Key::Character(s) if s == "\x1b")
        }
        "space" => {
            return matches!(key, Key::Named(NamedKey::Space))
                || phys == Some(KeyCode::Space)
                || matches!(key, Key::Character(s) if s == " ")
        }
        "f1" => return matches!(key, Key::Named(NamedKey::F1)) || phys == Some(KeyCode::F1),
        "f2" => return matches!(key, Key::Named(NamedKey::F2)) || phys == Some(KeyCode::F2),
        "f3" => return matches!(key, Key::Named(NamedKey::F3)) || phys == Some(KeyCode::F3),
        "f4" => return matches!(key, Key::Named(NamedKey::F4)) || phys == Some(KeyCode::F4),
        "f5" => return matches!(key, Key::Named(NamedKey::F5)) || phys == Some(KeyCode::F5),
        "f6" => return matches!(key, Key::Named(NamedKey::F6)) || phys == Some(KeyCode::F6),
        "f7" => return matches!(key, Key::Named(NamedKey::F7)) || phys == Some(KeyCode::F7),
        "f8" => return matches!(key, Key::Named(NamedKey::F8)) || phys == Some(KeyCode::F8),
        "f9" => return matches!(key, Key::Named(NamedKey::F9)) || phys == Some(KeyCode::F9),
        "f10" => return matches!(key, Key::Named(NamedKey::F10)) || phys == Some(KeyCode::F10),
        "f11" => return matches!(key, Key::Named(NamedKey::F11)) || phys == Some(KeyCode::F11),
        "f12" => return matches!(key, Key::Named(NamedKey::F12)) || phys == Some(KeyCode::F12),
        _ => {}
    }

    // 2. Character match (case-insensitive or exact)
    if let Key::Character(ch) = key {
        if ch.eq_ignore_ascii_case(part) {
            return true;
        }
    }

    // 3. PhysicalKey match for letters/digits/symbols
    if let Some(code) = phys {
        match (code, lower.as_str()) {
            (KeyCode::KeyA, "a") | (KeyCode::KeyB, "b") | (KeyCode::KeyC, "c") |
            (KeyCode::KeyD, "d") | (KeyCode::KeyE, "e") | (KeyCode::KeyF, "f") |
            (KeyCode::KeyG, "g") | (KeyCode::KeyH, "h") | (KeyCode::KeyI, "i") |
            (KeyCode::KeyJ, "j") | (KeyCode::KeyK, "k") | (KeyCode::KeyL, "l") |
            (KeyCode::KeyM, "m") | (KeyCode::KeyN, "n") | (KeyCode::KeyO, "o") |
            (KeyCode::KeyP, "p") | (KeyCode::KeyQ, "q") | (KeyCode::KeyR, "r") |
            (KeyCode::KeyS, "s") | (KeyCode::KeyT, "t") | (KeyCode::KeyU, "u") |
            (KeyCode::KeyV, "v") | (KeyCode::KeyW, "w") | (KeyCode::KeyX, "x") |
            (KeyCode::KeyY, "y") | (KeyCode::KeyZ, "z") => return true,
            (KeyCode::Digit0, "0") | (KeyCode::Digit1, "1") | (KeyCode::Digit2, "2") |
            (KeyCode::Digit3, "3") | (KeyCode::Digit4, "4") | (KeyCode::Digit5, "5") |
            (KeyCode::Digit6, "6") | (KeyCode::Digit7, "7") | (KeyCode::Digit8, "8") |
            (KeyCode::Digit9, "9") => return true,
            (KeyCode::BracketLeft, "[" | "{") => return true,
            (KeyCode::BracketRight, "]" | "}") => return true,
            (KeyCode::Backquote, "`" | "~") => return true,
            (KeyCode::Minus, "-" | "_") => return true,
            (KeyCode::Equal, "=" | "+") => return true,
            (KeyCode::Slash, "/" | "?") => return true,
            (KeyCode::Backslash, "\\" | "|") => return true,
            (KeyCode::Semicolon, ";" | ":") => return true,
            (KeyCode::Quote, "'" | "\"") => return true,
            (KeyCode::Comma, "," | "<") => return true,
            (KeyCode::Period, "." | ">") => return true,
            _ => {}
        }
    }

    false
}

fn parse_hex_bytes(s: &str) -> Option<Vec<u8>> {
    let s = s.trim();
    let mut bytes = Vec::new();
    for token in s.split(|c: char| c.is_whitespace() || c == ',' || c == ':') {
        let t = token.trim().trim_start_matches("0x");
        if t.is_empty() {
            continue;
        }
        let t = if t.len() == 1 {
            format!("0{}", t)
        } else {
            t.to_string()
        };
        for chunk in t.as_bytes().chunks(2) {
            if chunk.len() == 2 {
                let hex_str = std::str::from_utf8(chunk).ok()?;
                let b = u8::from_str_radix(hex_str, 16).ok()?;
                bytes.push(b);
            }
        }
    }
    if bytes.is_empty() {
        None
    } else {
        Some(bytes)
    }
}

fn parse_escaped_bytes(s: &str) -> Vec<u8> {
    let mut bytes = Vec::new();
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\\' {
            match chars.next() {
                Some('x') => {
                    let mut hex = String::new();
                    if let Some(&h1) = chars.peek()
                        && h1.is_ascii_hexdigit()
                    {
                        hex.push(chars.next().unwrap());
                        if let Some(&h2) = chars.peek()
                            && h2.is_ascii_hexdigit()
                        {
                            hex.push(chars.next().unwrap());
                        }
                    }
                    if let Ok(b) = u8::from_str_radix(&hex, 16) {
                        bytes.push(b);
                    }
                }
                Some('e') => bytes.push(0x1b),
                Some('n') => bytes.push(b'\n'),
                Some('r') => bytes.push(b'\r'),
                Some('t') => bytes.push(b'\t'),
                Some('\\') => bytes.push(b'\\'),
                Some('0') => bytes.push(0),
                Some(other) => {
                    let mut buf = [0u8; 4];
                    bytes.extend_from_slice(other.encode_utf8(&mut buf).as_bytes());
                }
                None => bytes.push(b'\\'),
            }
        } else {
            let mut buf = [0u8; 4];
            bytes.extend_from_slice(c.encode_utf8(&mut buf).as_bytes());
        }
    }
    bytes
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
