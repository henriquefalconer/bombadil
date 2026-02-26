# Security: feat/keys Key Dispatch Changes

## Risk Summary

The feat/keys branch extends key press support from 2 keys (Enter, Escape) to 8 keys (adding Backspace, Tab, and four arrow keys). The core change is making the CDP key dispatch conditional: keys that do not produce character input no longer send `text`/`unmodified_text` fields or the `Char` event. This is a correctness fix — the previous code on develop sent `"\r"` as the text for every key, and also listed Backspace and Tab in the TypeScript `keycodes()` without Rust-side support.

## Identified Risks

### 1. CDP field conflation (`code` vs `key`)

The `KeyInfo` struct has separate `code` and `key` fields, but every entry sets them to the same value. For the eight currently supported keys this is correct, but it creates a template that will produce wrong CDP events when the map is extended to keys where `code` and `key` diverge (printable characters, modifiers, numpad).

**Impact:** Applications that distinguish `event.code` from `event.key` (games, accessibility tools, internationalized UIs) would be fuzzed with incorrect key event values.

**Mitigation path:** Add a comment in `key_info()` noting that `code` and `key` happen to be identical for the current set and must diverge for printable/modifier keys. Alternatively, add a compile-time or test-time check that flags entries where `code == key` for keys known to have different values.

### 2. Dual-source key code lists

The valid key codes are defined in two places with no cross-validation:
- Rust: `SUPPORTED_KEY_CODES` / `key_info()` in `src/browser/keys.rs`
- TypeScript: `keycodes()` in `src/specification/random.ts`

If these drift apart, the fuzzer either generates actions that fail silently at runtime (code in TS but not Rust) or never exercises supported keys (code in Rust but not TS). The runner swallows action errors and retries, so drift goes unnoticed.

**Impact:** Reduced fuzzing coverage or wasted cycles on failing actions, with no user-visible signal.

**Mitigation path:** Add a test that asserts the TypeScript `keycodes()` output matches the Rust `SUPPORTED_KEY_CODES` set, or generate one list from the other at build time.

## Accepted Risks (Not Requiring Action)

- **Empty string sentinel for no-text keys:** Pragmatic, internal-only, consumed in one place with clear conditional checks.
- **`\r` for Enter:** Matches Chromium CDP convention. Already present on develop.
- **No modifier key support:** Known feature gap, not a regression from develop.
- **No `location` field:** Irrelevant for the current key set (no left/right or numpad variants).
- **`u8` key code range:** Sufficient for all standard virtual key codes. Pre-existing type choice.
