---
name: rust-github-workflows
description: Before committing changes to the git repository, run commands found in Github Workflows locally so that errors can be identified and corrected before pushing changes to Github 
---

# Rust Github Workflows

## Cargo make

The run commands from `.github/workflows/rust-tests.yml` are extracted by the script in `Makefile.toml`.  Run these commands with:

```bash
cargo make ci
```

Correct any resulting errors.

### Clippy warnings

- A "warning type" is defined by its lint name (e.g. `clippy::collapsible_if`). Group all occurrences of the same lint into a single item, regardless of how many files or lines are affected.
- Only present warnings that occur in files modified in the current session (i.e. files that appear in `git diff HEAD`). If the user asks to address pre-existing warnings in unmodified files, include those as well.
- For each warning type, present:
  - The lint name
  - The total number of occurrences
  - One representative code excerpt, including the compiler's suggested fix
  - A list of all affected `file:line` locations
- Present one warning type at a time and ask whether it should be corrected or ignored.
- Do not suppress a warning with `#[allow(...)]`, `#[expect(...)]`, or `#![allow(...)]` attributes unless the user explicitly requests that that warning should be ignored.

## Workflow

1. Run Cargo make. A step has **errored** if `cargo make ci` exits with a non-zero exit
   code (e.g. a compilation failure or a `cargo fmt --check` diff). Fix all errors and
   re-run Cargo make until it exits successfully. Do not proceed to step 2 until the
   exit code is 0.
2. Present each warning type to the user one-by-one and ask whether it should be
   corrected or ignored. (Warnings are lines prefixed with `warning:` in the output;
   they do not cause a non-zero exit code.)
    - If the user requests that the warning be corrected, fix the code and re-run
      Cargo make. If the re-run exits non-zero, treat it as a new error and return
      to step 1. If new warning types appear in the output, add them to the triage
      queue. Otherwise verify that the original warning is no longer present and
      continue to the next item.
    - If the user requests that the warning be ignored, present the next warning type
      that has not been corrected or ignored.
3. Once every warning type has been corrected or ignored, run Cargo make one final
   time to confirm a clean exit with no remaining warnings in modified files. Report
   the result to the user.
