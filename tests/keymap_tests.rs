use celerterm::term::keymap::{translate_key, Modifiers, KeyAction};
use winit::keyboard::{Key, NamedKey};

#[test]
fn test_option_as_alt_character_key() {
    let mods = Modifiers {
        alt: true,
        ctrl: false,
        shift: false,
        logo: false,
    };

    // When option_as_alt is true, Option+f sends ESC + 'f' -> b"\x1bf"
    let action_alt_on = translate_key(&Key::Character("f".into()), mods, true);
    assert_eq!(action_alt_on, Some(KeyAction::Bytes(b"\x1bf".to_vec())));

    let action_alt_b = translate_key(&Key::Character("b".into()), mods, true);
    assert_eq!(action_alt_b, Some(KeyAction::Bytes(b"\x1bb".to_vec())));

    let action_alt_d = translate_key(&Key::Character("d".into()), mods, true);
    assert_eq!(action_alt_d, Some(KeyAction::Bytes(b"\x1bd".to_vec())));
}

#[test]
fn test_option_as_alt_disabled_passes_text() {
    let mods = Modifiers {
        alt: true,
        ctrl: false,
        shift: false,
        logo: false,
    };

    // When option_as_alt is false, let macOS compose standard unicode character (e.g. ƒ)
    let action_alt_off = translate_key(&Key::Character("ƒ".into()), mods, false);
    assert_eq!(action_alt_off, Some(KeyAction::Text("ƒ".to_string())));
}

#[test]
fn test_control_keys() {
    let mods = Modifiers {
        alt: false,
        ctrl: true,
        shift: false,
        logo: false,
    };

    // Ctrl+C -> 0x03 (ETX)
    let action_ctrl_c = translate_key(&Key::Character("c".into()), mods, true);
    assert_eq!(action_ctrl_c, Some(KeyAction::Bytes(vec![0x03])));

    // Ctrl+D -> 0x04 (EOT)
    let action_ctrl_d = translate_key(&Key::Character("d".into()), mods, true);
    assert_eq!(action_ctrl_d, Some(KeyAction::Bytes(vec![0x04])));

    // Ctrl+Z -> 0x1A (SUB)
    let action_ctrl_z = translate_key(&Key::Character("z".into()), mods, true);
    assert_eq!(action_ctrl_z, Some(KeyAction::Bytes(vec![0x1a])));
}

#[test]
fn test_named_keys_arrows_and_enter() {
    let mods = Modifiers::default();

    assert_eq!(
        translate_key(&Key::Named(NamedKey::ArrowUp), mods, true),
        Some(KeyAction::Bytes(b"\x1b[A".to_vec()))
    );
    assert_eq!(
        translate_key(&Key::Named(NamedKey::ArrowDown), mods, true),
        Some(KeyAction::Bytes(b"\x1b[B".to_vec()))
    );
    assert_eq!(
        translate_key(&Key::Named(NamedKey::ArrowRight), mods, true),
        Some(KeyAction::Bytes(b"\x1b[C".to_vec()))
    );
    assert_eq!(
        translate_key(&Key::Named(NamedKey::ArrowLeft), mods, true),
        Some(KeyAction::Bytes(b"\x1b[D".to_vec()))
    );
    assert_eq!(
        translate_key(&Key::Named(NamedKey::Enter), mods, true),
        Some(KeyAction::Bytes(b"\r".to_vec()))
    );
    assert_eq!(
        translate_key(&Key::Named(NamedKey::Backspace), mods, true),
        Some(KeyAction::Bytes(vec![0x7f]))
    );
}

#[test]
fn test_alt_backspace_word_delete() {
    let mods = Modifiers {
        alt: true,
        ctrl: false,
        shift: false,
        logo: false,
    };

    // Alt+Backspace sends ESC + 0x7F to delete word backwards in readline/zsh
    assert_eq!(
        translate_key(&Key::Named(NamedKey::Backspace), mods, true),
        Some(KeyAction::Bytes(vec![0x1b, 0x7f]))
    );
}

#[test]
fn test_cmd_number_tab_selection() {
    let mods = Modifiers {
        alt: false,
        ctrl: false,
        shift: false,
        logo: true, // Command on macOS
    };

    // Cmd+1 to Cmd+9
    for i in 1..=9 {
        let key_str = i.to_string();
        let action = translate_key(&Key::Character(key_str.into()), mods, true);
        assert_eq!(action, Some(KeyAction::SelectTab(i)));
    }
}

#[test]
fn test_cmd_arrow_tab_navigation() {
    let mods = Modifiers {
        alt: false,
        ctrl: false,
        shift: false,
        logo: true, // Command on macOS
    };

    // Cmd+Left -> Previous Tab
    let action_prev = translate_key(&Key::Named(NamedKey::ArrowLeft), mods, true);
    assert_eq!(action_prev, Some(KeyAction::PreviousTab));

    // Cmd+Right -> Next Tab
    let action_next = translate_key(&Key::Named(NamedKey::ArrowRight), mods, true);
    assert_eq!(action_next, Some(KeyAction::NextTab));
}
