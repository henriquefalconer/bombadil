# Security Analysis: feat/keys Branch

## Fundamental Problems and Risky Assumptions

### Problem 1: `code` and `key` CDP fields always hold the same value

**Severity: Medium**

The CDP `DispatchKeyEventParams` has two distinct fields: `code` (the physical key identifier, e.g., `"Digit1"`) and `key` (the logical key value, e.g., `"1"`). The `KeyInfo` struct correctly declares separate `code` and `key` fields, but every entry in `key_info()` sets them to the same string. For the eight currently supported keys (Backspace, Tab, Enter, Escape, four arrows), `code` and `key` happen to be identical per the CDP specification, so this is correct today.

**Why this cannot be disregarded:** The `key_info()` function is the natural place to add new keys, and the pattern of identical `code`/`key` values across all eight entries creates a strong template effect. A future contributor adding a printable character key (e.g., code 49 → `code: "Digit1"`, `key: "1"`) or a modifier key (e.g., code 16 → `code: "ShiftLeft"`, `key: "Shift"`) may follow the existing pattern and set both fields to the same value, producing incorrect key events that pass the existing unit tests (which only check that `code` and `key` are both set to the expected name).

**Consequences:**
- Web applications that read `event.code` for layout-independent shortcuts would receive wrong values (e.g., `"1"` instead of `"Digit1"`).
- Web applications that display `event.key` to users would show physical key identifiers instead of logical values.
- Games, accessibility tools, and internationalized applications that distinguish physical from logical keys would be incorrectly fuzzed.
- The existing unit tests would not catch the error because they verify `code` and `key` independently but do not test that they differ when they should.

---

### Problem 2: Dual-source key code lists with no automated synchronization

**Severity: Medium**

The set of valid key codes exists in two places with no compile-time or test-time cross-validation:
- Rust: `SUPPORTED_KEY_CODES` const and `key_info()` match arms in `src/browser/keys.rs`
- TypeScript: `keycodes()` array in `src/specification/random.ts`

The `SUPPORTED_KEY_CODES` constant and its doc comment reference the TypeScript function, and there is a unit test ensuring `SUPPORTED_KEY_CODES` matches `key_info()`. But there is no test verifying the TypeScript list matches the Rust list.

**Why this cannot be disregarded:** The problem existed with the original two keys on develop but was lower-risk because the list was small and Backspace/Tab were already in the TS list without Rust support (silently producing errors). With eight keys and an established pattern of expansion, drift becomes more likely. Runtime failures during fuzzing are silent — the runner swallows action errors and retries with a different action, so missed coverage goes unnoticed.

**Consequences:**
- If a code is added to `keycodes()` but not to `key_info()`: the fuzzer generates `PressKey` actions that fail at runtime with "unknown key with code." The runner retries, wasting cycles and silently reducing key-press coverage.
- If a code is added to `key_info()` but not to `keycodes()`: the default action set never exercises that key. The key is supported but unreachable from the standard fuzzing configuration.
- Both failure modes are invisible to the user. There is no log message, no metric, and no test that would surface the inconsistency.

---

### Problem 3: Pre-existing mismatch between develop's `keycodes()` and `key_name()` (now fixed)

**Severity: Low (resolved by feat/keys)**

On develop, the TypeScript `keycodes()` function returned `[8, 9, 13, 27]` (Backspace, Tab, Enter, Escape), but the Rust `key_name()` function only recognized codes 13 (Enter) and 27 (Escape). Codes 8 and 9 would produce "unknown key" errors at runtime — exactly the drift described in Problem 2.

**This is resolved by feat/keys**, which adds Rust-side support for all codes in the TypeScript list and extends both lists with arrow keys. However, the resolution was manual — the same drift can recur because the underlying lack of automated cross-validation (Problem 2) is not addressed.

---

### Problem 4: Integration test does not export `clicks` as a baseline action

**Severity: Low**

The `test_key_press_tab_moves_focus` test exports only `tabKey` as its action. The existing test pattern (documented in PATTERNS.md) specifies: "When a test provides a custom spec and needs interaction to keep the runner loop cycling, export `clicks` as the baseline action. Only export a different action set when the test specifically exercises that action type."

The test specifically exercises Tab key behavior, so not exporting `clicks` is intentionally correct — the test needs only Tab presses to verify focus movement. However, by not including a baseline action, the test relies on Tab being the only available action, which means the runner has no fallback if the Tab action fails or if the page state makes Tab a no-op.

**This can be considered acceptable** because the test is specifically designed to verify Tab behavior in isolation, and including `clicks` would introduce non-deterministic clicking that could interfere with the focus-movement property being tested.

---

## Disregarded Items

The following items were evaluated and determined to not require action:

| Item | Reason for Disregard |
|------|---------------------|
| Empty string sentinel for no-text keys (`""`) | Pragmatic pattern, internal-only, consumed in one place with clear conditional checks. `Option<&'static str>` would be more explicit but adds overhead for a private type. |
| `\r` for Enter text value | Matches Chromium CDP convention. Already present on develop. Verified against Puppeteer's `USKeyboardLayout`. |
| No modifier key support (Shift, Ctrl, Alt, Meta) | Feature gap, not a regression. develop had no modifier support either. |
| No `location` field in key dispatch | Irrelevant for the current key set (no left/right modifier variants, no numpad keys). |
| `u8` key code type range (0–255) | Pre-existing type choice on develop. All standard virtual key codes fit in `u8`. |
| `div#result` in test fixture is unused | Harmless placeholder element. Does not affect test behavior. |

---

## Summary of Non-Disregardable Problems

| # | Problem | Severity | Core Risk |
|---|---------|----------|-----------|
| 1 | `code` and `key` fields always identical | Medium | Template effect leads to incorrect key events when map is extended to keys where code ≠ key |
| 2 | Dual-source key code lists with no automated sync | Medium | Silent runtime failures or missed fuzzing coverage from list drift |
