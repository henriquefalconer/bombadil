# Security: feat/keys Key Dispatch Changes

## Risk Summary

The feat/keys branch extends key press support from 2 keys (Enter, Escape) to 8 keys (adding Backspace, Tab, and four arrow keys). The core change makes CDP key dispatch conditional: keys that do not produce character input no longer send `text`/`unmodified_text` fields or the `Char` event. This is a correctness fix — the previous code on develop sent `"\r"` as the text for every key, which prevented native browser behavior (Tab not moving focus, Backspace not deleting, arrows not navigating). Additionally, develop listed Backspace and Tab in the TypeScript `keycodes()` without Rust-side support, causing silent runtime errors.

## Identified Risks

### 1. CDP field conflation (`code` vs `key`)

The `KeyInfo` struct has separate `code` and `key` fields, but every entry sets them to the same value. For the eight currently supported keys this is correct per the CDP specification for named keys, but it creates a template effect: future contributors extending the map to printable characters (code=49 → `code:"Digit1"`, `key:"1"`), modifiers (code=16 → `code:"ShiftLeft"`, `key:"Shift"`), or numpad keys (code=96 → `code:"Numpad0"`, `key:"0"`) may follow the existing pattern and set both fields identically, producing incorrect key events.

**Impact:** Applications that distinguish `event.code` from `event.key` — games, accessibility tools, internationalized UIs, keyboard shortcut handlers — would be fuzzed with incorrect key event values. The existing unit tests verify fields independently but do not flag entries where `code == key` for keys known to require different values.

**Mitigation:** A comment in `key_info()` warns that `code`/`key` identity is coincidental for the current set. This reduces but does not eliminate the template risk.

### 2. Dual-source key code lists

The valid key codes are defined in two places:
- Rust: `SUPPORTED_KEY_CODES` / `key_info()` in `src/browser/keys.rs`
- TypeScript: `keycodes()` in `src/specification/random.ts`

If these drift apart, the fuzzer either generates actions that fail silently at runtime (code in TS but not Rust) or never exercises supported keys (code in Rust but not TS). The runner swallows action errors and retries, so drift goes unnoticed during normal operation.

**Impact:** Reduced fuzzing coverage or wasted cycles on failing actions, with no user-visible signal.

**Mitigation:** A cross-boundary test (`keycodes_matches_supported_key_codes` in `random_test.rs`) asserts that the TypeScript `keycodes()` output matches the Rust `SUPPORTED_KEY_CODES` set. This catches drift at test time. The remaining risk is someone adding to one side without running the full test suite.

## Accepted Risks (Not Requiring Action)

- **Empty string sentinel for no-text keys:** Pragmatic, internal-only, consumed in one place with clear conditional checks. `Option<&'static str>` would be more idiomatic but adds complexity for a private type with a single consumer.
- **`\r` for Enter:** Matches Chromium CDP convention. Already present on develop. Consistent with Puppeteer's `USKeyboardLayout`.
- **No modifier key support:** Known feature gap, not a regression from develop. The current set covers navigation and editing keys relevant to form-heavy fuzzing.
- **No `location` field:** Irrelevant for the current key set (no left/right modifier variants, no numpad keys).
- **`u8` key code range:** Sufficient for all standard virtual key codes (0–255). Pre-existing type choice on develop.
- **Cross-boundary test accesses TypeScript `private` field:** The test reads `generator.elements` from a `From<T>` instance. TypeScript `private` is compile-time only; the property exists at runtime. If `From<T>` renames the field, the test fails — which is the desired behavior, surfacing the contract change.
