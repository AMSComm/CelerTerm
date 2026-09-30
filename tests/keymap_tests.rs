use celerterm::term::keymap::{translate_key, translate_key_event, translate_key_event_full, Modifiers, KeyAction};
use winit::keyboard::{Key, KeyCode, NamedKey};

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
fn test_shift_and_ctrl_enter_multiline_newline() {
    use celerterm::term::keymap::translate_key_event;
    use winit::keyboard::KeyCode;

    // Shift + Enter sends Line Feed (\n -> 0x0A) for multiline CLI inputs (agy, claude, etc.)
    let shift_mods = Modifiers {
        shift: true,
        ..Default::default()
    };
    assert_eq!(
        translate_key(&Key::Named(NamedKey::Enter), shift_mods, true),
        Some(KeyAction::Bytes(b"\n".to_vec()))
    );

    // Ctrl + Enter also sends Line Feed (\n -> 0x0A)
    let ctrl_mods = Modifiers {
        ctrl: true,
        ..Default::default()
    };
    assert_eq!(
        translate_key(&Key::Named(NamedKey::Enter), ctrl_mods, true),
        Some(KeyAction::Bytes(b"\n".to_vec()))
    );

    // Alt + Enter sends ESC + CR (\x1b\r)
    let alt_mods = Modifiers {
        alt: true,
        ..Default::default()
    };
    assert_eq!(
        translate_key(&Key::Named(NamedKey::Enter), alt_mods, true),
        Some(KeyAction::Bytes(b"\x1b\r".to_vec()))
    );

    // Physical key code for Shift + Enter
    let action_phys_shift_enter = translate_key_event(
        &Key::Named(NamedKey::Enter),
        Some(KeyCode::Enter),
        shift_mods,
        true,
    );
    assert_eq!(action_phys_shift_enter, Some(KeyAction::Bytes(b"\n".to_vec())));

    let action_phys_numpad_enter = translate_key_event(
        &Key::Named(NamedKey::Enter),
        Some(KeyCode::NumpadEnter),
        shift_mods,
        true,
    );
    assert_eq!(action_phys_numpad_enter, Some(KeyAction::Bytes(b"\n".to_vec())));

    // Fallback Character("\r") and Character("\n") with Shift
    assert_eq!(
        translate_key(&Key::Character("\r".into()), shift_mods, true),
        Some(KeyAction::Bytes(b"\n".to_vec()))
    );
    assert_eq!(
        translate_key(&Key::Character("\n".into()), shift_mods, true),
        Some(KeyAction::Bytes(b"\n".to_vec()))
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
    assert_eq!(
        translate_key(&Key::Named(NamedKey::Backspace), mods, false),
        Some(KeyAction::Bytes(vec![0x1b, 0x7f]))
    );
}

#[test]
fn test_option_arrow_word_navigation() {
    use celerterm::term::keymap::translate_key_event;
    use winit::keyboard::KeyCode;

    let mods = Modifiers {
        alt: true,
        ctrl: false,
        shift: false,
        logo: false,
    };

    // Option+Left moves backward word (ESC b -> b"\x1bb")
    assert_eq!(
        translate_key(&Key::Named(NamedKey::ArrowLeft), mods, true),
        Some(KeyAction::Bytes(b"\x1bb".to_vec()))
    );
    // Option+Right moves forward word (ESC f -> b"\x1bf")
    assert_eq!(
        translate_key(&Key::Named(NamedKey::ArrowRight), mods, true),
        Some(KeyAction::Bytes(b"\x1bf".to_vec()))
    );

    // Should also work when option_as_alt is false (arrow navigation is not text composition)
    assert_eq!(
        translate_key(&Key::Named(NamedKey::ArrowLeft), mods, false),
        Some(KeyAction::Bytes(b"\x1bb".to_vec()))
    );
    assert_eq!(
        translate_key(&Key::Named(NamedKey::ArrowRight), mods, false),
        Some(KeyAction::Bytes(b"\x1bf".to_vec()))
    );

    // Physical key code support (e.g. macOS winit keyboard event)
    let action_phys_left = translate_key_event(
        &Key::Named(NamedKey::ArrowLeft),
        Some(KeyCode::ArrowLeft),
        mods,
        true,
    );
    assert_eq!(action_phys_left, Some(KeyAction::Bytes(b"\x1bb".to_vec())));

    let action_phys_right = translate_key_event(
        &Key::Named(NamedKey::ArrowRight),
        Some(KeyCode::ArrowRight),
        mods,
        true,
    );
    assert_eq!(action_phys_right, Some(KeyAction::Bytes(b"\x1bf".to_vec())));
}

#[test]
fn test_option_delete_word() {
    use celerterm::term::keymap::translate_key_event;
    use winit::keyboard::KeyCode;

    let mods = Modifiers {
        alt: true,
        ctrl: false,
        shift: false,
        logo: false,
    };

    // Option+Delete deletes word forward (ESC d -> b"\x1bd")
    assert_eq!(
        translate_key(&Key::Named(NamedKey::Delete), mods, true),
        Some(KeyAction::Bytes(b"\x1bd".to_vec()))
    );
    assert_eq!(
        translate_key(&Key::Named(NamedKey::Delete), mods, false),
        Some(KeyAction::Bytes(b"\x1bd".to_vec()))
    );

    let action_phys_del = translate_key_event(
        &Key::Named(NamedKey::Delete),
        Some(KeyCode::Delete),
        mods,
        true,
    );
    assert_eq!(action_phys_del, Some(KeyAction::Bytes(b"\x1bd".to_vec())));
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

    // Cmd+Up -> Move Tab Left
    let action_move_left = translate_key(&Key::Named(NamedKey::ArrowUp), mods, true);
    assert_eq!(action_move_left, Some(KeyAction::MoveTabLeft));

    // Cmd+Down -> Move Tab Right
    let action_move_right = translate_key(&Key::Named(NamedKey::ArrowDown), mods, true);
    assert_eq!(action_move_right, Some(KeyAction::MoveTabRight));

    // Linux / cross-platform fallback with Ctrl+Shift
    let mods_ctrl_shift = Modifiers {
        alt: false,
        ctrl: true,
        shift: true,
        logo: false,
    };
    assert_eq!(
        translate_key(&Key::Named(NamedKey::ArrowUp), mods_ctrl_shift, true),
        Some(KeyAction::MoveTabLeft)
    );
    assert_eq!(
        translate_key(&Key::Named(NamedKey::ArrowDown), mods_ctrl_shift, true),
        Some(KeyAction::MoveTabRight)
    );
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
    use celerterm::app::is_japanese_char;

    #[derive(Debug, Clone, PartialEq, Eq)]
    enum TestImeCommitAction {
        Append(Vec<u8>),
        Backspace,
        Enter,
        ShiftEnter,
        Confirm,
        Escape,
        None,
    }

    fn process_ime_commit(
        had_preedit: bool,
        text: &str,
        action: TestImeCommitAction,
        is_japanese_env: bool,
    ) -> Vec<u8> {
        let mut out = text.as_bytes().to_vec();
        if had_preedit {
            match action {
                TestImeCommitAction::Append(extra) => {
                    let extra_str = String::from_utf8_lossy(&extra);
                    if !text.ends_with(extra_str.as_ref()) {
                        out.extend_from_slice(&extra);
                    }
                }
                TestImeCommitAction::Backspace => {
                    // Backspace is forwarded by WindowEvent::KeyboardInput which sends 0x7f.
                    // We must NOT send 0x7f here (which would delete 2 characters),
                    // and do NOT send fallback space (which would eat backspace and require 2 presses).
                }
                TestImeCommitAction::Enter => {
                    let is_jp = is_japanese_env || text.chars().any(is_japanese_char);
                    if is_jp {
                        // Japanese: confirm preedit only, no carriage return
                    } else {
                        // Vietnamese / Western: execute command immediately!
                        out.push(b'\r');
                    }
                }
                TestImeCommitAction::ShiftEnter => {
                    // Shift+Enter in Claude CLI / AGY / multiline prompt:
                    // Insert newline without executing command!
                    out.push(b'\n');
                }
                TestImeCommitAction::Confirm => {
                    // Confirm only
                }
                TestImeCommitAction::Escape => {
                    out.push(0x1b);
                }
                TestImeCommitAction::None => {
                    if !text.ends_with(' ') && !text.ends_with('\n') && !text.ends_with('\r') {
                        out.push(b' ');
                    }
                }
            }
        }
        out
    }

    // 1. Vietnamese word committed with Space
    let result_space = process_ime_commit(true, "tiếng", TestImeCommitAction::Append(b" ".to_vec()), false);
    assert_eq!(String::from_utf8(result_space).unwrap(), "tiếng ");

    // 2. Vietnamese command / text committed with Enter: executes command immediately!
    let result_vi_enter = process_ime_commit(true, "ls", TestImeCommitAction::Enter, false);
    assert_eq!(String::from_utf8(result_vi_enter).unwrap(), "ls\r");

    let result_vi_text_enter = process_ime_commit(true, "tiếng Việt", TestImeCommitAction::Enter, false);
    assert_eq!(String::from_utf8(result_vi_text_enter).unwrap(), "tiếng Việt\r");

    // 3. Japanese text committed with Enter: confirms text only, does NOT execute (\r is omitted)
    let result_jp_enter = process_ime_commit(true, "日本", TestImeCommitAction::Enter, false);
    assert_eq!(String::from_utf8(result_jp_enter).unwrap(), "日本");

    let result_jp_hiragana = process_ime_commit(true, "にほん", TestImeCommitAction::Enter, false);
    assert_eq!(String::from_utf8(result_jp_hiragana).unwrap(), "にほん");

    let result_jp_env_romaji = process_ime_commit(true, "nihon", TestImeCommitAction::Enter, true);
    assert_eq!(String::from_utf8(result_jp_env_romaji).unwrap(), "nihon");

    // 4. Shift+Enter in Claude CLI / AGY: appends newline \n without submitting
    let result_shift_enter = process_ime_commit(true, "dòng 1", TestImeCommitAction::ShiftEnter, false);
    assert_eq!(String::from_utf8(result_shift_enter).unwrap(), "dòng 1\n");

    // 4b. Explicit Confirm: confirms text without newline
    let result_explicit_confirm = process_ime_commit(true, "tiếng", TestImeCommitAction::Confirm, false);
    assert_eq!(String::from_utf8(result_explicit_confirm).unwrap(), "tiếng");

    // 5. Typing digit '0' during unconfirmed Vietnamese preedit confirms and appends '0'
    let result_digit_zero = process_ime_commit(true, "tiếng", TestImeCommitAction::Append(b"0".to_vec()), false);
    assert_eq!(String::from_utf8(result_digit_zero).unwrap(), "tiếng0");

    let result_v_zero = process_ime_commit(true, "v", TestImeCommitAction::Append(b"0".to_vec()), false);
    assert_eq!(String::from_utf8(result_v_zero).unwrap(), "v0");

    // 6. Typing other digits (e.g. '3' in python3) confirms and appends digit
    let result_python3 = process_ime_commit(true, "python", TestImeCommitAction::Append(b"3".to_vec()), false);
    assert_eq!(String::from_utf8(result_python3).unwrap(), "python3");

    // 7. Text already ending in digit does not duplicate
    let result_no_dup = process_ime_commit(true, "tiếng0", TestImeCommitAction::Append(b"0".to_vec()), false);
    assert_eq!(String::from_utf8(result_no_dup).unwrap(), "tiếng0");

    // 8. Fallback when extra key event was consumed: automatically adds Space
    let result_fallback = process_ime_commit(true, "Việt", TestImeCommitAction::None, false);
    assert_eq!(String::from_utf8(result_fallback).unwrap(), "Việt ");

    // 9. Committed text already ending in punctuation does not duplicate or add space
    let result_punct = process_ime_commit(true, "tiếng.", TestImeCommitAction::Append(b".".to_vec()), false);
    assert_eq!(String::from_utf8(result_punct).unwrap(), "tiếng.");

    // 10. Normal input without preedit (had_preedit = false) does not append anything
    let result_normal = process_ime_commit(false, "abc", TestImeCommitAction::Append(b" ".to_vec()), false);
    assert_eq!(String::from_utf8(result_normal).unwrap(), "abc");

    // 11. Vietnamese word committed with Backspace:
    let result_backspace = process_ime_commit(true, "tiếng", TestImeCommitAction::Backspace, false);
    assert_eq!(String::from_utf8(result_backspace.clone()).unwrap(), "tiếng");

    // 12. Simulating full Backspace key sequence: IME commit + KeyboardInput
    let mut terminal_input = result_backspace;
    terminal_input.push(0x7f); // from WindowEvent::KeyboardInput
    assert_eq!(terminal_input, b"ti\xe1\xba\xbfng\x7f".to_vec());

    // 13. Empty preedit cancelled with Backspace: does not emit extra bytes
    let result_empty_backspace = process_ime_commit(true, "", TestImeCommitAction::Backspace, false);
    assert_eq!(result_empty_backspace, Vec::<u8>::new());

    // 14. Vietnamese word committed with Escape (e.g. exit insert mode in nvim):
    // commits text and forwards ESC (\x1b)
    let result_escape = process_ime_commit(true, "tiếng", TestImeCommitAction::Escape, false);
    assert_eq!(result_escape, b"ti\xe1\xba\xbfng\x1b".to_vec());

    // 15. Empty preedit cancelled with Escape: sends ESC (\x1b) to exit insert mode in nvim
    let result_empty_escape = process_ime_commit(true, "", TestImeCommitAction::Escape, false);
    assert_eq!(result_empty_escape, vec![0x1b]);

    // 16. Verify is_japanese_char character classifications
    assert!(is_japanese_char('あ'));
    assert!(is_japanese_char('ん'));
    assert!(is_japanese_char('ア'));
    assert!(is_japanese_char('ン'));
    assert!(is_japanese_char('日'));
    assert!(is_japanese_char('本'));
    assert!(is_japanese_char('１')); // fullwidth 1
    assert!(!is_japanese_char('a'));
    assert!(!is_japanese_char('0'));
    assert!(!is_japanese_char('ế'));
    assert!(!is_japanese_char('đ'));
    assert!(!is_japanese_char('ư'));
}

#[test]
fn test_escape_key_translation() {
    use winit::keyboard::{KeyCode, NativeKey};

    let default_mods = Modifiers::default();

    // 1. Standard NamedKey::Escape -> 0x1b
    assert_eq!(
        translate_key_event(&Key::Named(NamedKey::Escape), None, default_mods, true),
        Some(KeyAction::Bytes(vec![0x1b]))
    );

    // 2. Physical KeyCode::Escape with NamedKey -> 0x1b
    assert_eq!(
        translate_key_event(&Key::Named(NamedKey::Escape), Some(KeyCode::Escape), default_mods, true),
        Some(KeyAction::Bytes(vec![0x1b]))
    );

    // 3. Raw Escape character \x1b -> 0x1b
    assert_eq!(
        translate_key_event(&Key::Character("\x1b".into()), None, default_mods, true),
        Some(KeyAction::Bytes(vec![0x1b]))
    );

    // 4. Physical KeyCode::Escape even if Key is Unidentified on macOS -> 0x1b
    assert_eq!(
        translate_key_event(&Key::Unidentified(NativeKey::MacOS(53)), Some(KeyCode::Escape), default_mods, true),
        Some(KeyAction::Bytes(vec![0x1b]))
    );

    // 5. Shift + Escape -> 0x1b
    let shift_mods = Modifiers {
        shift: true,
        ..Default::default()
    };
    assert_eq!(
        translate_key_event(&Key::Named(NamedKey::Escape), Some(KeyCode::Escape), shift_mods, true),
        Some(KeyAction::Bytes(vec![0x1b]))
    );

    // 6. Option / Alt + Escape -> 0x1b 0x1b (ESC ESC)
    let alt_mods = Modifiers {
        alt: true,
        ..Default::default()
    };
    assert_eq!(
        translate_key_event(&Key::Named(NamedKey::Escape), Some(KeyCode::Escape), alt_mods, true),
        Some(KeyAction::Bytes(vec![0x1b, 0x1b]))
    );

    // 7. Ctrl + Escape -> 0x1b
    let ctrl_mods = Modifiers {
        ctrl: true,
        ..Default::default()
    };
    assert_eq!(
        translate_key_event(&Key::Named(NamedKey::Escape), Some(KeyCode::Escape), ctrl_mods, true),
        Some(KeyAction::Bytes(vec![0x1b]))
    );
}

#[test]
fn test_japanese_and_cjk_preedit_display_width() {
    use unicode_width::UnicodeWidthStr;

    // Japanese Hiragana & Kanji are 2 columns wide each in terminal
    assert_eq!(UnicodeWidthStr::width("にほん"), 6);
    assert_eq!(UnicodeWidthStr::width("日本語"), 6);
    assert_eq!(UnicodeWidthStr::width("こんにちは"), 10);

    // Vietnamese accented characters are 1 column wide each
    assert_eq!(UnicodeWidthStr::width("tiếng"), 5);
    assert_eq!(UnicodeWidthStr::width("Việt"), 4);

    // ASCII is 1 column wide each
    assert_eq!(UnicodeWidthStr::width("hello"), 5);

    // Mixed text
    assert_eq!(UnicodeWidthStr::width("Rust日本語"), 4 + 6);
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

#[test]
fn test_customize_tab_color_shortcuts() {
    let cmd_shift = Modifiers {
        alt: false,
        ctrl: false,
        shift: true,
        logo: true,
    };

    // macOS: Cmd+Shift+T and Cmd+Shift+K
    assert_eq!(
        translate_key(&Key::Character("T".into()), cmd_shift, true),
        Some(KeyAction::CustomizeTabColor)
    );
    assert_eq!(
        translate_key(&Key::Character("t".into()), cmd_shift, true),
        Some(KeyAction::CustomizeTabColor)
    );
    assert_eq!(
        translate_key(&Key::Character("K".into()), cmd_shift, true),
        Some(KeyAction::CustomizeTabColor)
    );
    assert_eq!(
        translate_key(&Key::Character("k".into()), cmd_shift, true),
        Some(KeyAction::CustomizeTabColor)
    );

    // Linux/Cross-platform: Ctrl+Shift+T and Ctrl+Shift+K
    let ctrl_shift = Modifiers {
        alt: false,
        ctrl: true,
        shift: true,
        logo: false,
    };
    assert_eq!(
        translate_key(&Key::Character("T".into()), ctrl_shift, true),
        Some(KeyAction::CustomizeTabColor)
    );
    assert_eq!(
        translate_key(&Key::Character("t".into()), ctrl_shift, true),
        Some(KeyAction::CustomizeTabColor)
    );
    assert_eq!(
        translate_key(&Key::Character("K".into()), ctrl_shift, true),
        Some(KeyAction::CustomizeTabColor)
    );
    assert_eq!(
        translate_key(&Key::Character("k".into()), ctrl_shift, true),
        Some(KeyAction::CustomizeTabColor)
    );
}

#[test]
fn test_cmd_backquote_cycle_next_window() {
    use winit::keyboard::KeyCode;

    let cmd_mods = Modifiers {
        alt: false,
        ctrl: false,
        shift: false,
        logo: true,
    };

    // Cmd + Backquote physical key
    assert_eq!(
        translate_key_event(&Key::Character("`".into()), Some(KeyCode::Backquote), cmd_mods, true),
        Some(KeyAction::CycleNextWindow)
    );

    // Cmd + ` character key without physical key
    assert_eq!(
        translate_key(&Key::Character("`".into()), cmd_mods, true),
        Some(KeyAction::CycleNextWindow)
    );

    // Cmd + ~ (with shift)
    let cmd_shift = Modifiers {
        alt: false,
        ctrl: false,
        shift: true,
        logo: true,
    };
    assert_eq!(
        translate_key(&Key::Character("~".into()), cmd_shift, true),
        Some(KeyAction::CycleNextWindow)
    );
}

#[test]
fn test_app_cursor_mode_home_end_and_arrows() {
    let mods = Modifiers::default();

    // 1. Normal mode (app_cursor = false) -> CSI sequences
    assert_eq!(
        translate_key_event_full(&Key::Named(NamedKey::Home), None, mods, true, false, None),
        Some(KeyAction::Bytes(b"\x1b[H".to_vec()))
    );
    assert_eq!(
        translate_key_event_full(&Key::Named(NamedKey::End), None, mods, true, false, None),
        Some(KeyAction::Bytes(b"\x1b[F".to_vec()))
    );
    assert_eq!(
        translate_key_event_full(&Key::Named(NamedKey::ArrowUp), None, mods, true, false, None),
        Some(KeyAction::Bytes(b"\x1b[A".to_vec()))
    );
    assert_eq!(
        translate_key_event_full(&Key::Named(NamedKey::ArrowDown), None, mods, true, false, None),
        Some(KeyAction::Bytes(b"\x1b[B".to_vec()))
    );
    assert_eq!(
        translate_key_event_full(&Key::Named(NamedKey::ArrowRight), None, mods, true, false, None),
        Some(KeyAction::Bytes(b"\x1b[C".to_vec()))
    );
    assert_eq!(
        translate_key_event_full(&Key::Named(NamedKey::ArrowLeft), None, mods, true, false, None),
        Some(KeyAction::Bytes(b"\x1b[D".to_vec()))
    );

    // 2. Application Cursor Mode (app_cursor = true, DECCKM) -> SS3 sequences
    assert_eq!(
        translate_key_event_full(&Key::Named(NamedKey::Home), None, mods, true, true, None),
        Some(KeyAction::Bytes(b"\x1bOH".to_vec()))
    );
    assert_eq!(
        translate_key_event_full(&Key::Named(NamedKey::End), None, mods, true, true, None),
        Some(KeyAction::Bytes(b"\x1bOF".to_vec()))
    );
    assert_eq!(
        translate_key_event_full(&Key::Named(NamedKey::ArrowUp), None, mods, true, true, None),
        Some(KeyAction::Bytes(b"\x1bOA".to_vec()))
    );
    assert_eq!(
        translate_key_event_full(&Key::Named(NamedKey::ArrowDown), None, mods, true, true, None),
        Some(KeyAction::Bytes(b"\x1bOB".to_vec()))
    );
    assert_eq!(
        translate_key_event_full(&Key::Named(NamedKey::ArrowRight), None, mods, true, true, None),
        Some(KeyAction::Bytes(b"\x1bOC".to_vec()))
    );
    assert_eq!(
        translate_key_event_full(&Key::Named(NamedKey::ArrowLeft), None, mods, true, true, None),
        Some(KeyAction::Bytes(b"\x1bOD".to_vec()))
    );
}

#[test]
fn test_physical_key_fallbacks_navigation() {
    let mods = Modifiers::default();
    let unidentified = Key::Unidentified(winit::keyboard::NativeKey::MacOS(0));

    // Home & End via physical key
    assert_eq!(
        translate_key_event_full(&unidentified, Some(KeyCode::Home), mods, true, false, None),
        Some(KeyAction::Bytes(b"\x1b[H".to_vec()))
    );
    assert_eq!(
        translate_key_event_full(&unidentified, Some(KeyCode::Home), mods, true, true, None),
        Some(KeyAction::Bytes(b"\x1bOH".to_vec()))
    );
    assert_eq!(
        translate_key_event_full(&unidentified, Some(KeyCode::End), mods, true, false, None),
        Some(KeyAction::Bytes(b"\x1b[F".to_vec()))
    );
    assert_eq!(
        translate_key_event_full(&unidentified, Some(KeyCode::End), mods, true, true, None),
        Some(KeyAction::Bytes(b"\x1bOF".to_vec()))
    );

    // Insert, Delete, PageUp, PageDown via physical key
    assert_eq!(
        translate_key_event_full(&unidentified, Some(KeyCode::Insert), mods, true, false, None),
        Some(KeyAction::Bytes(b"\x1b[2~".to_vec()))
    );
    assert_eq!(
        translate_key_event_full(&unidentified, Some(KeyCode::Delete), mods, true, false, None),
        Some(KeyAction::Bytes(b"\x1b[3~".to_vec()))
    );
    assert_eq!(
        translate_key_event_full(&unidentified, Some(KeyCode::PageUp), mods, true, false, None),
        Some(KeyAction::Bytes(b"\x1b[5~".to_vec()))
    );
    assert_eq!(
        translate_key_event_full(&unidentified, Some(KeyCode::PageDown), mods, true, false, None),
        Some(KeyAction::Bytes(b"\x1b[6~".to_vec()))
    );

    // Arrows via physical key
    assert_eq!(
        translate_key_event_full(&unidentified, Some(KeyCode::ArrowUp), mods, true, false, None),
        Some(KeyAction::Bytes(b"\x1b[A".to_vec()))
    );
    assert_eq!(
        translate_key_event_full(&unidentified, Some(KeyCode::ArrowDown), mods, true, false, None),
        Some(KeyAction::Bytes(b"\x1b[B".to_vec()))
    );
}

#[test]
fn test_neovim_ctrl_6_alternate_buffer_and_special_ctrl_keys() {
    let ctrl_mods = Modifiers {
        ctrl: true,
        ..Default::default()
    };

    // Ctrl+6 should emit 0x1E (Record Separator / <C-^>) for Neovim alternate buffer switch
    assert_eq!(
        translate_key(&Key::Character("6".into()), ctrl_mods, true),
        Some(KeyAction::Bytes(vec![0x1e]))
    );
    assert_eq!(
        translate_key_event(&Key::Character("6".into()), Some(KeyCode::Digit6), ctrl_mods, true),
        Some(KeyAction::Bytes(vec![0x1e]))
    );
    assert_eq!(
        translate_key(&Key::Character("^".into()), ctrl_mods, true),
        Some(KeyAction::Bytes(vec![0x1e]))
    );

    // Ctrl+2 / Ctrl+@ -> 0x00 (NUL)
    assert_eq!(
        translate_key(&Key::Character("2".into()), ctrl_mods, true),
        Some(KeyAction::Bytes(vec![0x00]))
    );
    assert_eq!(
        translate_key(&Key::Named(NamedKey::Space), ctrl_mods, true),
        Some(KeyAction::Bytes(vec![0x00]))
    );

    // Ctrl+3 -> 0x1B (ESC)
    assert_eq!(
        translate_key(&Key::Character("3".into()), ctrl_mods, true),
        Some(KeyAction::Bytes(vec![0x1b]))
    );

    // Ctrl+4 -> 0x1C (FS)
    assert_eq!(
        translate_key(&Key::Character("4".into()), ctrl_mods, true),
        Some(KeyAction::Bytes(vec![0x1c]))
    );

    // Ctrl+5 -> 0x1D (GS)
    assert_eq!(
        translate_key(&Key::Character("5".into()), ctrl_mods, true),
        Some(KeyAction::Bytes(vec![0x1d]))
    );

    // Ctrl+7 -> 0x1F (US)
    assert_eq!(
        translate_key(&Key::Character("7".into()), ctrl_mods, true),
        Some(KeyAction::Bytes(vec![0x1f]))
    );

    // Ctrl+8 -> 0x7F (DEL)
    assert_eq!(
        translate_key(&Key::Character("8".into()), ctrl_mods, true),
        Some(KeyAction::Bytes(vec![0x7f]))
    );
}

#[test]
fn test_function_keys_f1_to_f12() {
    let mods = Modifiers::default();

    assert_eq!(translate_key(&Key::Named(NamedKey::F1), mods, true), Some(KeyAction::Bytes(b"\x1bOP".to_vec())));
    assert_eq!(translate_key(&Key::Named(NamedKey::F2), mods, true), Some(KeyAction::Bytes(b"\x1bOQ".to_vec())));
    assert_eq!(translate_key(&Key::Named(NamedKey::F3), mods, true), Some(KeyAction::Bytes(b"\x1bOR".to_vec())));
    assert_eq!(translate_key(&Key::Named(NamedKey::F4), mods, true), Some(KeyAction::Bytes(b"\x1bOS".to_vec())));
    assert_eq!(translate_key(&Key::Named(NamedKey::F5), mods, true), Some(KeyAction::Bytes(b"\x1b[15~".to_vec())));
    assert_eq!(translate_key(&Key::Named(NamedKey::F6), mods, true), Some(KeyAction::Bytes(b"\x1b[17~".to_vec())));
    assert_eq!(translate_key(&Key::Named(NamedKey::F7), mods, true), Some(KeyAction::Bytes(b"\x1b[18~".to_vec())));
    assert_eq!(translate_key(&Key::Named(NamedKey::F8), mods, true), Some(KeyAction::Bytes(b"\x1b[19~".to_vec())));
    assert_eq!(translate_key(&Key::Named(NamedKey::F9), mods, true), Some(KeyAction::Bytes(b"\x1b[20~".to_vec())));
    assert_eq!(translate_key(&Key::Named(NamedKey::F10), mods, true), Some(KeyAction::Bytes(b"\x1b[21~".to_vec())));
    assert_eq!(translate_key(&Key::Named(NamedKey::F11), mods, true), Some(KeyAction::Bytes(b"\x1b[23~".to_vec())));
    assert_eq!(translate_key(&Key::Named(NamedKey::F12), mods, true), Some(KeyAction::Bytes(b"\x1b[24~".to_vec())));

    // Physical key fallback for F1
    let unidentified = Key::Unidentified(winit::keyboard::NativeKey::MacOS(0));
    assert_eq!(
        translate_key_event(&unidentified, Some(KeyCode::F1), mods, true),
        Some(KeyAction::Bytes(b"\x1bOP".to_vec()))
    );
}

#[test]
fn test_modified_arrow_keys_ctrl_and_shift() {
    let ctrl_mods = Modifiers {
        ctrl: true,
        ..Default::default()
    };
    let shift_mods = Modifiers {
        shift: true,
        ..Default::default()
    };

    // Ctrl + Arrows (word navigation in shells/CLI)
    assert_eq!(
        translate_key(&Key::Named(NamedKey::ArrowRight), ctrl_mods, true),
        Some(KeyAction::Bytes(b"\x1b[1;5C".to_vec()))
    );
    assert_eq!(
        translate_key(&Key::Named(NamedKey::ArrowLeft), ctrl_mods, true),
        Some(KeyAction::Bytes(b"\x1b[1;5D".to_vec()))
    );
    assert_eq!(
        translate_key(&Key::Named(NamedKey::ArrowUp), ctrl_mods, true),
        Some(KeyAction::Bytes(b"\x1b[1;5A".to_vec()))
    );
    assert_eq!(
        translate_key(&Key::Named(NamedKey::ArrowDown), ctrl_mods, true),
        Some(KeyAction::Bytes(b"\x1b[1;5B".to_vec()))
    );

    // Shift + Arrows (text selection in editors)
    assert_eq!(
        translate_key(&Key::Named(NamedKey::ArrowRight), shift_mods, true),
        Some(KeyAction::Bytes(b"\x1b[1;2C".to_vec()))
    );
    assert_eq!(
        translate_key(&Key::Named(NamedKey::ArrowLeft), shift_mods, true),
        Some(KeyAction::Bytes(b"\x1b[1;2D".to_vec()))
    );
}

#[test]
fn test_custom_keybindings_configuration() {
    use std::collections::HashMap;

    let mut bindings = HashMap::new();
    bindings.insert("cmd+left".to_string(), "send_hex:01".to_string());
    bindings.insert("cmd+right".to_string(), "send_hex:05".to_string());
    bindings.insert("cmd+shift+[".to_string(), "previous_tab".to_string());
    bindings.insert("cmd+shift+]".to_string(), "next_tab".to_string());
    bindings.insert("alt+home".to_string(), "send_bytes:\\x1b[1;3H".to_string());

    let cmd_mods = Modifiers {
        logo: true,
        ..Default::default()
    };
    let cmd_shift = Modifiers {
        logo: true,
        shift: true,
        ..Default::default()
    };
    let alt_mods = Modifiers {
        alt: true,
        ..Default::default()
    };

    // 1. Without custom bindings, cmd+left/right switches tabs (default behavior)
    assert_eq!(
        translate_key(&Key::Named(NamedKey::ArrowLeft), cmd_mods, true),
        Some(KeyAction::PreviousTab)
    );
    assert_eq!(
        translate_key(&Key::Named(NamedKey::ArrowRight), cmd_mods, true),
        Some(KeyAction::NextTab)
    );

    // 2. With custom bindings, cmd+left sends Ctrl+A (0x01) and cmd+right sends Ctrl+E (0x05)
    assert_eq!(
        translate_key_event_full(&Key::Named(NamedKey::ArrowLeft), None, cmd_mods, true, false, Some(&bindings)),
        Some(KeyAction::Bytes(vec![0x01]))
    );
    assert_eq!(
        translate_key_event_full(&Key::Named(NamedKey::ArrowRight), None, cmd_mods, true, false, Some(&bindings)),
        Some(KeyAction::Bytes(vec![0x05]))
    );

    // 3. Tab switching remapped to Cmd+Shift+[ and Cmd+Shift+]
    assert_eq!(
        translate_key_event_full(&Key::Character("[".into()), None, cmd_shift, true, false, Some(&bindings)),
        Some(KeyAction::PreviousTab)
    );
    assert_eq!(
        translate_key_event_full(&Key::Character("]".into()), None, cmd_shift, true, false, Some(&bindings)),
        Some(KeyAction::NextTab)
    );

    // 4. Custom escaped bytes
    assert_eq!(
        translate_key_event_full(&Key::Named(NamedKey::Home), None, alt_mods, true, false, Some(&bindings)),
        Some(KeyAction::Bytes(b"\x1b[1;3H".to_vec()))
    );
}

#[test]
fn test_close_all_windows_shortcuts() {
    let cmd_shift = Modifiers {
        logo: true,
        shift: true,
        ..Default::default()
    };
    let opt_cmd = Modifiers {
        logo: true,
        alt: true,
        ..Default::default()
    };
    let ctrl_shift = Modifiers {
        ctrl: true,
        shift: true,
        ..Default::default()
    };

    // 1. Cmd+Shift+Q -> CloseAllWindows
    assert_eq!(
        translate_key(&Key::Character("q".into()), cmd_shift, true),
        Some(KeyAction::CloseAllWindows)
    );
    assert_eq!(
        translate_key(&Key::Character("Q".into()), cmd_shift, true),
        Some(KeyAction::CloseAllWindows)
    );

    // 2. Option+Cmd+W and Option+Cmd+Q -> CloseAllWindows
    assert_eq!(
        translate_key(&Key::Character("w".into()), opt_cmd, true),
        Some(KeyAction::CloseAllWindows)
    );
    assert_eq!(
        translate_key(&Key::Character("q".into()), opt_cmd, true),
        Some(KeyAction::CloseAllWindows)
    );

    // 3. Ctrl+Shift+Q -> CloseAllWindows
    assert_eq!(
        translate_key(&Key::Character("q".into()), ctrl_shift, true),
        Some(KeyAction::CloseAllWindows)
    );

    // 4. Custom keybindings mapping close_all_windows and quit_all
    let mut bindings = std::collections::HashMap::new();
    bindings.insert("ctrl+q".to_string(), "close_all_windows".to_string());
    bindings.insert("alt+q".to_string(), "quit_all".to_string());

    let ctrl_mods = Modifiers {
        ctrl: true,
        ..Default::default()
    };
    let alt_mods = Modifiers {
        alt: true,
        ..Default::default()
    };

    assert_eq!(
        translate_key_event_full(&Key::Character("q".into()), None, ctrl_mods, true, false, Some(&bindings)),
        Some(KeyAction::CloseAllWindows)
    );
    assert_eq!(
        translate_key_event_full(&Key::Character("q".into()), None, alt_mods, true, false, Some(&bindings)),
        Some(KeyAction::CloseAllWindows)
    );
}




