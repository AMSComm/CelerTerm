# CelerTerm

<p align="center">
  <strong>A high-performance minimalist terminal emulator built in pure Rust.</strong>
</p>

<p align="center">
  <a href="https://github.com/AMSComm/CelerTerm/releases"><img src="https://img.shields.io/github/v/release/AMSComm/CelerTerm?style=flat-square" alt="Release"></a>
  <a href="https://github.com/AMSComm/CelerTerm/actions"><img src="https://img.shields.io/badge/CI-passing-brightgreen?style=flat-square" alt="CI"></a>
  <a href="https://www.rust-lang.org"><img src="https://img.shields.io/badge/Rust-2024_Edition-orange?style=flat-square" alt="Rust"></a>
  <a href="LICENSE"><img src="https://img.shields.io/badge/license-MIT-blue?style=flat-square" alt="License"></a>
</p>

---

## ✨ Features

- **⚡ Blazing Fast & Lightweight**: Zero runtime bloat, powered by `alacritty_terminal`, `softbuffer`, and `cosmic-text` text shaping.
- **🎨 Tokyo Night Aesthetics**: Beautiful dark theme default (`#1A1B26`), full 24-bit TrueColor support, ANSI 256 colors, and crisp box-drawing glyphs.
- **🗂 Multi-Workspace & Native Tabs**:
  - Integrated titlebar tabs (`tabs_in_titlebar = true`).
  - Workspaces manager (`Cmd+Shift+O` / `Ctrl+Shift+O`): group tabs into named sessions, rename, delete, switch, or open in new windows.
  - Automatic snapshot persistence across app restarts with scrollback preservation.
- **🔄 In-App Updates & Quick Menu**:
  - Top-right `☰` App Menu with fast access to updates, workspace manager, config reloading, and repository links.
  - Interactive "Check for Updates" modal (`Cmd+Shift+U` / `Ctrl+Shift+U`) with scrollable release notes and one-click GitHub downloads.
- **🇻🇳 First-Class IME Composition**: Native macOS IME composition window anchoring for Vietnamese (Telex, VNI) and Japanese input methods.
- **⌨️ Intuitive Shortcuts**: Standard macOS and Linux key mappings for instant productivity.

---

## ⌨️ Keyboard Shortcuts

| Action | macOS Shortcut | Linux / Windows Shortcut |
| :--- | :--- | :--- |
| **New Tab** | `Cmd + T` | `Ctrl + Shift + T` |
| **Close Tab** | `Cmd + W` | `Ctrl + Shift + W` |
| **Select Tab (1..9)** | `Cmd + 1` .. `Cmd + 9` | `Alt + 1` .. `Alt + 9` |
| **Previous Tab** | `Cmd + Left` or `Cmd + Shift + [` | `Ctrl + PageUp` |
| **Next Tab** | `Cmd + Right` or `Cmd + Shift + ]` | `Ctrl + PageDown` |
| **Workspace Manager** | `Cmd + Shift + O` | `Ctrl + Shift + O` |
| **New Workspace** | `Cmd + Shift + N` | `Ctrl + Shift + N` |
| **Check for Updates** | `Cmd + Shift + U` | `Ctrl + Shift + U` |
| **Reload Config** | `Cmd + Shift + R` | `Ctrl + Shift + R` |
| **Copy Selection** | `Cmd + C` | `Ctrl + Shift + C` |
| **Paste Clipboard** | `Cmd + V` | `Ctrl + Shift + V` |
| **Clear Screen** | `Cmd + K` | `Ctrl + L` |
| **Reset Terminal** | `Cmd + Alt + K` | `Ctrl + Shift + K` |

---

## ⚙️ Configuration

CelerTerm configuration is stored in `~/.config/celerterm/config.toml` (or `~/Library/Application Support/celerterm/config.toml` on macOS).

```toml
[font]
family = "JetBrainsMono Nerd Font"
fallback_families = ["Menlo", "Monaco", "Courier New"]
size = 13.0
line_height = 1.2
ligatures = true

[window]
padding_x = 10.0
padding_y = 6.0
hide_traffic_lights = false
tabs_in_titlebar = true

[workspace]
restore_on_startup = true
save_scrollback = true
max_scrollback_lines = 1000

[macos]
option_as_alt = true
```

---

## 🚀 Building from Source

### Prerequisites

- [Rust](https://rustup.rs/) (version 1.85+ recommended for Rust 2024 edition).
- macOS: Xcode Command Line Tools.
- Linux: standard development packages (`libfontconfig1-dev`, `libxkbcommon-dev`, `libxcb-xfixes0-dev`).

### Build & Run

```bash
# Clone the repository
git clone https://github.com/AMSComm/CelerTerm.git
cd CelerTerm

# Run tests
cargo test

# Build and run release binary
cargo run --release
```

---

## 📦 Releases & Downloads

Pre-built binary packages and installers are available on the [Releases page](https://github.com/AMSComm/CelerTerm/releases):

- **macOS**: `CelerTerm-macOS.dmg` and standalone `CelerTerm.app.zip`.
- **Linux**: `celerterm-linux-x86_64.tar.gz` and Debian package `.deb`.

---

## 📄 License

CelerTerm is licensed under the [MIT License](LICENSE).
