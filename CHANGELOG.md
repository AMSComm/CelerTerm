# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.2.12] - 2026-09-28

### Added & Improved

- **Modern Terminal App Icon**:
  - Redesigned CelerTerm application icon following macOS Big Sur+ Human Interface Guidelines (1024x1024 Squircle).
  - Unmistakable terminal emulator motif: classic prompt chevron `>` with a luminous cursor block `_` set against a deep obsidian console screen with subtle glass rim highlights and window control accents.
  - Distinctive supersonic aerodynamic speed streaks sculpt the prompt chevron in vibrant electric cyan and radiant azure, symbolizing swift GPU acceleration and the "Celer" identity.
  - Bundled high-resolution multi-size `AppIcon.icns` (16x16 to 1024x1024) and crisp alpha master `icon.png`.

### Fixed

- **macOS Escape Key & Neovim Normal Mode**:
  - Resolved an issue on macOS where pressing the Escape key (especially in Neovim or during IME text input) failed to register or erroneously inserted a whitespace character instead of exiting Insert mode.
  - Added dedicated `ImeCommitAction::Escape` to detect `keyCode == 53` during IME preedit commit and immediately forward the `\x1b` (ESC) byte to the PTY.
  - Handled IME preedit cancellation by forwarding `\x1b` when the user cancels preedit using the Escape key.
  - Implemented 150ms trailing debounce protection (`last_ime_escape`) to prevent duplicate escape sequences from trailing OS keyboard events.
  - Enhanced `translate_key_event` to universally capture `KeyCode::Escape`, raw `\x1b` characters, and `Option + Escape` (`\x1b\x1b`).

## [0.2.11] - 2026-09-28

### Changed & Improved

- **Clean macOS App Switcher & Dock Appearance**:
  - Removed the red `NSDockTile` badge label across workspaces to prevent intrusive and unseemly badge overlays in macOS App Switcher (`Cmd + Tab`) and the macOS Dock.
  - Active workspace identity continues to be cleanly and natively displayed directly below the application icon via `NSProcessInfo.processName` (`CelerTerm (<Workspace Name>)`), as well as on the application window title.

## [0.2.10] - 2026-09-28

### Added

- **Workspace Environment Variables**:
  - Automatically exports `$CELERTERM_WORKSPACE` (workspace name), `$CELER_WORKSPACE` (convenient alias for prompt customizations), and `$CELERTERM_WORKSPACE_ID` (unique identifier) into every spawned shell/PTY session.
  - Dynamically synchronizes host process and child environment variables when switching, creating, or renaming workspaces.
- **macOS Workspace Visibility & Multi-Window Switching**:
  - **Application Switcher (`Cmd + Tab`)**: Sets `NSProcessInfo.processName` to `CelerTerm (<Workspace Name>)` (e.g. `CelerTerm (Backend)`), allowing instant differentiation between multiple workspace windows in macOS `Cmd + Tab`.
  - **Dock Badge (`NSDockTile`)**: Directly displays the active workspace name on the red Dock badge for each CelerTerm window.
  - **Window Title & Mission Control**: Window titles now display `CelerTerm - <Workspace Name>` for clear identification in Mission Control, Dock right-click window menus, and third-party window switchers (AltTab, Raycast).
  - **Application Menu**: Reflects `CelerTerm (<Workspace Name>)` in the macOS top system menu bar.
  - **Direct Window Cycling Shortcut (`Cmd + \``)**: Native macOS window cycling shortcut (`Cmd + Backtick / Tilde`) to seamlessly rotate focus across all active CelerTerm workspace windows.

### Fixed & Improved

- **Smooth Mouse Drag Selection in Neovim & TUI (SGR 1006 / 1002)**:
  - Fixed an issue where dragging to select text in Neovim (`set mouse=a`) or other TUI applications did not update the visual selection smoothly, only snapping to the selection after the mouse button was released.
  - Implemented real-time SGR mouse drag (`\x1b[<32;col;rowM`) and motion reporting in `WindowEvent::CursorMoved` with character cell boundary deduplication to prevent PTY flooding while delivering silky-smooth 60-120 FPS visual updates.
  - Expanded mouse input handling to fully support Left, Middle, and Right mouse buttons and modifier keys, with `Shift` bypass for native terminal clipboard selection.

## [0.2.9] - 2026-09-28

### Added

