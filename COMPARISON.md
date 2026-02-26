# Comparison: feat/keys vs develop

This document describes only the code that was altered between the `develop` and `feat/keys` branches. No pre-existing code is described unless it is directly adjacent to a change and required for context.

---

## Changed Files

Four source files were modified and one new test fixture directory was added.

### 1. `src/browser/keys.rs`

#### Before (develop)

A single function `key_name(code: u8) -> Option<&'static str>` that mapped two key codes to their names:

```rust
pub fn key_name(code: u8) -> Option<&'static str> {
    match code {
        13 => Some("Enter"),
        27 => Some("Escape"),
        _ => None,
    }
}
```

No struct. No constants. No unit tests.

#### After (feat/keys)

The function was replaced with a `KeyInfo` struct and a `key_info(code: u8) -> Option<KeyInfo>` function. A `SUPPORTED_KEY_CODES` constant was added. The struct carries three fields:

```rust
pub struct KeyInfo {
    pub code: &'static str,
    pub key: &'static str,
    pub text: &'static str,
}
```

The `key_info` function maps eight key codes (up from two):

| Code | `code` field | `key` field | `text` field |
|------|-------------|-------------|-------------|
| 8    | Backspace   | Backspace   | `""`        |
| 9    | Tab         | Tab         | `""`        |
| 13   | Enter       | Enter       | `"\r"`      |
| 27   | Escape      | Escape      | `""`        |
| 37   | ArrowLeft   | ArrowLeft   | `""`        |
| 38   | ArrowUp     | ArrowUp     | `""`        |
| 39   | ArrowRight  | ArrowRight  | `""`        |
| 40   | ArrowDown   | ArrowDown   | `""`        |

A `SUPPORTED_KEY_CODES` constant lists all eight codes with a doc comment referencing the TypeScript `keycodes()` function as the other side of the cross-boundary contract.

A `#[cfg(test)] mod tests` block was added with seven unit tests:
- One per named key verifying `code`, `key`, and `text` fields
- Arrow keys tested as a group via loop
- Unknown codes returning `None`
- All `SUPPORTED_KEY_CODES` entries having corresponding `key_info` results

#### What Changed in Substance

- The return type changed from `Option<&'static str>` to `Option<KeyInfo>`, introducing a struct that bundles separate CDP `code` and `key` fields with the text payload for dispatch.
- The text field distinguishes keys that produce character input (Enter → `"\r"`) from keys that only produce key events (all others → `""`).
- Six new key codes were added (8, 9, 37, 38, 39, 40).
- A `SUPPORTED_KEY_CODES` constant was added as the Rust side of the cross-boundary contract.
- Unit tests were added covering all keys and the `SUPPORTED_KEY_CODES` ↔ `key_info` invariant.

---

### 2. `src/browser/actions.rs`

#### Before (develop)

The `PressKey` arm called `key_name(*code)` inside a closure, and unconditionally set `.text("\r")` and `.unmodified_text("\r")` on every key dispatch. It always sent three CDP events: `RawKeyDown`, `Char`, `KeyUp`. The error for unknown key codes was raised inside the closure.

```rust
BrowserAction::PressKey { code } => {
    let build_params = |event_type| {
        if let Some(name) = key_name(*code) {
            input::DispatchKeyEventParams::builder()
                .r#type(event_type)
                .native_virtual_key_code(*code as i64)
                .windows_virtual_key_code(*code as i64)
                .code(name)
                .key(name)
                .unmodified_text("\r")
                .text("\r")
                .build()
                .map_err(|err| anyhow!(err))
        } else {
            bail!("unknown key with code: {:?}", code)
        }
    };
    page.execute(build_params(input::DispatchKeyEventType::RawKeyDown)?).await?;
    page.execute(build_params(input::DispatchKeyEventType::Char)?).await?;
    page.execute(build_params(input::DispatchKeyEventType::KeyUp)?).await?;
}
```

#### After (feat/keys)

The error check was moved before the closure using `.ok_or_else()`. The closure now sets `.code(info.code)` and `.key(info.key)` as separate fields. Text and `Char` event are conditionally included only when `info.text` is non-empty.

