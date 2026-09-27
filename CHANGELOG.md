# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

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
