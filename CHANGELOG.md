# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.2.3] - 2026-09-26

### Added

- **Shift+Enter & Ctrl+Enter Multiline Newline**:
  - Emits line feed `\n` (`0x0A`) for `Shift+Enter` and `Ctrl+Enter`, enabling seamless multiline prompts in modern CLI assistants like **Antigravity CLI (`agy`)**, Claude Code, and REPLs without submitting the command.
  - Added `Alt+Enter` (`\x1b\r`) escape sequence mapping.
  - Added automated unit test coverage `test_shift_and_ctrl_enter_multiline_newline` (69/69 tests passing).
- **macOS Sequoia Local Network Privacy & Ad-hoc Code Signing**:
  - Added `NSLocalNetworkUsageDescription` key to `assets/Info.plist` describing local network permissions for SSH, dev servers, and local tooling.
  - Added ad-hoc bundle code signing (`codesign --force --deep -s -`) to the macOS packaging CI/CD pipeline to ensure persistent TCC privacy database authorization.

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
