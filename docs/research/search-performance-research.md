# Search Performance Research

## Context

The application has a text search feature across the right panel
(playlist), left panel file tree, and left panel tag tree. With a
collection of ~32,000 music tracks, the current implementation causes
the UI to freeze for a noticeable duration on every search keystroke.

## Current Implementation

The search fires on every keystroke via `Message::SearchQueryChanged`,
which synchronously runs **three independent full scans** on the UI
thread:

1. **`recompute_filtered_right_panel_files`** — iterates all 32k
   `right_panel_files`, calling `file_field_matches` on each track's
   metadata fields. Each call lowercases the query string and every
   field value, allocating new `String`s per comparison.
2. **`recompute_filtered_nodes`** — recurses the full file tree.
3. **`recompute_filtered_tag_nodes`** — recurses the full tag tree.

All of this runs inside `iced::update()` on the main thread, blocking
rendering and event handling until the scan completes.

Key pain points:

- **No debouncing** — every keystroke fires a search (typing "prog"
  runs the full search 4 times).
- **Repeated `.to_ascii_lowercase()` allocations** — the query string
  is lowercased separately inside every `file_field_matches` call,
  producing N×M extra `String` allocations (N=fields, M=tracks).
- **Three full passes** — the right panel, file tree, and tag tree
  filters all scan independently with no sharing.
- **Synchronous on the UI thread** — `iced::update()` blocks rendering.

## Investigated Approaches

### 1. Debounce Input

Don't recompute on every keystroke. Wait until the user stops typing
for 150–300ms, then fire the search once.

- **Effort:** Very low
- **Speedup:** 4–10× fewer runs (depends on typing speed)
- **UI stays responsive?** No — still blocks when it does run
- **New dependencies?** No

**Pros:**

- Trivial to implement (store pending query, use `Task::perform` with
  `sleep`)
- Eliminates the "every keystroke triggers 3 scans" problem
- Zero architectural changes

**Cons:**

- Doesn't make the search itself faster — just runs it less often
- Adds slight perceived lag before results appear
- Doesn't help if the user pastes a long query

---

### 2. Pre-Lowercase Query Once

The query string is lowercased independently inside every field check.
Extract it once at the top of each filter function and pass the
pre-lowercased string down.

- **Effort:** Trivial
- **Speedup:** Small allocation savings
- **UI stays responsive?** No
- **New dependencies?** No

**Pros:**

- Dead simple — change a function signature and a few call sites
- Eliminates N×M extra `String` allocations

**Cons:**

- Tiny absolute gain relative to the overall problem
- Doesn't help the tree filters (they have their own lowercasing)

---

### 3. Memoized Filtered Results (Refinement Cache)

Cache the filtered list and only recompute when the query or the
underlying data changes. On each keystroke, check if the new query is a
refinement of the old one (old query is a prefix). If so, filter the
already-filtered list instead of the full 32k. Otherwise, fall back to
a full scan.

- **Effort:** Low
- **Speedup:** Moderate (2nd+ character filters smaller lists)
- **UI stays responsive?** No
- **New dependencies?** No

**Pros:**

- "rock" → "rocki" → "rockin'" would filter 5000 → 500 → 50 items,
  getting faster with each keystroke
- No new dependencies

**Cons:**

- Doesn't help on the first character typed (still scans 32k)
- Must reset when query direction changes (e.g., "rock" → "jazz")
- Adds some complexity for edge cases (deleting characters, etc.)
- Only helps the right panel, not the tree filter scans

---

### 4. Offload Search to Background Thread (Async)

Use `Task::perform` to run the search on a background thread, then send
the filtered results back via `Message`. The UI stays responsive
throughout.

- **Effort:** Medium
- **Speedup:** None in CPU time, but eliminates the freeze
- **UI stays responsive?** **Yes**
- **New dependencies?** No

**Pros:**

- **Solves the freeze** — UI stays interactive even while searching
- Works for all three filter passes
- Contained change: make filter functions async, return via
  `Task::perform`

**Cons:**

- Results arrive asynchronously — `filtered_right_panel_files` could
  briefly show stale results from a previous search until the new ones
  arrive
- The tree filters (`filter_file_node`, `filter_tag_node`) clone/return
  large `FileNode`/`TagTreeNode` trees — need care to avoid expensive
  `Send+Sync` restrictions or deep clones at the async boundary
- Slightly more complex control flow in `update()`

---

### 5. Pre-Compute Lowercase Versions of Metadata Fields

Store pre-lowercased copies of all metadata fields (`creator_lower`,
`album_lower`, `title_lower`, `genre_lower`) in `RightPanelFile`. The
filter then compares already-lowercased strings — no allocations during
search.

- **Effort:** Low
- **Speedup:** Large allocation savings (potentially ~200k `String`
  allocs avoided per search)
- **UI stays responsive?** No
- **New dependencies?** No

**Pros:**

- Eliminates all per-search `.to_ascii_lowercase()` allocations
- No third-party dependencies
- Predictable memory cost (~32k × 4 `String`s)

**Cons:**

- Roughly doubles memory per track for the four string fields
- Requires adding fields to the struct and populating them on load
- Still does O(n) substring scan over 32k items

---

### 6. In-Memory Full-Text Search Index (tantivy)

