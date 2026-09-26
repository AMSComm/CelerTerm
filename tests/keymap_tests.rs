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

#[test]
fn test_space_key() {
    let mods = Modifiers::default();
    let action = translate_key(&Key::Named(NamedKey::Space), mods, true);
    assert_eq!(action, Some(KeyAction::Bytes(vec![b' '])));
}

#[test]
fn test_cmd_t_new_tab_and_cmd_w_close_tab() {
    let mods = Modifiers {
        alt: false,
        ctrl: false,
        shift: false,
        logo: true,
    };

    let action_new_tab = translate_key(&Key::Character("t".into()), mods, true);
    assert_eq!(action_new_tab, Some(KeyAction::NewTab));

    let action_close_tab = translate_key(&Key::Character("w".into()), mods, true);
    assert_eq!(action_close_tab, Some(KeyAction::CloseTab));

    let action_quit = translate_key(&Key::Character("q".into()), mods, true);
    assert_eq!(action_quit, Some(KeyAction::Quit));
}

#[test]
fn test_cmd_shift_workspace_shortcuts() {
    let mods = Modifiers {
        alt: false,
        ctrl: false,
        shift: true,
        logo: true,
    };

    let action_new_ws = translate_key(&Key::Character("N".into()), mods, true);
    assert_eq!(action_new_ws, Some(KeyAction::NewWorkspace));

    let action_prev_ws = translate_key(&Key::Character("{".into()), mods, true);
    assert_eq!(action_prev_ws, Some(KeyAction::PreviousWorkspace));

    let action_next_ws = translate_key(&Key::Character("}".into()), mods, true);
    assert_eq!(action_next_ws, Some(KeyAction::NextWorkspace));
}

#[test]
fn test_option_q_neovim_physical_key() {
    use celerterm::term::keymap::translate_key_event;
    use winit::keyboard::KeyCode;

    let mods = Modifiers {
        alt: true,
        ctrl: false,
        shift: false,
        logo: false,
    };

    // On macOS, Alt+Q produces logical character "œ", but physical key is KeyCode::KeyQ
    let action = translate_key_event(
        &Key::Character("œ".into()),
        Some(KeyCode::KeyQ),
        mods,
        true,
    );
    assert_eq!(action, Some(KeyAction::Bytes(b"\x1bq".to_vec())));
}

#[test]
fn test_cmd_clipboard_and_utility_shortcuts() {
    let mods = Modifiers {
        alt: false,
        ctrl: false,
        shift: false,
        logo: true,
    };

    assert_eq!(
        translate_key(&Key::Character("c".into()), mods, true),
        Some(KeyAction::Copy)
    );
    assert_eq!(
        translate_key(&Key::Character("v".into()), mods, true),
        Some(KeyAction::Paste)
    );
    assert_eq!(
        translate_key(&Key::Character("k".into()), mods, true),
        Some(KeyAction::ClearScreen)
    );
    assert_eq!(
        translate_key(&Key::Character("=".into()), mods, true),
        Some(KeyAction::IncreaseFontSize)
    );
    assert_eq!(
        translate_key(&Key::Character("+".into()), mods, true),
        Some(KeyAction::IncreaseFontSize)
    );
    assert_eq!(
        translate_key(&Key::Character("-".into()), mods, true),
        Some(KeyAction::DecreaseFontSize)
    );
    assert_eq!(
        translate_key(&Key::Character("0".into()), mods, true),
        Some(KeyAction::ResetFontSize)
    );
}

#[test]
fn test_vietnamese_ime_commit_decision_logic() {
    fn process_ime_commit(
        had_preedit: bool,
        text: &str,
        extra: Option<&[u8]>,
    ) -> Vec<u8> {
        let mut out = text.as_bytes().to_vec();
        if had_preedit {
            if let Some(extra_bytes) = extra {
                let extra_str = String::from_utf8_lossy(extra_bytes);
                if !text.ends_with(extra_str.as_ref()) {
                    out.extend_from_slice(extra_bytes);
                }
            } else if !text.ends_with(' ') && !text.ends_with('\n') && !text.ends_with('\r') {
                out.push(b' ');
            }
        }
        out
    }

    // 1. Vietnamese word committed with Space
    let result_space = process_ime_commit(true, "tiếng", Some(b" "));
    assert_eq!(String::from_utf8(result_space).unwrap(), "tiếng ");

    // 2. Vietnamese word committed with Enter
    let result_enter = process_ime_commit(true, "tiếng", Some(b"\r"));
    assert_eq!(String::from_utf8(result_enter).unwrap(), "tiếng\r");

    // 3. Fallback when extra key event was consumed: automatically adds Space
    let result_fallback = process_ime_commit(true, "Việt", None);
    assert_eq!(String::from_utf8(result_fallback).unwrap(), "Việt ");

    // 4. Committed text already ending in punctuation does not duplicate or add space
    let result_punct = process_ime_commit(true, "tiếng.", Some(b"."));
    assert_eq!(String::from_utf8(result_punct).unwrap(), "tiếng.");

    // 5. Normal input without preedit (had_preedit = false) does not append anything
    let result_normal = process_ime_commit(false, "abc", Some(b" "));
    assert_eq!(String::from_utf8(result_normal).unwrap(), "abc");
}

#[test]
fn test_workspace_modal_and_reload_config_shortcuts() {
    let cmd_shift = Modifiers {
        alt: false,
        ctrl: false,
        shift: true,
        logo: true,
    };

    // Cmd+Shift+P / Cmd+Shift+O -> ToggleWorkspaceModal
    assert_eq!(
        translate_key(&Key::Character("P".into()), cmd_shift, true),
        Some(KeyAction::ToggleWorkspaceModal)
    );
    assert_eq!(
        translate_key(&Key::Character("p".into()), cmd_shift, true),
        Some(KeyAction::ToggleWorkspaceModal)
    );
    assert_eq!(
        translate_key(&Key::Character("O".into()), cmd_shift, true),
        Some(KeyAction::ToggleWorkspaceModal)
    );

    // Cmd+Shift+R -> ReloadConfig
    assert_eq!(
        translate_key(&Key::Character("R".into()), cmd_shift, true),
        Some(KeyAction::ReloadConfig)
    );
    assert_eq!(
        translate_key(&Key::Character("r".into()), cmd_shift, true),
        Some(KeyAction::ReloadConfig)
    );

    // Linux/Cross-platform: Ctrl+Shift+P / Ctrl+Shift+R
    let ctrl_shift = Modifiers {
        alt: false,
        ctrl: true,
        shift: true,
        logo: false,
    };
    assert_eq!(
        translate_key(&Key::Character("P".into()), ctrl_shift, true),
        Some(KeyAction::ToggleWorkspaceModal)
    );
    assert_eq!(
        translate_key(&Key::Character("R".into()), ctrl_shift, true),
        Some(KeyAction::ReloadConfig)
    );
}

