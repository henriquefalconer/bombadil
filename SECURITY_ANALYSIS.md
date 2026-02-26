# Security Analysis: Fundamental Problems and Risky Assumptions

This document evaluates every identified problem and assumption in the code added on `feat/keys` relative to `develop`. For each, it determines: whether the problem is a direct result of the new code, whether it can be disregarded based on existing project patterns and the tool's purpose, and what consequences remain for problems that cannot be disregarded.

---

## Problem 1: Backspace text value may not match real browser behavior

**Description**: `KeyInfo` for Backspace (code 8) sets `text: "\u{0008}"`. This causes a `Char` event to be dispatched with the BS control character. Reference implementations (Puppeteer's `USKeyboardLayout`) dispatch Backspace with no text and no `Char` event — Backspace is a control key that triggers delete-backward, not a text-producing key.

**Origin**: Direct result of the new code. The develop branch did not handle Backspace at all (it returned `None` from `key_name`).

**Disregardable?** No. This is a correctness issue affecting testing fidelity.

**Full consequence analysis**:

- **Failure mode**: When Backspace is pressed in a text input, the browser receives both a `RawKeyDown` event (which triggers the native delete-backward behavior) and a `Char` event with `"\u{0008}"`. The `RawKeyDown` correctly deletes the character before the cursor. The subsequent `Char` event with the BS control character is ambiguous — in most modern browsers and input implementations, it will be ignored or treated as a no-op, but some frameworks or custom input handlers may interpret it as a second delete or display it as a replacement character.
- **Likelihood of impact**: Low for standard HTML inputs. Higher for applications using `contenteditable`, custom text editors (CodeMirror, Monaco, ProseMirror), or frameworks that intercept `keypress`/`input` events at a low level. These commonly check the `data` property of `InputEvent`, and a control character may trigger unexpected code paths.
- **Scope of effect**: Only affects text inputs and textareas when the fuzzer randomly selects Backspace (1 in 8 chance from `keycodes()`, weighted further by the 1:3 PressKey-to-TypeText ratio in `inputs`, so roughly 1 in 32 input interactions).
- **Comparison to develop**: On develop, Backspace triggered a `bail!` error 100% of the time. The new behavior is strictly better — it produces a largely correct key event instead of a guaranteed failure. The text value issue is a fidelity refinement, not a regression.
- **Mitigation**: Change `text` to `""` for Backspace, matching Puppeteer's behavior and the pattern already used for Escape and arrow keys.

---

## Problem 2: Tab text value may not match real browser behavior

**Description**: `KeyInfo` for Tab (code 9) sets `text: "\t"`. This causes a `Char` event to be dispatched with a tab character. Reference implementations (Puppeteer's `USKeyboardLayout`) dispatch Tab with no text and no `Char` event. A real Tab keypress triggers focus navigation, not text insertion.

**Origin**: Direct result of the new code. The develop branch did not handle Tab at all.

**Disregardable?** No. This is a correctness issue affecting testing fidelity.

**Full consequence analysis**:

- **Failure mode**: When Tab is pressed, the browser receives a `RawKeyDown` event (which normally triggers focus navigation) followed by a `Char` event with `"\t"`. The `RawKeyDown` may move focus to the next element. The `Char` event with `"\t"` may then insert a literal tab character into whatever element now has focus, or it may be ignored if focus moved to a non-editable element. The combined effect is unpredictable.
- **Specific scenario**: If Tab is pressed in a `<textarea>`, the `RawKeyDown` may be intercepted by the browser to move focus, but the `Char` event with `"\t"` might insert a tab into the textarea before focus leaves (event ordering is implementation-dependent). Or, in applications that trap Tab to insert indentation (code editors), the Char event may cause a double-indent.
- **Scope of effect**: Same frequency as Backspace (roughly 1 in 32 input interactions). However, Tab's focus-navigation side effect makes the behavior more visible — it can change which element is active, affecting subsequent actions in the fuzzing run.
- **Comparison to develop**: On develop, Tab triggered a `bail!` error. The new behavior is strictly better.
- **Mitigation**: Change `text` to `""` for Tab, matching Puppeteer's behavior and the pattern already used for Escape and arrow keys.

---

## Problem 3: `KeyInfo.name` is used for both CDP `code` and `key` fields

**Description**: In `actions.rs`, both `.code(info.name)` and `.key(info.name)` are set to the same value. In the CDP protocol, `code` represents the physical key location (e.g., `"NumpadEnter"`) while `key` represents the logical key value (e.g., `"Enter"`). For the eight currently supported keys, these values happen to be identical, but the conflation creates a latent trap for future key additions.

**Origin**: Pre-existing pattern (develop already set both `.code(name)` and `.key(name)` to the same value). The new code extends this pattern to more keys but does not introduce it.

**Disregardable?** Yes. For all currently supported keys (Enter, Escape, Backspace, Tab, ArrowLeft, ArrowUp, ArrowRight, ArrowDown), the CDP `code` and `key` values are identical strings. The conflation only becomes incorrect for keys where they differ (e.g., numpad keys, Shift+letter combinations). None of these are in scope. The `KeyInfo` struct could be extended with separate `code` and `key` fields if such keys are added in the future.

---

## Problem 4: No integration tests for the new keys

**Description**: There are no integration tests specifically verifying that arrow keys, Backspace, or Tab produce correct browser behavior when dispatched via CDP. The existing `test_random_text_input` test exercises `PressKey` indirectly (via the `inputs` action generator) but only asserts that input text eventually appears — it does not validate individual key behavior.

**Origin**: Partially pre-existing (Enter and Escape also had no dedicated tests), partially new (six new keys added without tests).

**Disregardable?** Yes, conditionally. The existing pattern in the project is that individual key behaviors are not integration-tested — the fuzzer's correctness is validated through higher-level property tests. However, given Problems 1 and 2 (incorrect text values for Backspace and Tab), the absence of tests means these issues could persist undetected.

---

## Problem 5: Manual synchronization between `keycodes()` (TypeScript) and `key_info()` (Rust)

**Description**: The set of key codes in `random.ts`'s `keycodes()` function must exactly match the set handled by `key_info()` in `keys.rs`. There is no shared source of truth, compile-time check, or runtime validation that the two lists are identical. If they drift, the TypeScript side may generate codes that the Rust side rejects (causing action errors), or the Rust side may support codes that the TypeScript side never generates (dead code).

**Origin**: Pre-existing. The develop branch already had this same manual synchronization requirement between `keycodes()` (`[8, 9, 13, 27]`) and `key_name()` (only handled `13, 27`). In fact, develop had an active desynchronization — `keycodes()` included 8 and 9 but `key_name()` did not, causing 50% of PressKey actions to fail. The new code fixes this desynchronization and extends both lists consistently.

**Disregardable?** Yes. The synchronization requirement is inherent to the TS/Rust boundary design and follows the same pattern used for other cross-boundary contracts (e.g., `Action` type in TypeScript vs `JsAction` enum in Rust). The risk is mitigated by the fact that desynchronization produces immediate, visible errors at runtime.

---

## Problem 6: Empty string sentinel instead of `Option<&str>` for text presence

**Description**: `KeyInfo.text` uses an empty string (`""`) to indicate "no text payload" rather than `Option<&'static str>`. The calling code checks `!info.text.is_empty()` to determine whether to set text fields and send a `Char` event. This sentinel-value pattern is less type-safe than `Option` — it would be possible (though unlikely) for a future contributor to add a key with `text: ""` intending it to produce an empty-string text event, which the code would interpret as "no text."

**Origin**: Direct result of the new code. The develop branch used `Option<&str>` implicitly (the function returned `Option<&str>` and used `if let Some(name)` to determine text handling, though it then hardcoded `"\r"` rather than using the matched value).

**Disregardable?** Yes. Empty strings as "absent" sentinels are idiomatic in Rust for `&str` fields in simple data structs, especially when the alternative (`Option<&str>`) adds nesting at every use site. The check `!info.text.is_empty()` is clear and the semantic of "empty text means no text" is natural. The risk of misuse is theoretical.

---

## Summary

| # | Problem | Origin | Disregardable? | Risk |
|---|---------|--------|----------------|------|
| 1 | Backspace text value | New code | **No** | Medium |
| 2 | Tab text value | New code | **No** | Medium |
| 3 | code/key CDP field conflation | Pre-existing | Yes | Low |
| 4 | No integration tests for new keys | Partially pre-existing | Yes | Low |
| 5 | Manual TS/Rust key code sync | Pre-existing | Yes | Low |
| 6 | Empty string sentinel for text | New code | Yes | None |

**Two non-disregardable problems identified (Problems 1 and 2).** Both are correctness issues, not security vulnerabilities. Both have the same fix: change the `text` field to `""` for Backspace and Tab, matching Puppeteer's behavior and the pattern already established for Escape and arrow keys. This would make all non-text-producing keys consistent: only Enter produces text (`"\r"`), and only Enter dispatches a `Char` event.
