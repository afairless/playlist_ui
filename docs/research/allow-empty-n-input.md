# Allow Empty Input in N Text Field

## Problem

The `N:` text input on the left panel's top row rejects any non-integer input.
When the user erases all digits (completely backspacing/deleting the field
contents), the empty string `""` fails `parse::<usize>()` and the field resets
to the last valid value (e.g., `"6"`). This means the last digit can never be
deleted — the user cannot clear the field to type a new number from scratch.

## Current Behaviour

The `RandomCountChanged` handler in `src/gui/update.rs`:

```rust
Message::RandomCountChanged(new_text) => {
    if let Ok(n) = new_text.parse::<usize>() {
        if n > 0 {
            app.random_count = n;
            app.random_count_input = new_text;
        } else {
            // 0 is not a positive integer — revert
            app.random_count_input = app.random_count.to_string();
        }
    } else {
        // Not a valid integer — revert
        app.random_count_input = app.random_count.to_string();
    }
    Task::none()
},
```

When `new_text` is `""`, `"".parse::<usize>()` returns `Err`, so the code
hits the second `else` branch: `random_count_input` is reset to
`random_count.to_string()`. The field "bounces back" to the old value,
preventing the last digit from ever being erased.

## Proposed Fix

Treat the empty string as a special case — allow it to overwrite
`random_count_input` while keeping the last valid `random_count` value intact:

```rust
Message::RandomCountChanged(new_text) => {
    if new_text.is_empty() {
        // Allow empty input so the user can clear the field and type anew
        app.random_count_input = new_text;
    } else if let Ok(n) = new_text.parse::<usize>() {
        if n > 0 {
            app.random_count = n;
            app.random_count_input = new_text;
        } else {
            // 0 is not a positive integer — revert
            app.random_count_input = app.random_count.to_string();
        }
    } else {
        // Not a valid integer — revert
        app.random_count_input = app.random_count.to_string();
    }
    Task::none()
},
```

### Design Detail

- When the field becomes empty (`""`), `random_count_input` stores `""` — the
  text input widget displays an empty field.
- `random_count` retains its **previous valid value** (e.g., `6`), so any
  random-add operation still uses that number.
- When the user starts typing a new number, the `parse::<usize>()` branch
  fires and updates both fields normally.
- No downstream code needs changing — every place that reads `app.random_count`
  gets the last valid number even while the field is blank, and every place
  that reads `app.random_count_input` displays whatever the user has typed.

### Behaviour Summary

| User action | `random_count_input` | `random_count` |
|---|---|---|
| Type "3" | `"3"` | `3` |
| Backspace once | `""` | `3` (unchanged) |
| Type "4" | `"4"` | `4` |
| Type "0" | resets to `"4"` (0 rejected) | `4` (unchanged) |
| Type "abc" | resets to `"4"` | `4` (unchanged) |
| Erase all, leave empty | `""` | `4` (unchanged) |
| Type "12" | `"12"` | `12` |

## Affected Files

| File | Change |
|------|--------|
| `src/gui/update.rs` | Add `new_text.is_empty()` early-return in `RandomCountChanged` handler |
| `src/gui/update.rs` (tests) | Update `test_random_count_empty_reverts` → `test_random_count_empty_accepted`; add `test_random_count_empty_keeps_previous_count` |

## Implementation Steps

### Step 1 — Modify the RandomCountChanged handler

**File:** `src/gui/update.rs`

Insert an `if new_text.is_empty()` branch at the top of the
`RandomCountChanged` arm that stores the empty string into
`random_count_input` and skips the parse.

**Verification:** `cargo check` passes.

### Step 2 — Update existing tests and add new ones

**File:** `src/gui/update.rs`

1. **Rename** `test_random_count_empty_reverts` to
   `test_random_count_empty_accepted` and update its assertion: an empty
   string should now set `random_count_input` to `""` (not revert it).
   Add a doc comment (`///`) describing the new behaviour.

2. **Add** `test_random_count_empty_keeps_previous_count` — verify that
   when `random_count_input` becomes `""`, `random_count` retains its
   previous value. Arrange: set `random_count` to 6 and
   `random_count_input` to `"6"`, send `RandomCountChanged("")`, assert
   `random_count_input == ""` and `random_count == 6`.

3. **Add** `test_random_count_erase_then_type` — simulate the full user
   workflow: start with `"6"`, erase to `""`, then type `"42"`. After the
   empty state, `random_count` should still be 6; after typing `"42"`, both
   fields should be `"42"` / `42`.

**Verification:** `cargo test` passes with the new and updated tests.

### Step 3 — Lint and format

```sh
cargo fmt --check
cargo clippy
cargo test
```

## Edge Cases

- **Empty field + user presses a random-add button**: `random_count` still
  holds the last valid number, so the operation uses that number. No crash,
  no weird behaviour.
- **User opens the app, immediately erases the default "6"**: `random_count`
  is still `6` from initialisation. The field shows blank. Typing a new
  number works.
- **Empty field + user pastes invalid text**: The paste fires
  `RandomCountChanged("abc")` (or whatever pasted string). It's not empty,
  and it fails `parse::<usize>()`, so the field reverts to the last valid
  value. No regression.
- **Tab away from empty field / focus loss**: The field stays empty, because
  there's no "commit" event in iced's `text_input` — every keystroke fires
  `on_input`. The empty state is permanent until the user types something.
  This is fine: the blank field is a valid editing state, and any random-add
  operation still works because `random_count` is unchanged.
