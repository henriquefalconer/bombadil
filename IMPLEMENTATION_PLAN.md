# Implementation Plan

## Context

The `feat/keys` branch adds arrow key support and fixes key dispatch logic. All planned correctness and code-quality items are now complete.

## Already Complete

- Backspace and Tab text values fixed in `key_info()` (text="" for non-character keys)
- Conditional Char event dispatch in `actions.rs` (skipped for non-text keys)
- Arrow key codes (37-40) added to both Rust `key_info()` and TS `keycodes()`
- Integration test `test_key_press_tab_moves_focus` verifying Tab moves focus
- `KeyInfo.name` split into `code` and `key` fields (CDP correctness; SECURITY.md §1)
- `SUPPORTED_KEY_CODES` const added with cross-boundary comment referencing TS `keycodes()`
- `all_supported_codes_have_key_info` unit test guards against Rust↔TS drift (SECURITY.md §2)
- `tests/key-press/index.html` rewritten to minimal fixture style (PATTERNS.md)
- Verbose comment block removed from `test_key_press_tab_moves_focus` (PATTERNS.md)
- Unit test functions renamed to drop redundant `test_` prefix (PATTERNS.md)
