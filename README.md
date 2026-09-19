# Application Priority Setter

A lightweight Linux desktop application for viewing running applications and changing their CPU priority. The system backend is written in Rust and the interface uses Qt 6/QML through CXX-Qt.

## Current scope

- Groups current-user processes under their main application using systemd application scopes and process-tree ancestry.
- Uses installed desktop-entry icons, with an initial-letter badge when no icon can be found.
- Refreshes automatically every 1–60 seconds (five seconds by default).
- Searches by application name or executable path.
- Applies bounded CPU nice values to every process in an application group.
- Resets a selected application's grouped processes to the normal nice value of 0.
- Optionally saves a priority per application (`~/.config/application-priority-setter/rules.json`, keyed by desktop ID with executable fallback) and re-applies it automatically on every refresh.
- Optional startup entry (Preferences) that starts the app when you log in, using the system autostart folder.
- System tray icon with Show/Hide and Quit, plus an optional close-to-tray mode (Preferences) so closing the window keeps the app running in the background.
- Verifies PID start times before changing priority to prevent PID-reuse mistakes.
- Reports when a higher priority requires authorization.

The application does not run as root. A narrowly scoped Polkit helper for authorized negative nice values is planned separately.

## Build

Requirements include Rust, CMake 3.24+, a C++ compiler, Qt 6 Core/Gui/QML/Quick/QuickControls2, and a linker supported by CXX-Qt.

```bash
./build.sh app
./build-release/application-priority-setter
```

Set `BUILD_JOBS` to control build parallelism, for example `BUILD_JOBS=8 ./build.sh app`.

## Build an Arch package

Plain `./build.sh` defaults to the native Arch package build:

```bash
./build.sh
sudo pacman -U dist/arch/application-priority-setter-0.2.0-1-x86_64.pkg.tar.zst
```

This uses [the included PKGBUILD](packaging/arch/PKGBUILD), writes the package under `dist/arch/`, and does not install or launch it automatically.

## Test the Rust backend

```bash
cargo test --manifest-path rust/Cargo.toml
cargo clippy --manifest-path rust/Cargo.toml --all-targets -- -D warnings
```

# Disclaimer
This project is made with the assistance if AI.