- **Workspace Distinct Theme & Accent Colors**:
  - Automatically assigns non-colliding accent colors to new and existing workspaces from a curated 20-color Tokyo Night palette (`#7aa2f7`, `#2ac3de`, `#7dcfff`, `#bb9af7`, `#9d7cd8`, `#73daca`, `#b4f9f8`, `#9ece6a`, `#e0af68`, `#ff9e64`, `#f7768e`, `#db4b4b`, `#c0caf5`, `#a9b1d6`, `#9aa5ce`, `#565f89`, `#ff757f`, `#449dab`, `#ff007f`, `#00e5ff`).
  - Added full color and background customization in the Workspace Management modal (`c` key or `[c] Color` button).
  - Terminal canvas dynamically fills with the active workspace's custom background color (or Tokyo Night default `#1A1B26`).
  - Added 2px top accent stripe under the header reflecting the active workspace color, plus a colored bullet dot `●` badge on the workspace indicator.
- **Tab Visual Highlight & Per-Tab Color Customization**:
  - **Active Tab Highlight**: 2px top accent stripe at the top edge of the active tab, rendered in the tab's custom color (or inheriting the active workspace's color), plus a colored bullet dot `●` adjacent to the tab title.
  - **Individual Tab Color Overrides**: Tabs can have custom color overrides to visually categorize tasks (dev, server, database, test, logs).
  - **Inactive Tab Custom Dot**: Inactive tabs with custom colors retain their colored bullet dot `●` for easy scanning, while default tabs remain clean and minimal.
  - **Interactive Tab Color Popover**:
    - Right-click on any tab header to open the color selector popover directly beneath that tab.
    - Keyboard shortcut `Cmd + Shift + T` or `Cmd + Shift + K` on macOS (`Ctrl + Shift + T` or `Ctrl + Shift + K` on Linux) to open color picker for the active tab.
    - Features 20 curated Tokyo Night swatches (navigable via arrow keys or mouse clicks), custom 6-character HEX input with live preview box, `[Enter]` to apply, `[r]` to reset back to inheriting workspace color, and `[Esc]` / click outside to close.
- **Persistence & Backward Compatibility**:
  - Full disk snapshot persistence for workspace and tab colors across application restarts.
  - 100% backward compatible deserialization for legacy snapshots without color fields.

### Fixed & Tested

- **Font Shaping & Coverage**:
  - Verified and confirmed full support for Vietnamese tone marks shaping across all supported font fallbacks including Firple VN.

## [0.2.8] - 2026-09-27

### Fixed & Improved

- **Multi-Window Workspace Synchronization & Deletion Persistence**:
  - Resolved an issue where deleting a workspace in one window was not synchronized to other open windows and could be resurrected when another window saved its snapshot.
  - Sourced authoritative unowned workspace state from disk in `merge_workspace_managers`, eliminating the fallback that restored deleted workspaces.
  - Implemented high-performance mtime-guarded reload on window focus, before toggling the workspace modal, during frame rendering while the modal is open, and before cycling workspaces.
  - Guaranteed clean termination of tab PTY sessions in memory when deleting a workspace.
- **Software Update Modal Sizing & Overflow Protection**:
  - Widened the Software Update dialog from `520px` to `640px` (+23% width) to comfortably accommodate long descriptions, release notes, and footer action buttons across both standard and Retina displays.
  - Added safe UTF-8 character boundary truncation with ellipsis for dynamic strings (`status_text`, `target_line`, `err_line`) to guarantee zero text spillage outside modal bounds.
  - Added boundary clamping for the release date string when rendered adjacent to version info.

## [0.2.7] - 2026-09-27

### Fixed & Improved

- **Vietnamese IME Enter Immediate Execution**:
  - In Vietnamese IME, pressing Enter with active unconfirmed preedit immediately executes the command (commits preedit and sends `\r` directly to the PTY), matching WezTerm behavior so users do not need to press Enter twice when running terminal commands.
  - Redundant trailing `KeyboardInput` Enter events from winit are guarded within 150ms to prevent duplicate command execution.
- **Japanese IME Enter Confirmation**:
  - In Japanese IME (detected via macOS Carbon TIS input source ID and Japanese Unicode character sets: Hiragana, Katakana, Kanji, fullwidth forms), pressing Enter only confirms the preedit/candidate text into the terminal buffer without emitting `\r` or running the command.
- **Shift+Enter Multiline Support (Claude CLI, AGY, etc.)**:
  - Pressing Shift+Enter during active IME composition (Vietnamese or Japanese) commits the text and emits `\n` (newline) instead of `\r`, enabling multiline input without premature prompt submission.
