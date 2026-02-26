# Security Analysis: feat/keys Branch

## Fundamental Problems and Risky Assumptions

### Problem 1: `code` and `key` CDP fields always hold the same value

**Severity: Medium**

The CDP `DispatchKeyEventParams` has two distinct fields: `code` (the physical key identifier, e.g., `"Digit1"`) and `key` (the logical key value, e.g., `"1"`). The `KeyInfo` struct correctly declares separate `code` and `key` fields, but every entry in `key_info()` sets them to the same string. For the eight currently supported keys (Backspace, Tab, Enter, Escape, four arrows), `code` and `key` happen to be identical per the CDP specification, so this is correct today.

**Direct result of feat/keys code:** Yes. The `KeyInfo` struct and its eight entries were introduced in this branch. The develop branch used a single `&'static str` return value (inherently no conflation issue since only one string existed per key).

**Why this cannot be disregarded:** The `key_info()` function is the natural extension point for adding new keys, and the pattern of identical `code`/`key` values across all eight entries creates a strong template effect. A future contributor adding a printable character key (e.g., code 65 → `code: "KeyA"`, `key: "a"`) or a modifier key (e.g., code 16 → `code: "ShiftLeft"`, `key: "Shift"`) may follow the existing pattern and set both fields to the same value, producing incorrect key events that pass the existing unit tests (which verify fields independently but don't test that they should differ when they must).

**Connected systems and influences:**
- `actions.rs` reads `info.code` and `info.key` separately and passes them to CDP — the separation infrastructure is correct
- The existing unit tests verify each field's value but don't encode the CDP spec requirement that code ≠ key for certain key categories
- The `SUPPORTED_KEY_CODES` cross-boundary test only validates set membership, not field correctness
- Web applications under test receive these values via `KeyboardEvent.code` and `KeyboardEvent.key` — incorrect values change what the app perceives

**Consequences of deploying:**
- Today: no issue. All 8 named keys correctly have identical code/key values per CDP spec.
- Future extension without awareness: web applications that read `event.code` for layout-independent shortcuts would receive wrong values. Games, accessibility tools, and internationalized UIs that distinguish physical from logical keys would be incorrectly fuzzed. The coverage-guided engine might miss real bugs because incorrect events don't trigger the code paths that the real keyboard layout would.
- The error propagates silently — the app receives syntactically valid but semantically wrong events, doesn't crash, and the fuzzer continues without any signal that something is wrong.

**Existing mitigation:** A detailed comment in `key_info()` warns that `code`/`key` identity is coincidental and must not be copied for other key categories. This reduces the template risk but cannot eliminate it.

---

### Problem 2: Dual-source key code lists with no compile-time synchronization

**Severity: Medium (reduced from High by cross-boundary test)**

The set of valid key codes exists in two places:
- Rust: `SUPPORTED_KEY_CODES` const and `key_info()` match arms in `src/browser/keys.rs`
- TypeScript: `keycodes()` array in `src/specification/random.ts`

**Direct result of feat/keys code:** Partially. The dual-source pattern was pre-existing on develop (TS had `[8, 9, 13, 27]`, Rust had only `[13, 27]`). The branch both worsened it (more codes = more surface for drift) and mitigated it (added cross-boundary test + `SUPPORTED_KEY_CODES` constant with doc comment).

**Why this cannot be disregarded:** Runtime failures during fuzzing are silent. The runner catches action errors and retries with a different action. If a code exists in TypeScript but not Rust, the fuzzer wastes cycles generating actions that always fail. If a code exists in Rust but not TypeScript, a supported key is never exercised. Both failure modes are invisible to the user — no log message, no metric, no test failure during normal development.

**Connected systems and influences:**
- `src/specification/defaults/actions.ts` calls `keycodes().generate()` to produce random key codes for the default action set
- `src/runner.rs` receives `PressKey { code }` actions and calls `BrowserAction::apply()`, which calls `key_info(code)`
- The runner's error handling swallows the "unknown key" error and picks a new action, masking the inconsistency
- The `SUPPORTED_KEY_CODES` ↔ `key_info()` invariant is enforced by a Rust-side unit test (`all_supported_codes_have_key_info`)
- The TS ↔ Rust invariant is now enforced by `keycodes_matches_supported_key_codes` in `random_test.rs`

**Consequences of deploying:**
- Today: both lists contain `[8, 9, 13, 27, 37, 38, 39, 40]`. The cross-boundary test verifies this. No drift.
- Future addition without running full tests: someone adds a code to one side, runs language-specific tests only, and pushes. The cross-boundary test is in `random_test.rs` (a Rust test) — it catches TS-only additions when `cargo test` runs, but someone editing only TypeScript might not run `cargo test`.
- The `SUPPORTED_KEY_CODES` constant could also drift from the `key_info()` match arms (three sources, not two), though the `all_supported_codes_have_key_info` test catches this within Rust.

**Existing mitigation:** Cross-boundary test + doc comment on `SUPPORTED_KEY_CODES` referencing the TypeScript function. Significantly reduces risk compared to develop's manual-sync-only approach.

---

### Problem 3: Pre-existing mismatch between develop's `keycodes()` and `key_name()` (resolved)

**Severity: Low (resolved by feat/keys)**

On develop, the TypeScript `keycodes()` function returned `[8, 9, 13, 27]` but the Rust `key_name()` function only recognized codes 13 and 27. Codes 8 (Backspace) and 9 (Tab) would produce "unknown key with code" errors at runtime — exactly the drift described in Problem 2. This was a live bug on develop.

**Resolution:** feat/keys adds Rust-side support for all codes in the TypeScript list and extends both lists with arrow keys. The cross-boundary test prevents recurrence.

---

### Problem 4: Integration test exports only `tabKey` with no `clicks` baseline

**Severity: Low (intentionally correct)**

The `test_key_press_tab_moves_focus` test exports only `tabKey` as its action set, without the `clicks` baseline that PATTERNS.md recommends for tests needing interaction. This is intentionally correct: the test specifically exercises Tab key behavior in isolation, and including `clicks` would introduce non-deterministic clicking that could interfere with the focus-movement property being tested.

**Considered and disregarded:** The PATTERNS.md rule explicitly allows omitting `clicks` when "the test specifically exercises that action type." Tab focus movement is the action type under test.

---

## Disregarded Items

The following items were evaluated and determined to not require action:

| Item | Reason for Disregard |
|------|---------------------|
| Empty string sentinel for no-text keys (`""`) | Pragmatic pattern, internal-only, consumed in one place with clear conditional checks. `Option<&'static str>` would be more explicit but adds overhead for a private type with a single consumer. The project does not enforce Option for every absence semantic. |
| `\r` for Enter text value | Matches Chromium CDP convention. Already present on develop. Verified against Puppeteer's `USKeyboardLayout`. |
| No modifier key support (Shift, Ctrl, Alt, Meta) | Feature gap, not a regression. Develop had no modifier support either. Current set covers navigation and editing keys relevant to form-heavy fuzzing. |
| No `location` field in key dispatch | Irrelevant for the current key set. No left/right modifier variants, no numpad keys. Would be needed if modifiers are added. |
| `u8` key code type range (0–255) | Pre-existing type choice on develop. All standard virtual key codes fit in `u8`. |
| `div#result` in test fixture is unused | Harmless placeholder element. Does not affect test behavior. Minor fixture hygiene issue only. |
| Cross-boundary test accesses TS `private` field | TypeScript `private` is compile-time only; the `elements` property exists at runtime. If `From<T>` renames the field, the test fails — which is the desired behavior, surfacing the contract change. The coupling is intentional. |
| Unused `from` import in test spec | The test spec imports `from` from `@antithesishq/bombadil` but does not use it. This is a minor code style issue with no runtime impact; the TS transpiler does not enforce unused-import errors. |

---

## Summary of Non-Disregardable Problems

| # | Problem | Severity | Core Risk | Mitigation Status |
|---|---------|----------|-----------|-------------------|
| 1 | `code` and `key` fields always identical | Medium | Template effect leads to incorrect key events when map is extended to keys where code ≠ key | Comment in `key_info()` warns of coincidental identity |
| 2 | Dual-source key code lists | Medium | Silent runtime failures or missed fuzzing coverage from list drift | Cross-boundary test in `random_test.rs` catches drift at test time |
