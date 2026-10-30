# Contributing to MathPrime

Glad you want to help. A few agreements:

## Building and testing

```powershell
cargo test --workspace
cargo run -p rb-gui
```

Every change keeps `cargo test --workspace` green and the release build free of warnings (`cargo build --release` with
`RUSTFLAGS=-D warnings`, as the CI on GitHub does).

If you have Mathcad Prime sheets (.mcdx), this test compares MathPrime with the results Mathcad itself stored:

```powershell
$env:MATHPRIME_MCDX_DIRS = "C:\path\to\sheets"
cargo test -p rb-mcdx -- --nocapture
```

## Rules

- **No code or files from PTC.** Only the open .mcdx format and the documented Automation API (only in `tools/`).
- **No new external crates** without discussion: MathPrime deliberately builds on a small set of dependencies (egui,
  eframe, egui_plot, miniz_oxide, rfd).
- Code and comments are in Dutch; follow the style of the surrounding code. Issues, pull requests and documentation can
  be in English.
- A new user-visible message written in the code goes in Dutch, with its translation in
  `crates/rb-units/src/messages.txt` (a test fails if you forget) or `crates/rb-gui/src/taal.rs` for interface texts.
- A new function gets a test, and a line in `README.md` and `docs/INVENTORY.md`.

## Layout

See the chapter "Layout" in `README.md` for the structure of the crates; the design notes are in `docs/PLAN.md` (Dutch).

## Reporting bugs

Open an issue on GitHub with the worksheet (`.rkn`) or a small example that shows the problem, what you expected and what
MathPrime gave.
