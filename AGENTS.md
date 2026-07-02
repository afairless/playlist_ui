# AGENTS.md — Conventions for AI-Assisted Development

## Project Overview

Playlist UI is a cross-platform desktop application for browsing, filtering, and
exporting playlists from local audio files. It follows the
[iced](https://github.com/iced-rs/iced) Elm architecture: a single `FileTreeApp`
model, a `Message` enum for all user actions, a pure `update()` function, and a
`view()` function that composes the UI tree.

## Build and Test Commands

```sh
# Build
cargo build
cargo build --release

# Run
cargo run --release

# Test (all)
cargo test

# Test (single)
cargo test <test_name>

# Lint
cargo clippy

# Format check
cargo fmt --check

# CI (requires cargo-make)
cargo make ci
```

The project uses Rust **edition 2024** and requires a **stable** toolchain.

## Code Conventions

### Formatting

- `max_width = 80` (column limit)
- `tab_spaces = 4` (spaces, not hard tabs)
- Imports are grouped `StdExternalCrate` with `Crate` granularity
- Reorder imports (`reorder_imports = true`)
- Wrap comments and format code in doc comments

Run `cargo fmt` before every commit.

### Naming

- Follow standard Rust naming: `snake_case` for functions/variables,
  `UpperCamelCase` for types, `SCREAMING_CASE` for constants
- Module-level items use `pub(crate)` visibility unless they need `pub`
- Private helper functions use `pub(crate)` or no visibility qualifier

### Documentation

- Every **public-facing** function and **non-trivial internal** function gets a
  docstring (`///`) answering: what it does, what it returns, and edge cases.
- Every module (every `.rs` file except thin `mod.rs` wrappers) has a
  **module-level docstring** (`//!`) explaining the module's purpose and its
  public API.
- The `//!` comment at the top of each module should list the public symbols
  under a "Public API:" section.
- Inline comments explain **why**, not **what** — the code expresses the what.

### Testing

- Tests are written inline (`#[cfg(test)] mod tests { ... }`) in each source
  file, following the Arrange-Act-Assert pattern.
- The project follows the **None-One-Many** principle for parameter coverage:
  empty input, single-element input, multi-element input.
- Test function names follow the pattern `test_<module>_<scenario>` or
  descriptive snake_case names.

### Error Handling

- Use `unwrap()` / `expect()` sparingly and only when failure is truly
  unrecoverable (e.g., sled/corruption, bincode serialisation of developer-
  controlled data).
- Use `map_err` / `ok` / `warn!` logging for recoverable errors (e.g., tag
  parsing failures, metadata extraction failures on corrupt files).
- `sled::Error` and `std::io::Error` are the most common error types.

### Safety

- No `unsafe` code.
- The project uses `lofty` for audio metadata parsing — malformed files are
  handled gracefully (returns default metadata).
- File paths read from the filesystem are user-controlled — never executed.
- All user-facing paths are opened/displayed via OS-standard mechanisms
  (`xdg-open`, `open`, `cmd /C start`).

## Project Structure

```
src/
├── main.rs                 — Entry point: DB init, tag tree build, iced launch
├── utils.rs                — format_duration(), tests
├── gui/                    — Elm-architecture UI
│   ├── mod.rs              — Re-exports
│   ├── state.rs            — FileTreeApp model, Message enum, types
│   ├── view.rs             — Layout composition, style structs
│   ├── update.rs           — Pure state transitions
│   ├── left_panel.rs       — Left sidebar assembly
│   ├── right_panel.rs      — Right panel assembly
│   └── render_node.rs      — Recursive tree rendering + highlight logic
├── fs/                     — Filesystem operations
│   ├── mod.rs              — Re-exports
│   ├── file_tree.rs        — FileNode + scan_directory()
│   ├── media_metadata.rs   — MediaMetadata + tag tree builders
│   ├── media_metadata_async.rs — [Experimental, not wired]
│   └── xspf.rs             — XSPF export
└── db/
    ├── mod.rs              — Re-exports
    └── sled_store.rs       — SledStore for tag tree persistence
```

## Data Flow (Elm Architecture)

1. **Model**: `FileTreeApp` holds all state (file trees, tag trees, playlist,
   sort settings, expansions, extensions).
2. **Message**: A `Message` variant represents every user action (click,
   toggle, add, remove, sort, export, etc.).
3. **Update**: `update(&mut FileTreeApp, Message) -> Task<Message>` is a pure
   state transition. Side effects (file dialogs, export) return `Task` objects.
4. **View**: `view(&FileTreeApp) -> Element<Message>` composes the UI tree
   from the current state. No mutations.

## Doing Work

When adding a feature or fixing a bug:

1. Read the relevant source files and understand the Elm-architecture flow.
2. Write tests first (or add to existing test modules).
3. Implement the change in small, compilable increments.
4. Run `cargo test` and `cargo clippy` before committing.
5. Update module-level docstrings if the public API surface changes.

## Common Pitfalls

- **Sled database backward compatibility**: `TagTreeNode` derives `bincode::Decode`
  which reads fields positionally. Adding a new field will invalidate old
  persisted blobs. Use `#[serde(default)]` for serde and accept data loss for
  bincode, or manually implement `Decode`.
- **iced 0.13 API**: The project uses iced 0.13. Some widget APIs differ from
  earlier versions — check the iced 0.13 docs before using widgets.
- **Context menus**: `iced_aw::ContextMenu` closures capture state by value.
  Clone paths inside the closure to avoid move issues.
- **Async code**: The `media_metadata_async.rs` module exists but is **not
  wired into the update path**. Do not refactor sync code to async unless the
  async path is also connected.
