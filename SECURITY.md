# Security: feat/keys Key Dispatch Changes

## Risk Summary

The feat/keys branch extends key press support from 2 keys (Enter, Escape) to 8 keys (adding Backspace, Tab, and four arrow keys). The core security-relevant change is making the CDP key dispatch conditional: keys that do not produce character input no longer send `text`/`unmodified_text` fields or the `Char` event. This is a correctness fix — the previous code sent `"\r"` as the text for every key, causing non-text keys to behave as if they typed a carriage return.

## Identified Risks

### 1. CDP field conflation (`code` vs `key`)

The `KeyInfo` struct uses a single `name` field for both the CDP `code` and `key` parameters. For the eight currently supported keys, these values are identical. For future keys (printable characters, modifiers, numpad), they diverge. A future contributor extending the map may follow the single-name pattern and produce incorrect key events that pass unit tests but cause wrong behavior in target applications.

**Impact:** Incorrect `event.code` or `event.key` values in the target application's keyboard handlers. Applications that use `event.code` for layout-independent shortcuts or `event.key` for display would receive wrong values.

**Mitigation path:** Split `name` into `code` and `key` fields in `KeyInfo`, or add a comment explicitly noting that the fields are identical for the current set and must be differentiated when adding keys where they diverge.

### 2. Dual-source key code lists

The valid key codes are defined in two places with no cross-validation:
- Rust: `key_info()` match arms in `src/browser/keys.rs`
- TypeScript: `keycodes()` array in `src/specification/random.ts`

If these drift apart, the fuzzer either generates actions that fail at runtime (code in TS but not Rust) or never exercises supported keys (code in Rust but not TS). Both failures are silent — the runner swallows action errors and retries.

**Impact:** Reduced fuzzing coverage or wasted cycles on failing actions, with no user-visible signal.

**Mitigation path:** Add a test that asserts the TS `keycodes()` output matches the set of codes accepted by `key_info()`, or generate one list from the other.

## Accepted Risks (Not Requiring Action)

- **Empty string sentinel for no-text keys:** Pragmatic, internal-only, consumed in one place with clear conditional checks.
- **`\r` for Enter:** Matches Chromium CDP convention. Already present on develop.
- **No modifier key support:** Known feature gap, not a regression from develop.
- **No `location` field:** Irrelevant for the current key set (no left/right or numpad variants).
- **`u8` key code range:** Sufficient for all standard virtual key codes. Pre-existing type choice.
