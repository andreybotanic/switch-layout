# SwitchLayout

Minimal Windows desktop application scaffold on Rust with Slint.

## Requirements

- Rust stable toolchain

## Commands

```powershell
cargo run
cargo run --release
cargo fmt --check
cargo clippy --all-targets --all-features
```

In debug builds, the console stays available for diagnostics.
In release builds, the app runs as a windowed Windows application without a separate console window.
