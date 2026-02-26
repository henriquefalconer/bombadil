# Implementation Plan

## Context

The `feat/keys` branch adds arrow key support and fixes key dispatch logic. SECURITY.md identifies two correctness concerns that still need fixes, and PATTERNS.md compliance issues exist in the current code.

The Backspace/Tab text payload fix and basic tests are in place. The remaining work addresses the two open SECURITY.md risks and brings existing code into PATTERNS.md compliance.

## Remaining Items (Priority Order)

### 1. Split `KeyInfo.name` into `code` and `key` fields (SECURITY.md §1)

**Problem:** `KeyInfo` has a single `name` field used for both CDP `.code()` and `.key()` parameters. For the current 8 keys these values happen to be identical, but CDP defines them as distinct concepts (`code` = physical key, `key` = logical key). A future contributor adding printable characters or modifiers would follow the single-field pattern and produce incorrect key events.

**Fix:**
- In `src/browser/keys.rs`: rename `name` to two fields `code: &'static str` and `key: &'static str` in `KeyInfo`
- In `src/browser/actions.rs`: update `.code(info.name).key(info.name)` to `.code(info.code).key(info.key)`
- Update unit tests to assert both fields
- For the current 8 keys, both fields have the same value — this is correct per Puppeteer's `USKeyboardLayout`

### 2. Add cross-validation test for Rust ↔ TypeScript key code lists (SECURITY.md §2)

**Problem:** Valid key codes are defined in two places with no cross-validation: `key_info()` match arms in Rust and `keycodes()` array in TypeScript. Drift causes silent failures — the runner swallows action errors.

**Fix:**
- In `src/browser/keys.rs`: add a `const SUPPORTED_KEY_CODES: &[u8]` array listing all supported codes `[8, 9, 13, 27, 37, 38, 39, 40]`
- Add a unit test `all_supported_codes_have_key_info` that iterates `SUPPORTED_KEY_CODES` and asserts `key_info(code).is_some()` for each
- Add a comment on `SUPPORTED_KEY_CODES` noting that `keycodes()` in `src/specification/random.ts` must match this list, per PATTERNS.md cross-boundary contract rule
- Optionally: add an integration test that exercises `keycodes().generate()` → `PressKey` to verify the full Rust↔TS round-trip

### 3. Fix test fixture HTML to match PATTERNS.md conventions

**Problem:** `tests/key-press/index.html` uses `<!doctype html>`, `<meta charset="UTF-8" />`, `lang="en"`, and self-closing `<input />` tags. PATTERNS.md requires: omit DOCTYPE, meta, lang attributes, and self-closing tags unless the test exercises them.

**Fix:** Rewrite `tests/key-press/index.html` to match existing fixture style (e.g., `console-error/index.html`):
```html
<html>
  <head>
    <title>Key Press</title>
  </head>
  <body>
    <input id="first" type="text" autofocus>
    <input id="second" type="text">
    <div id="result"></div>
  </body>
</html>
```

### 4. Remove multi-line comment block from integration test (PATTERNS.md)

**Problem:** `test_key_press_tab_moves_focus` has an 8-line comment block explaining before/after behavior. PATTERNS.md says multi-line explanations belong in commit messages, not test bodies.

**Fix:** Remove the comment block from the test function body. A one-line comment is acceptable if needed for non-obvious setup, but the test name and spec string are self-explanatory.

### 5. Fix unit test naming in keys.rs (PATTERNS.md)

**Problem:** Unit test functions use `test_` prefix (e.g., `test_backspace_has_no_text`). PATTERNS.md says: "Do not add a redundant `test_` prefix — the `#[test]` attribute already marks functions as tests."

**Fix:** Rename all test functions:
- `test_backspace_has_no_text` → `backspace_has_no_text`
- `test_tab_has_no_text` → `tab_has_no_text`
- `test_enter_has_text` → `enter_has_text`
- `test_escape_has_no_text` → `escape_has_no_text`
- `test_arrow_keys_have_no_text` → `arrow_keys_have_no_text`
- `test_unknown_codes_return_none` → `unknown_codes_return_none`

## Already Complete

- Backspace and Tab text values fixed in `key_info()` (text="" for non-character keys)
- Conditional Char event dispatch in `actions.rs` (skipped for non-text keys)
- Arrow key codes (37-40) added to both Rust `key_info()` and TS `keycodes()`
- Integration test `test_key_press_tab_moves_focus` verifying Tab moves focus (proves fix works; would fail if reverted since `eventually` would violate after 10s)
