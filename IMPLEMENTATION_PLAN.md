# Implementation Plan

## Context

The `feat/keys` branch adds arrow key support and fixes key dispatch logic. SECURITY.md identifies two correctness concerns: Backspace and Tab send text payloads (`"\u{0008}"` and `"\t"`) and `Char` CDP events, diverging from Puppeteer's reference implementation which sends neither. PATTERNS.md requires matching Puppeteer's `USKeyboardLayout`.

Confirmed via research: Chrome's `Char` event with `"\u{0008}"` **inserts a literal U+0008 control character** (does not delete). `Char` with `"\t"` **inserts a literal tab character** (does not move focus). This is because `Char` events feed into `insertText()`, bypassing the keyboard command pipeline. The fix (removing text values) causes `RawKeyDown` to be processed through Chrome's command pipeline, triggering the correct native actions.

## Status: COMPLETE

All three implementation items have been completed:
1. Fixed Backspace and Tab text values in `src/browser/keys.rs`
2. Added unit tests for `key_info()` in `keys.rs`
3. Added integration test for Tab key behavior in `tests/integration_tests.rs`

The branch is ready for merging to main.
