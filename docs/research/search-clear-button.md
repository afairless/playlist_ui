# Search Clear Button (✕)

Add a small "✕" button inside the search box, on the right-hand side. When
clicked, it clears the current search query.

## Motivation

Users currently have to manually delete the search text (select-all + Backspace)
to clear a search. A dedicated clear button provides a single-click escape hatch
that matches a well-known UX pattern (browser address bars, mobile search
fields, etc.).

## Current Implementation

The search box is built in `create_search_row()` (src/gui/left_panel.rs, lines
313–337). It creates a `text_input` placeholder and a mode-toggle button inside
a `row`:

```rust
let search_input = text_input::<Message, iced::Theme, iced::Renderer>(
    "Search...",
    &app.search_query,
)
.on_input(Message::SearchQueryChanged);

let mode_button = button(text(mode_label).size(menu_style.text_size))
    .on_press(Message::ToggleSearchMode);

row![search_input, mode_button].spacing(menu_style.spacing).into()
```

- `Message::SearchQueryChanged(String)` is the only message wired to the input
- `app.search_query` stores the current query string
- Sending `SearchQueryChanged(String::new())` clears the query and triggers
  `recompute_filtered_*` calls in the update function

## Constraints (from AGENTS.md & codebase)

- **iced 0.13 API**: Widget APIs may differ from earlier versions
- **Elm architecture**: new messages go in the `Message` enum, handled in
  `update()`, rendered in `view()`/helper functions
- **Module docstrings**: update module-level docstrings if the public API
  surface changes
- **Tests**: write tests first (inline `#[cfg(test)] mod tests { .. }`),
  follow Arrange-Act-Assert, use None-One-Many principle
- **Column limit**: 80 characters, 4-space indentation
- **No unsafe**, no blocking errors

## Design Decisions

### New Message vs. Reusing `SearchQueryChanged`

| Option | Pros | Cons |
|--------|------|------|
| **New `Message::SearchCleared`** | Explicit intent, easy to test, no side-effect confusion | One more variant in the enum, one more arm in `update()` |
| **Reuse `SearchQueryChanged("")`** | Zero new code in the update path | No semantic distinction; harder to add future clear-specific side effects |

**Chosen: New `Message::SearchCleared`** — keeps the codebase consistent with
how other discrete actions are modelled (e.g. `ClearRightPanel` is its own
message, not a `vec![]` variant). It makes testing and future enhancements
(e.g. animation, focus management) straightforward.

### Button Placement

The "✕" sits **between the search input and the mode-toggle button**, inside
the same `row`. It is rendered as a small clickable widget.

- If `search_query` is empty, the button is omitted or visually hidden
  (conditional rendering).
- The button uses the same "flat" style as other UI buttons (no background,
  white text).

## Implementation Plan

### Step 1: Add `Message::SearchCleared` variant and update docstring

**File:** `src/gui/state.rs`

Add a new variant to the `Message` enum:

```rust
pub enum Message {
    // … existing variants …
    SearchCleared,
}
```

Update the module-level docstring's "Public API:" section — add
`SearchCleared` to the `Message` bullet list in the `//!` docstring.

### Step 2: Handle `SearchCleared` in the update function

**File:** `src/gui/update.rs`

Add a new arm in the `match message` block inside `update()`:

```rust
Message::SearchCleared => {
    app.search_query = String::new();
    app.filtered_root_nodes = recompute_filtered_nodes(app);
    app.filtered_tag_tree_roots = recompute_filtered_tag_nodes(app);
    app.filtered_right_panel_files =
        recompute_filtered_right_panel_files(app);
    Task::none()
},
```

This is identical to `SearchQueryChanged("")` but expressed explicitly.

### Step 3: Add the "✕" button to the search row

**File:** `src/gui/left_panel.rs`, function `create_search_row()`

Import `button` from `iced::widget` (already imported). In the search row,
conditionally append a small "✕" button between the input and the mode button:

```rust
let clear_button = if !app.search_query.is_empty() {
    Some(
        button(text("✕").size(menu_style.text_size))
            .on_press(Message::SearchCleared)
            .into(),
    )
} else {
    None
};

// Original: row![search_input, mode_button]
// Updated:  row with optional clear_button
let mut children = vec![search_input.into()];
if let Some(btn) = clear_button {
    children.push(btn);
}
children.push(mode_button.into());

row(children).spacing(menu_style.spacing).into()
```

The `text("✕")` uses a Unicode multiplication-sign-like character, common in
UI clear buttons. It inherits the menu text size and the flat-button style
(already used elsewhere in the left panel via `flat_button_style` passed down
by `create_left_panel`).

**Design note:** The existing `create_search_row` does not receive
`flat_button_style`. We could either:

- (a) Add `flat_button_style` as a parameter — more wiring, but consistent
  with other functions
- (b) Use `button::Style { background: None, .. }` inline — simpler
- (c) Build the button with `style(flat_button_style)` and thread the param

**Recommended: (c)** — thread `flat_button_style` through to
`create_search_row`. This keeps the visual style consistent with the rest of
the left panel. Update the caller in `create_left_panel` (line 761) to pass
it.

If the threading is too invasive for one call, fall back to (b) — a dedicated
closure or a simple `background: None` style override.

### Step 4: Write/update tests

**Files:** `src/gui/left_panel.rs` and `src/gui/update.rs`

> **Note:** The existing `test_create_search_row_*` tests call
> `create_search_row(app, menu_style)`. When the function gains the
> `flat_button_style` parameter, these tests must be updated to pass a
> matching closure or placeholder.

#### `src/gui/left_panel.rs` — `create_search_row` tests

- **`test_create_search_row_clears_button_present_with_query`**: Set up an
  app with a non-empty `search_query`, call `create_search_row`, assert no
  panic.
- **`test_create_search_row_clears_button_absent_when_empty`**: Set up an
  app with an empty `search_query`, call `create_search_row`, assert no
  panic. (The button simply isn't rendered; no crash.)

(We cannot easily inspect widget trees in iced tests, so these are smoke
tests — no panic means the conditional rendering is wired correctly.)

#### `src/gui/update.rs` — `update` test

- **`test_search_cleared`**: Set `app.search_query = "something"`, send
  `Message::SearchCleared`, assert `app.search_query == ""`.

### Step 5: Run full test suite and linter

```sh
cargo test
cargo clippy
cargo fmt --check
```

Verify no regressions.

## Edge Cases

- **Empty query + user clicks ✕**: The button is not rendered when
  `search_query` is empty (conditional check). No-op, no message sent.
- **Rapid double-click on ✕**: Sends `SearchCleared` twice. Second dispatch
  is a no-op (query already empty, recomputations are idempotent).
- **Long query + narrow window**: The ✕ character is a single glyph. No
  wrapping concerns inside a row. If the row becomes crowded, the mode label
  could be truncated; that's pre-existing and not affected by this change.
- **Accessibility**: `text("✕")` has no alt-text. iced 0.13 does not have
  built-in ARIA support. Acceptable for a desktop app at this stage.

## Files Changed

| File | Change |
|------|--------|
| `src/gui/state.rs` | Add `Message::SearchCleared`, update module docstring |
| `src/gui/update.rs` | Add `SearchCleared` arm in `update()` |
| `src/gui/left_panel.rs` | Add clear button to `create_search_row()`, thread `flat_button_style` |
| `src/gui/update.rs` (tests) | Add `test_search_cleared` |
| `src/gui/left_panel.rs` (tests) | Add smoke tests for clear-button presence/absence |