- **Digit Confirmation on IME Commit (Fix for '0' and Numbers)**:
  - Fixed an issue where pressing '0' (or other digits) during unconfirmed Vietnamese preedit swallowed the digit or inserted an unwanted space.
  - Added ASCII digit and keycode detection for all numbers (row & keypad) so typing e.g. `v` + `0` cleanly produces `v0` and `tieng` + `0` produces `tieng0`.

## [0.2.6] - 2026-09-27

### Fixed

- **IME Preedit Confirmation on Enter (Japanese & Vietnamese)**:
  - Standardized Enter key behavior during active IME preedit composition to match WezTerm, iTerm2, and modern terminal conventions: pressing Enter on unconfirmed preedit text now **only confirms (commits)** the text into the terminal buffer.
  - Resolved an issue where pressing Enter to confirm preedit sent a carriage return (`\r`), prematurely executing the command or breaking code lines in `nvim`/`vim`.
  - Added `ImeCommitAction::Confirm` and a dedicated `last_ime_confirm` event guard to suppress subsequent `KeyboardInput` Enter events associated with the preedit confirmation.
  - Pressing Enter a second time after the text is committed cleanly executes the command in the shell.

## [0.2.5] - 2026-09-27

### Added

- **Multi-Window Duplicate Workspace Prevention & Auto-Focus**:
  - Implemented single-instance workspace ownership: prevents opening the same workspace in multiple windows simultaneously.
  - Automatically brings existing window to the front (`focus_instance`) via Cocoa native `NSRunningApplication::activateWithOptions` on macOS and `xdotool`/`wmctrl` on Linux whenever switching to an already-open workspace (via Enter, row click, modal chip, or Next/PreviousWorkspace shortcuts).
  - Opening a workspace in a new window (`w`/`W` key, "window" chip, or CLI `--workspace <name>`) focuses the existing window if it is already open rather than spawning a duplicate window.
  - Launching additional CelerTerm windows without arguments automatically selects the first unoccupied workspace or creates a new `Workspace N`.
  - Added visual badge `● In Window` (sky blue) in the Workspace Modal alongside `● Active` (green) to clearly indicate which workspaces are active in other windows.
  - Protected active workspaces in other windows from accidental deletion.

### Fixed

- **Vietnamese & CJK IME Backspace Deletion**:
  - Fixed an issue where pressing Backspace on unconfirmed/preedit text deleted 2 characters instead of 1.
  - Refactored IME commit action state with `ImeCommitAction::{Append, Backspace, None}` so Backspace confirmation emits only the committed preedit slice without redundant space fallbacks, letting `WindowEvent::KeyboardInput` delete exactly 1 character.
- **Multi-Window Atomic Relaunch on In-App Auto-Update**:
  - Fixed an issue where performing an in-app update with multiple workspace windows open only restarted a single window.
  - Added active instance tracking (`~/.config/celerterm/active_instances.json`) that records running PIDs and their corresponding workspaces.
  - Updated restart script generator to cleanly terminate all other active CelerTerm instances and relaunch every open workspace in its own window (`--workspace <name>`).

## [0.2.4] - 2026-09-27

### Added

- **SSH Remote Process Dynamic Title Indicator (`[🌐command]`)**:
  - Automatically captures ANSI OSC 0/2 window title escape sequences (`Event::Title` / `Event::ResetTitle`) emitted by remote shells (bash, zsh) and active commands (`tail`, `nvim`, `top`, `htop`, etc.) over SSH connections.
  - Displays a dedicated globe indicator on the tab title (e.g. `[🌐tail]`, `[🌐nvim]`, `[🌐~]`, or `[🌐ssh]`), making remote terminal sessions instantly identifiable at a glance.
  - Automatically resets dynamic titles and cleanly restores local folder names upon SSH exit (`exit`).

### Fixed

- **macOS App Nap Prevention**:
  - Disabled macOS App Nap via `NSAppSleepDisabled` and `NSSupportsAppNap` in `assets/Info.plist`.
  - Added programmatic App Nap assertion via `NSProcessInfo.beginActivityWithOptions:reason:` (`NSActivityUserInitiatedAllowingIdleSystemSleep | NSActivityLatencyCritical`) ensuring background PTY readers and event loops keep running when clamshell mode / `caffeinate` is active.
