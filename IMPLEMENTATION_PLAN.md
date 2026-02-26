# Implementation Plan

## Context

The `feat/keys` branch adds arrow key support and fixes key dispatch logic. All planned items are complete.

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
- Comment in `key_info()` explaining `code`/`key` coincidence for current special keys and divergence rules for printable/modifier/numpad keys (SECURITY.md §1)
- Cross-boundary test `keycodes_matches_supported_key_codes` in `random_test.rs` verifying TS `keycodes()` elements match Rust `SUPPORTED_KEY_CODES` (SECURITY.md §2)
