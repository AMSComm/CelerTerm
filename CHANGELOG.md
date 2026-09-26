# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

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