Build an inverted index over all metadata fields using
[`tantivy`](https://github.com/quickwit-oss/tantivy), the Rust
equivalent of Lucene. Queries become index lookups — O(1) or O(log n).

- **Effort:** High
- **Speedup:** Huge (indexed)
- **UI stays responsive?** Yes
- **New dependencies?** `tantivy`

**Pros:**

- Sub-millisecond search on 32k tracks even with substring/fuzzy
  queries
- Supports advanced queries (fuzzy matching, prefix, phrase queries)

**Cons:**

- Heavy dependency — adds significant compile time and binary size
- Index must be rebuilt/updated when the right panel changes
- Overkill for 32k items
- Thread-safety and lifetime management with iced

---

### 7. Simple Reverse Word Index

Build your own lightweight inverted index:
`HashMap<String, Vec<usize>>` mapping each lowercased word → indices
into `right_panel_files`. Search splits the query into words and
intersects the postings lists.

- **Effort:** Medium
- **Speedup:** Huge (for word-based queries)
- **UI stays responsive?** Yes (index lookup is fast)
- **New dependencies?** No (just `std::collections`)

**Pros:**

- Very fast — hash lookups instead of 32k scans
- Tiny dependency footprint
- Easy to understand and maintain

**Cons:**

- Doesn't handle substring matching natively (a query for "rock" won't
  match "progressive rock")
- Index must be updated when tracks are added/removed
- Intersection complexity for multi-word queries

---

### 8. N-gram (Trigram) Index

Index every 3-character substring (trigram) of each lowercased field.
A query like "rock" is decomposed into `["roc", "ock"]`, and the
trigram postings lists are intersected. This gives substring search
from an index.

- **Effort:** Medium–High
- **Speedup:** Huge (any substring query)
- **UI stays responsive?** Yes
- **New dependencies?** No

**Pros:**

- Truly fast substring search — no linear scan
- Hash lookups + set intersection
- Works well for 32k items
- Can be built with `HashMap<String, Vec<usize>>` — no deps

**Cons:**

- More complex than the simple word index
- Index size is ~3× the text size (every 3-char window becomes a key)
- 1- and 2-character queries need a separate fallback or a unigram
  / bigram index too
- Index must be kept in sync when the right panel changes

---

### 9. Use the Existing Sled Database

You already have `sled_store` in the app (used for tag tree persistence).
Sled is an embedded key-value store. Write a secondary index:
key = lowercased word, value = set of file paths or indices.

- **Effort:** Medium
- **Speedup:** Large (word lookups)
- **UI stays responsive?** Yes (sled reads are fast)
- **New dependencies?** No (already present)

**Pros:**

- No new dependency — `sled` is already in `Cargo.toml`
- Persistent — survives restarts
- Thread-safe

**Cons:**

- Sled doesn't natively support substring or prefix search — must be
  implemented on top
- Write overhead when tracks are added
- Slower than an in-memory `HashMap` for this scale

---

### 10. Virtual Scrolling / Paginated Results

Only compute the first N matches (e.g. 200) and show a "load more"
option. Stops the filter early once the threshold is reached.

- **Effort:** Low
- **Speedup:** Reduces per-search work significantly
- **UI stays responsive?** No (still blocks, but finishes faster)
- **New dependencies?** No

**Pros:**

- Drastically reduces the work per search
- No indexing needed
- The UI never has to render 32k rows either

**Cons:**

- Users might miss tracks that are deep in the results
- Doesn't help if the user wants to "select all matching"
- Only helps the right panel, not the left panel filtering

---

### 11. Combined: Debounce + Background Thread + Pre-Lowercased Fields

My recommended layered combination:

- **Layer 1** — Pre-compute lowercased metadata fields in
  `RightPanelFile` (approach #5)
- **Layer 2** — Debounce input with a 150ms delay (approach #1)
- **Layer 3** — Offload the filter to a background async task
  (approach #4)

- **Effort:** Medium
- **Speedup:** Large allocation savings + fewer runs + non-blocking
- **UI stays responsive?** **Yes**
- **New dependencies?** No

**Pros:**

- Directly addresses all three causes of the freeze (allocation spam,
  repeated runs, main-thread blocking)
- No new dependencies
- Architectural changes are contained to `update.rs`
- Each layer is independently useful and can be shipped incrementally

**Cons:**

- More implementation work than any single approach alone
- Async boundary for tree structures needs careful handling (Send+Sync)

## Summary

| # | Approach | Effort | Speedup | Responsive? | New deps? |
|---|---|---|---|---|---|
| 1 | Debounce input | Very low | 4–10× fewer runs | No | No |
| 2 | Pre-lowercase query | Trivial | Small alloc savings | No | No |
| 3 | Memoized refinement | Low | Moderate (2nd+ char) | No | No |
| 4 | Background thread | Medium | None in CPU time | **Yes** | No |
| 5 | Pre-lowercased fields | Low | Large alloc savings | No | No |
| 6 | tantivy FTS | High | Huge (indexed) | Yes | `tantivy` |
| 7 | Word index | Medium | Huge (word queries) | Yes | No |
| 8 | Trigram index | Medium–High | Huge (any substring) | Yes | No |
| 9 | Sled FTS index | Medium | Large | Yes | Already present |
| 10 | Virtual scrolling | Low | Reduces work per render | No | No |
| **11** | **Debounce + BG + Pre-lowercase** | **Medium** | **Big + responsive** | **Yes** | **No** |