```rust
BrowserAction::PressKey { code } => {
    let info = key_info(*code).ok_or_else(|| {
        anyhow!("unknown key with code: {:?}", code)
    })?;
    let build_params = |event_type| {
        let mut builder = input::DispatchKeyEventParams::builder()
            .r#type(event_type)
            .native_virtual_key_code(*code as i64)
            .windows_virtual_key_code(*code as i64)
            .code(info.code)
            .key(info.key);
        if !info.text.is_empty() {
            builder = builder.unmodified_text(info.text).text(info.text);
        }
        builder.build().map_err(|err| anyhow!(err))
    };
    page.execute(build_params(input::DispatchKeyEventType::RawKeyDown)?).await?;
    if !info.text.is_empty() {
        page.execute(build_params(input::DispatchKeyEventType::Char)?).await?;
    }
    page.execute(build_params(input::DispatchKeyEventType::KeyUp)?).await?;
}
```

#### What Changed in Substance

- The import changed from `key_name` to `key_info`.
- The error for unknown key codes is raised earlier (before the closure) rather than inside the closure on every invocation.
- CDP `code` and `key` fields are set from separate `info.code` and `info.key` values (previously both came from a single `name` string).
- Text payload is conditionally attached: keys with empty text no longer send `text` or `unmodified_text` fields.
- The `Char` event is conditionally skipped for non-text keys, so the browser receives only `RawKeyDown` + `KeyUp` for these keys, allowing native behavior (Tab moves focus, Backspace deletes, arrows navigate).

---

### 3. `src/specification/random.ts`

#### Before (develop)

```typescript
export function keycodes(): Generator<number> {
  return from([8, 9, 13, 27]);
}
```

#### After (feat/keys)

```typescript
export function keycodes(): Generator<number> {
  return from([8, 9, 13, 27, 37, 38, 39, 40]);
}
```

#### What Changed in Substance

Four arrow key codes (37–40) were added to the random key code generator. The default action set can now randomly produce arrow key presses during fuzzing.

Note: on develop, `keycodes()` already included codes 8 (Backspace) and 9 (Tab), but the Rust `key_name()` function did not recognize them — they would produce "unknown key" errors at runtime. The feat/keys branch adds Rust-side support for these codes.

---

### 4. `tests/integration_tests.rs`

A new test function `test_key_press_tab_moves_focus` was added:

```rust
#[tokio::test]
async fn test_key_press_tab_moves_focus() {
    run_browser_test(
        "key-press",
        Expect::Success,
        Duration::from_secs(TEST_TIMEOUT_SECONDS),
        Some(
            r#"
import { actions, extract, eventually, from } from "@antithesishq/bombadil";

export const tabKey = actions(() => [{ PressKey: { code: 9 } }]);

const focusedId = extract((state) => state.document.activeElement?.id ?? null);

export const tab_moves_focus_to_second_input = eventually(
  () => focusedId.current === "second"
).within(10, "seconds");
"#,
        ),
    )
    .await;
}
```

The test exports only a `tabKey` action (no `clicks`). It verifies that pressing Tab (code 9) moves focus from the first input to the second by checking `activeElement.id`.

---

### 5. `tests/key-press/index.html` (new file)

A new test fixture with two text inputs and a result div:

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

The first input has `autofocus`. The fixture follows the minimal HTML style (no `<!DOCTYPE html>`, no `<meta>`, no `lang` attribute) consistent with the documented fixture pattern.

---

## Files NOT Changed

- `src/browser.rs` (module declarations unchanged — `keys` was already declared as `pub mod keys;`)
- `src/specification/js.rs` (JsAction enum and conversion logic unchanged — PressKey variant already existed)
- `src/specification/actions.ts` (Action type unchanged — PressKey variant already existed)
- `src/specification/defaults/actions.ts` (default actions unchanged — already used `keycodes().generate()`)
- `src/runner.rs` (action timeout for PressKey already set at 50ms)
- `Cargo.toml`, `package.json`, build configuration
