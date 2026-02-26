# Implementation Plan

## Context

The `feat/keys` branch adds arrow key support and fixes key dispatch logic. SECURITY.md identifies two correctness concerns: Backspace and Tab send text payloads (`"\u{0008}"` and `"\t"`) and `Char` CDP events, diverging from Puppeteer's reference implementation which sends neither. PATTERNS.md requires matching Puppeteer's `USKeyboardLayout`.

Confirmed via research: Chrome's `Char` event with `"\u{0008}"` **inserts a literal U+0008 control character** (does not delete). `Char` with `"\t"` **inserts a literal tab character** (does not move focus). This is because `Char` events feed into `insertText()`, bypassing the keyboard command pipeline. The fix (removing text values) causes `RawKeyDown` to be processed through Chrome's command pipeline, triggering the correct native actions.

## Items

### 1. Fix Backspace and Tab text values in `keys.rs`
**Status: TODO**

Change `key_info()` so Backspace (code 8) and Tab (code 9) have `text: ""` instead of `text: "\u{0008}"` and `text: "\t"`. This makes them behave like Escape and arrow keys: `RawKeyDown` + `KeyUp` only, no `Char` event, no text payload. Matches Puppeteer's `USKeyboardLayout` where Backspace and Tab have no `text` property.

**File:** `src/browser/keys.rs` (lines 8-15)

### 2. Add unit tests for `key_info()` in `keys.rs`
**Status: TODO**

Add a `#[cfg(test)] mod tests` block to `keys.rs` with tests that:
- Verify all 8 key mappings return correct `name` and `text` values
- Verify that keys without text (Backspace, Tab, Escape, arrows) have `text: ""`
- Verify that only Enter has non-empty text (`"\r"`)
- Verify unknown codes (e.g., 0, 255) return `None`

These tests directly demonstrate the fix: before the change, assertions for Backspace and Tab `text == ""` would fail; after the fix, they pass.

### 3. Add integration test for Tab key behavior
**Status: TODO**

Create a test that fails with the old Tab text value and passes with the fix.

**Fixture:** `tests/key-press/index.html` — page with two `<input>` fields. The first is focused by default. JavaScript listens for `keydown` on the first input and sets `#result` text when Tab is pressed.

**Spec:** Custom spec that:
- Exports only `PressKey` actions generating code 9 (Tab)
- Extracts `document.activeElement` identity and first input's value
- Asserts `eventually`: the second input has focus (Tab moved focus, not inserted `\t`)

**Why it distinguishes before/after:**
- Before fix: Tab sends `Char` with `"\t"` → literal tab inserted in first input, focus stays → test fails (second input never gets focus)
- After fix: Tab sends only `RawKeyDown` + `KeyUp` → browser navigates focus to second input → test passes

**Files:**
- `tests/key-press/index.html`
- `tests/integration_tests.rs` — new `test_key_press` function
