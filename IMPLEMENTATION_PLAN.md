# Implementation Plan

## Context

The `feat/keys` branch adds arrow key support and fixes key dispatch logic. Two SECURITY.md mitigations remain incomplete.

## Remaining Items (priority order)

### 1. Add `code`/`key` coincidence comment in `key_info()` (SECURITY.md §1)

**Problem:** Every entry in `key_info()` sets `code` and `key` to the same string. For the current 8 keys this is correct per CDP spec, but the uniform pattern creates a template effect — future contributors adding printable/modifier keys may copy the pattern and produce incorrect events.

**Fix:** Add a comment at the top of the `key_info()` match body noting that `code` and `key` happen to be identical for the current set of special keys and must diverge for printable characters (e.g., code 49 → `code: "Digit1"`, `key: "1"`), modifiers (e.g., code 16 → `code: "ShiftLeft"`, `key: "Shift"`), and numpad keys. This satisfies PATTERNS.md "Cross-Boundary Contracts" §3 and SECURITY.md §1.

**Location:** `src/browser/keys.rs`, inside `key_info()` above the first match arm.

### 2. Add cross-boundary test: TS `keycodes()` ↔ Rust `SUPPORTED_KEY_CODES` (SECURITY.md §2)

**Problem:** The `all_supported_codes_have_key_info` unit test only validates Rust-internal consistency (every `SUPPORTED_KEY_CODES` entry has a `key_info` result). There is no test verifying the TypeScript `keycodes()` array matches `SUPPORTED_KEY_CODES`. If the lists drift, the fuzzer either generates actions that fail silently at runtime or never exercises supported keys.

**Fix:** Add a unit test in `src/specification/random_test.rs` (following the existing `load_random_module` pattern) that:
1. Loads the `random.js` module via `load_bombadil_module` (same as existing tests)
2. Calls `keycodes()` to get the `From<number>` generator object
3. Reads the `.elements` property from the returned object (the `From` class stores its array as `this.elements`)
4. Converts each JS number to `u8` and collects into a sorted `Vec<u8>`
5. Asserts equality with a sorted copy of `SUPPORTED_KEY_CODES`

This approach:
- Follows the established `random_test.rs` pattern (load module directly, no full `Verifier`)
- Is deterministic (reads the array, doesn't sample via `.generate()`)
- Catches drift in either direction (extra codes in TS or in Rust)
- Satisfies PATTERNS.md "Cross-Boundary Contracts" §1: "back the correspondence with an automated check"

**Location:** `src/specification/random_test.rs`, new test function `keycodes_matches_supported_key_codes`.

## Already Complete

- Backspace and Tab text values fixed in `key_info()` (text="" for non-character keys)
- Conditional Char event dispatch in `actions.rs` (skipped for non-text keys)
- Arrow key codes (37-40) added to both Rust `key_info()` and TS `keycodes()`
- Integration test `test_key_press_tab_moves_focus` verifying Tab moves focus
- `KeyInfo.name` split into `code` and `key` fields (CDP correctness)
- `SUPPORTED_KEY_CODES` const added with cross-boundary comment referencing TS `keycodes()`
- `all_supported_codes_have_key_info` unit test guards Rust-internal consistency
- `tests/key-press/index.html` rewritten to minimal fixture style (PATTERNS.md)
- Verbose comment block removed from `test_key_press_tab_moves_focus` (PATTERNS.md)
- Unit test functions renamed to drop redundant `test_` prefix (PATTERNS.md)
