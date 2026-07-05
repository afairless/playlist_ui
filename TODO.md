# Implementation Plan: Search Clear Button (✕)

Source: `docs/research/search-clear-button.md`

| # | Commit message | Logical unit | Key deliverables | Tests |
|---|---|---|---|---|
| 1 | `feat: add SearchCleared message variant` | Message variant | `src/gui/state.rs` — add `SearchCleared` to `Message` enum, update module docstring | — |
| 2 | `feat: handle SearchCleared in update` | Update handler | `src/gui/update.rs` — add `Message::SearchCleared` arm in `update()` | Unit |
| 3 | `feat: add clear button (✕) to search row` | Clear button UI | `src/gui/left_panel.rs` — conditional ✕ button in `create_search_row()` with inline flat style | Smoke |
| 4 | `test: verify full test suite and lint` | Verification | Run `cargo test`, `cargo clippy`, `cargo fmt --check` | — |