- **PTY EINTR Signal Resilience**:
  - Handled `ErrorKind::Interrupted` (`EINTR`) and `ErrorKind::WouldBlock` in the PTY reader thread loop, preventing premature thread termination and `SIGHUP` kill signals to child processes (`agy`, `claude code`, dev servers) when macOS changes display/power states.
- **Lid Close / Sleep Terminal Grid Resize Protection**:
  - Added guards to `recalculate_grid` to ignore sub-threshold / zero dimensions (`width < 120` or `height < 80`) when displays disconnect or sleep.
  - Enforced minimum grid boundaries (`MIN_COLS = 20`, `MIN_ROWS = 4`), preventing `1x1` SIGWINCH terminal resizes that crash TUI applications (like Bubbletea-based `agy` or Ink-based `claude code`).
- **Vietnamese & CJK IME Backspace Handling**:
  - Handled Backspace (`kVK_Delete` keycode 51, `kVK_ForwardDelete` keycode 117) during IME composition commit in `get_ime_commit_extra`.
  - Automatically consumes and applies the Backspace character (`0x7f`) when confirming uncommitted preedit text, matching standard terminal behavior (like WezTerm) without requiring users to press Backspace twice.
- **Japanese & CJK Preedit Display & Cursor Jitter Fix**:
  - Hid the artificial block cursor during IME preedit composition (matches WezTerm and Alacritty; the cursor only appears once text is confirmed).
  - Used `unicode_width::UnicodeWidthStr::width` for accurate calculation of double-width Japanese Hiragana, Katakana, and Kanji characters in preedit background and underline styling.
  - Fixed candidate popup anchoring coordinate scaling on Retina displays by passing physical pixel coordinates to `winit::dpi::Position::Physical` instead of logical position.
- **Shift+Enter & Real-Time Hardware Modifier Synchronization**:
  - Polled real-time macOS modifier flags via `NSEvent::modifierFlags_class()`, preventing sticky/desynced modifier states across workspace switching shortcuts (`Cmd+Shift+[`, `Cmd+Shift+]`, modal).
  - Enhanced Shift+Enter / Ctrl+Enter key translation to consistently match physical keycodes, named keys, and character representations (`\r`, `\n`).
  - Automatically resets modifier states on window blur (`WindowEvent::Focused(false)`).
- **Display Sleep Surface Recovery**:
  - Handled `WindowEvent::Occluded` to schedule redraws on wake.
  - Added automatic re-initialization of `softbuffer::Surface` in `RedrawRequested` if the macOS graphics backing store is invalidated during sleep/wake cycles.

## [0.2.3] - 2026-09-26

### Added

- **Shift+Enter & Ctrl+Enter Multiline Newline**:
  - Emits line feed `\n` (`0x0A`) for `Shift+Enter` and `Ctrl+Enter`, enabling seamless multiline prompts in modern CLI assistants like **Antigravity CLI (`agy`)**, Claude Code, and REPLs without submitting the command.
  - Added `Alt+Enter` (`\x1b\r`) escape sequence mapping.
  - Added automated unit test coverage `test_shift_and_ctrl_enter_multiline_newline` (69/69 tests passing).
- **macOS Sequoia Local Network Privacy & Ad-hoc Code Signing**:
  - Added `NSLocalNetworkUsageDescription` key to `assets/Info.plist` describing local network permissions for SSH, dev servers, and local tooling.
  - Added ad-hoc bundle code signing (`codesign --force --deep -s -`) to the macOS packaging CI/CD pipeline to ensure persistent TCC privacy database authorization.

### Fixed

- **Login Shell & PATH Environment Bootstrapping (Neovim / Homebrew fix)**:
  - Spawns terminal shells as login shells (`-l`), ensuring shell profile files (`~/.zprofile`, `~/.zshrc`, `~/.bash_profile`) are properly evaluated so Homebrew (`/opt/homebrew/bin`), cargo, and user PATH configurations load completely.
  - Added automatic PATH bootstrapping for GUI-launched app instances, prepending existing system paths (`/opt/homebrew/bin`, `/opt/homebrew/sbin`, `/usr/local/bin`, `~/.cargo/bin`, `~/.local/bin`) if not already present.
  - Guarantees CLI tools like `nvim` (Neovim), `git`, `node`, `bun`, and `cargo` work identically on fresh starts, session restores, and app relaunches.
  - Added unit test coverage for PATH bootstrapping and login shell environment (71/71 tests passing).

