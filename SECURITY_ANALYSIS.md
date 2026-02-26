# Security Analysis: feat/keys Branch

## Fundamental Problems and Risky Assumptions

### Problem 1: `code` and `key` fields use the same value for all keys

**Severity: Medium**

The CDP `DispatchKeyEventParams` has two distinct fields: `code` (the physical key identifier, e.g., `"Tab"`) and `key` (the logical key value, e.g., `"Tab"`). In the current implementation, both are set to `info.name`. For the eight keys currently supported, `code` and `key` happen to be identical, but this is not universally true (e.g., `code: "Digit1"` vs `key: "1"`, or `code: "ShiftLeft"` vs `key: "Shift"`). If the key map is extended to include printable characters, modifier keys, or numpad keys, using the same value for both fields will produce incorrect key events.

**Considered for disregard:** This is a direct result of the feat/keys code. It cannot be disregarded because the `key_info` function is the natural place to add new keys, and whoever adds them may follow the existing pattern of a single `name` field without realizing it conflates two distinct CDP concepts.

**Consequences:** Web applications that read `event.code` vs `event.key` in their keyboard handlers would receive incorrect values. This is particularly relevant for: (a) games and accessibility tools that distinguish physical key position from logical value, (b) keyboard shortcut detection that checks `event.code` for layout-independent bindings, (c) any extension of the key map to printable characters or modifier keys, where `code` and `key` diverge.

---

### Problem 2: `keycodes()` and `key_info()` can become inconsistent

**Severity: Medium**

The set of valid key codes exists in two places: the Rust `key_info()` match arms and the TypeScript `keycodes()` array. There is no compile-time or test-time check that these two lists are synchronized. If a code is added to `keycodes()` but not to `key_info()`, the fuzzer will generate `PressKey` actions that fail at runtime with "unknown key with code." If a code is added to `key_info()` but not to `keycodes()`, the fuzzer will never exercise that key.

**Considered for disregard:** This is a direct result of the feat/keys code — the problem existed with the original two keys but was less consequential because the list was small and obvious. With eight keys and an expanding pattern, the risk of drift increases. This is not disregardable because runtime failures during fuzzing are silent from the user's perspective — the runner retries a different action and the missed coverage goes unnoticed.

**Consequences:** (a) Runtime errors in the runner loop when an unsupported code reaches `key_info()` — the action fails and the runner picks another, silently reducing key-press coverage. (b) Asymmetric key coverage — some keys never get tested despite being supported. (c) Future contributors adding keys in one place but not the other, since there is no shared source of truth or cross-language validation.

---

### Problem 3: Empty string as sentinel for "no text"

**Severity: Low**

The `KeyInfo.text` field uses `""` (empty string) to mean "this key does not produce character input." The dispatch logic in `actions.rs` checks `!info.text.is_empty()` to decide whether to include text fields and whether to send the `Char` event. This works correctly for all currently supported keys, but empty string as a sentinel is fragile: it conflates "no text" with "text that happens to be empty" and relies on every future contributor understanding this convention.

**Considered for disregard:** This follows a pragmatic pattern that is common in Rust (using `""` or empty vec as "none" for non-optional data). The struct is small, internal, and only consumed in one place. The alternative (using `Option<&'static str>`) would be more explicit but adds syntactic overhead for a purely internal type. **This can be disregarded** — the current approach is acceptable for the scope and visibility of the type, and the conditional checks in `actions.rs` make the intent clear.

---

### Problem 4: Hardcoded `\r` for Enter key text value

**Severity: Low**

Enter's text is hardcoded as `"\r"` (carriage return). In the previous code on develop, `"\r"` was unconditionally set for all keys — a clear bug that the feat/keys branch fixes. However, the choice of `"\r"` specifically (vs `"\n"` or `"\r\n"`) is a CDP convention detail. Chromium's own key event dispatch uses `"\r"` for Enter, so this is correct, but it is an undocumented assumption.

**Considered for disregard:** This matches Chromium's actual behavior (verified by the CDP protocol documentation and Puppeteer's source). The value was already present on develop and is not new to feat/keys. **This can be disregarded** — it is correct and matches the upstream protocol.

---

### Problem 5: No modifier key support (Shift, Ctrl, Alt, Meta)

**Severity: Low**

The `DispatchKeyEventParams` builder supports `modifiers` (a bitmask for Shift, Ctrl, Alt, Meta) but the `PressKey` action and `KeyInfo` struct have no mechanism to express or send modifier state. All key presses are dispatched as unmodified. This means: (a) Shift+Tab (reverse tab navigation) cannot be tested, (b) Ctrl+A, Ctrl+C, and other keyboard shortcuts cannot be fuzzed, (c) Alt+arrow and other accessibility shortcuts are unreachable.

**Considered for disregard:** Modifier support was not present on develop either, and feat/keys does not claim to add it. This is a feature gap, not a regression. **This can be disregarded** as a known limitation rather than a problem introduced by the branch.

---

### Problem 6: No `location` field in key dispatch

**Severity: Low**

The CDP `DispatchKeyEventParams` supports a `location` field that distinguishes between left and right variants of modifier keys (Left Shift vs Right Shift) and numpad keys vs main keys. The current implementation does not set this field. For the eight keys currently supported, this is harmless — none have left/right variants and none are numpad duplicates.

**Considered for disregard:** None of the currently supported keys are affected. This only becomes relevant if the key map expands to include modifier keys or numpad keys. **This can be disregarded** for the current scope.

---

### Problem 7: `u8` key code type limits the addressable key space

**Severity: Low**

The `PressKey` variant uses `code: u8`, limiting key codes to 0–255. Standard virtual key codes (Windows VK_ codes, which the CDP `windowsVirtualKeyCode` field references) fit within 0–254 for all common keys, so this is sufficient for the current and foreseeable key set. However, some extended keys and OEM keys can theoretically exceed 255.

**Considered for disregard:** This is not introduced by feat/keys — the `u8` type was already the type on develop. All commonly used virtual key codes fit in a `u8`. **This can be disregarded**.

---

## Summary of Non-Disregardable Problems

| # | Problem | Severity | Risk |
|---|---------|----------|------|
| 1 | `code` and `key` fields conflated into single `name` | Medium | Incorrect key events when map is extended to keys where code != key |
| 2 | Dual-source key code lists with no synchronization | Medium | Silent runtime failures or missed coverage during fuzzing |