## [0.2.2] - 2026-09-26

### Added

- **Interactive Mouse Text Selection & Copy**:
  - Click-and-drag selection (`SelectionType::Simple`), double-click word selection (`SelectionType::Semantic`), triple-click line selection (`SelectionType::Lines`), and Option+drag rectangular block selection (`SelectionType::Block`).
  - Automatic viewport scrolling when dragging past top/bottom screen boundaries.
  - Theme selection highlight rendering (Pass 1 background fill).
  - Native clipboard copy via `Cmd+C` on macOS / `Ctrl+Shift+C` on Linux, and paste via `Cmd+V` / `Ctrl+Shift+V`.
- **macOS Option Word Navigation & Deletion**:
  - Configurable Option+Left arrow (`\x1bb`) and Option+Right arrow (`\x1bf`) word navigation in terminal shells.
  - Option+Delete (`\x17` / `Ctrl+W`) word deletion support.
- **TDD Test Suite Expansion**:
  - Comprehensive unit test coverage for text selection lifecycle, word boundaries, line selection, color parsing, and word navigation keymaps (68/68 tests passing).

### Fixed

- **Multi-Workspace Persistence & Session Activation**:
  - Fixed workspace snapshot overwrite race condition across multiple open windows by merging memory state with disk state before saving.
  - Automatically activate all workspace tabs upon restore so secondary workspaces retain full state across app restarts.

## [0.2.1] - 2026-09-26

### Added

- **In-App Auto-Update & Atomic Relaunch**:
  - Direct download and staging of GitHub release packages within the terminal application (`CelerTerm-macOS.app.zip` for macOS, `celerterm-linux-x86_64.tar.gz` for Linux).
  - Background asynchronous downloading via curl thread, preventing any terminal frame drops or lag.
  - Interactive 3-stage update modal: `[Enter] Update Now` -> `[...] Downloading` -> `[Enter] Restart & Update`.
  - Atomic relaunch script preserving open workspace state and smoothly replacing application binaries.
- **TDD Test Suite Expansion**:
  - Added unit and platform-matching tests for GitHub release asset filters and dynamic update modal states (56/56 tests passing).

## [0.2.0] - 2026-09-26

### Added

- **Tab Reordering Shortcuts**:
  - Move active tab to the left (previous position) with `Cmd + Up` (`Ctrl + Shift + Up` on Linux).
  - Move active tab to the right (next position) with `Cmd + Down` (`Ctrl + Shift + Down` on Linux).
  - Safe boundary clamping at first and last tab slots to prevent accidental wrap-around when cycling rapidly.
  - Automatic real-time tab number updates in the titlebar (`1.`, `2.`, ...) and immediate session snapshot persistence to disk.
- **TDD Test Suite**:
  - Comprehensive unit and regression test coverage for tab reordering, boundary conditions, and keymap translation (56/56 tests passing).

## [0.1.0] - 2026-09-26

### Added

- **Core Terminal Engine**:
  - High-performance, low-latency terminal rendering engine powered by `alacritty_terminal`, `softbuffer`, and `cosmic-text`.
  - Full 24-bit TrueColor RGB support, ANSI 256 colors, and Unicode box-drawing glyphs.
  - Tokyo Night dark theme by default (`#1A1B26`).
- **Tab & Workspace Management**:
  - Integrated titlebar native tabs (`tabs_in_titlebar = true`).
  - Interactive Workspace Management Modal (`Cmd+Shift+O` / `Ctrl+Shift+O`): create, rename, switch, delete workspaces, or spawn them into isolated windows.
  - Workspace state persistence across restarts preserving tab sessions and scrollback history.
- **In-App Menu & Update Checker**:
  - In-app `☰` dropdown menu in the top-right header for quick actions.
  - Interactive "Check for Updates" modal (`Cmd+Shift+U` / `Ctrl+Shift+U`) checking GitHub API (`AMSComm/CelerTerm`) with scrollable release notes and one-click downloads.
- **IME & Input Support**:
  - Native macOS IME composition candidate anchoring for Vietnamese (Telex, VNI) and Japanese input methods.
  - Option-as-Alt configurable support for seamless terminal navigation and Neovim compatibility.
- **Packaging & CI/CD**:
  - Automated GitHub Actions release workflow building macOS `.dmg` / `.app.zip` and Linux `.tar.gz` / `.deb` installers.
  - Desktop entries and high-resolution icons for macOS and Linux.
