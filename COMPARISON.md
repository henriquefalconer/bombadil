# Comparison: feat/keys vs develop

This document describes only the code that was altered between the `develop` and `feat/keys` branches. No pre-existing code is described unless it is directly adjacent to a change and required for context.

---

## Changed Files

Four source files were modified, one new test file was added, and one new test fixture directory was created.

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

No struct. No constants. No unit tests. The entire file was 7 lines.

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

A `SUPPORTED_KEY_CODES` constant lists all eight codes with a doc comment referencing the TypeScript `keycodes()` function as the other side of the cross-boundary contract:

```rust
/// All key codes supported by Bombadil. Must match `keycodes()` in
/// `src/specification/random.ts` — that list is the TypeScript side of this
/// cross-boundary contract.
pub const SUPPORTED_KEY_CODES: &[u8] = &[8, 9, 13, 27, 37, 38, 39, 40];
```

A comment inside `key_info()` warns that `code` and `key` are coincidentally identical for the current set of named keys and must diverge for other key categories:

```rust
// NOTE: For this set of special keys `code` and `key` happen to be
// identical strings. This is correct per CDP spec for named keys
// (Backspace, Tab, Enter, Escape, Arrow*). For other key categories
// they must diverge — do NOT copy this pattern blindly:
//   • Printable chars: code=49 → code:"Digit1", key:"1"
//   • Modifiers:       code=16 → code:"ShiftLeft", key:"Shift"
//   • Numpad:          code=96 → code:"Numpad0", key:"0"
```

A `#[cfg(test)] mod tests` block was added with seven unit tests:
- `backspace_has_no_text`: verifies code, key, and empty text for code 8
- `tab_has_no_text`: verifies code, key, and empty text for code 9
- `enter_has_text`: verifies code, key, and `"\r"` text for code 13
- `escape_has_no_text`: verifies code, key, and empty text for code 27
- `arrow_keys_have_no_text`: loop over codes 37–40 verifying code, key, and empty text
- `unknown_codes_return_none`: verifies codes 0 and 255 return `None`
- `all_supported_codes_have_key_info`: iterates `SUPPORTED_KEY_CODES` and asserts each returns `Some` from `key_info()`

#### What Changed in Substance

- Return type changed from `Option<&'static str>` to `Option<KeyInfo>`, introducing a struct that bundles separate CDP `code` and `key` fields with the text payload for dispatch.
- The text field distinguishes keys that produce character input (Enter → `"\r"`) from keys that only produce key events (all others → `""`).
- Six new key codes were added (8, 9, 37, 38, 39, 40).
- `SUPPORTED_KEY_CODES` constant added as the Rust side of the cross-boundary contract.
- Unit tests added covering all keys and the `SUPPORTED_KEY_CODES` ↔ `key_info` invariant.

---

### 2. `src/browser/actions.rs`

#### Before (develop)

The `PressKey` arm called `key_name(*code)` inside a closure, and unconditionally set `.text("\r")` and `.unmodified_text("\r")` on every key dispatch. It always sent three CDP events: `RawKeyDown`, `Char`, `KeyUp`. The error for unknown key codes was raised inside the closure via `bail!`.

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

The import changed from `key_name` to `key_info`. The error check was moved before the closure using `.ok_or_else()`. The closure now sets `.code(info.code)` and `.key(info.key)` as separate struct field accesses. Text and the `Char` event are conditionally included only when `info.text` is non-empty.

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

- Import changed from `key_name` to `key_info`.
- Error for unknown key codes raised earlier (before the closure) rather than inside the closure on every invocation.
- CDP `code` and `key` fields set from separate `info.code` and `info.key` values (previously both from a single `name` string).
- Text payload conditionally attached: keys with empty text no longer send `text` or `unmodified_text` fields.
- `Char` event conditionally skipped for non-text keys, so the browser receives only `RawKeyDown` + `KeyUp` for these keys, allowing native behavior (Tab moves focus, Backspace deletes, arrows navigate).

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

Note: on develop, `keycodes()` already included codes 8 (Backspace) and 9 (Tab), but the Rust `key_name()` function did not recognize them — they would produce "unknown key" errors at runtime. The feat/keys branch adds Rust-side support for these codes, fixing the pre-existing mismatch.

---

### 4. `src/specification/random_test.rs`

A new test function `keycodes_matches_supported_key_codes` was added. No existing code was modified.

```rust
#[test]
fn keycodes_matches_supported_key_codes() {
    use crate::browser::keys::SUPPORTED_KEY_CODES;

    let (mut context, module) = load_random_module(vec![]);

    let keycodes_fn = js::module_exports(&module, &mut context)
        .unwrap()
        .get(&PropertyKey::String(js_string!("keycodes")))
        .unwrap()
        .clone();
    let generator = keycodes_fn
        .as_callable()
        .unwrap()
        .call(&JsValue::undefined(), &[], &mut context)
        .unwrap();

    let elements_val = generator
        .as_object()
        .unwrap()
        .get(js_string!("elements"), &mut context)
        .unwrap();
    let elements_obj = elements_val.as_object().unwrap();
    let length = elements_obj
        .get(js_string!("length"), &mut context)
        .unwrap()
        .to_u32(&mut context)
        .unwrap() as usize;

    let mut ts_codes: Vec<u8> = (0..length as u32)
        .map(|i| {
            elements_obj
                .get(i, &mut context)
                .unwrap()
                .to_u32(&mut context)
                .unwrap() as u8
        })
        .collect();
    ts_codes.sort_unstable();

    let mut rust_codes: Vec<u8> = SUPPORTED_KEY_CODES.to_vec();
    rust_codes.sort_unstable();

    assert_eq!(
        ts_codes, rust_codes,
        "TypeScript keycodes() elements must match Rust SUPPORTED_KEY_CODES"
    );
}
```

The test loads the `random.js` module via the Boa JS engine, calls `keycodes()`, introspects the resulting `From<number>` generator's `elements` array at runtime (TypeScript `private` is compile-time only), collects and sorts the values, then asserts sorted equality with the Rust `SUPPORTED_KEY_CODES` constant.

---

### 5. `tests/integration_tests.rs`

A new test function `test_key_press_tab_moves_focus` was added. No existing code was modified.

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

The test exports only a `tabKey` action (no `clicks`). It verifies that pressing Tab (code 9) moves focus from the first input to the second by checking `activeElement.id`. Uses `TEST_TIMEOUT_SECONDS` (120s) and an LTL `eventually(...).within(10, "seconds")` bound.

---

### 6. `tests/key-press/index.html` (new file)

A new test fixture directory with a minimal HTML page:

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

The first input has `autofocus` so focus starts there. The fixture follows the minimal HTML style (no `<!DOCTYPE html>`, no `<meta>`, no `lang` attribute) consistent with other fixtures in the `tests/` directory.

---

## Files NOT Changed

The following files are relevant to the key press feature but were not modified:

- `src/browser.rs` — module declarations unchanged; `keys` was already declared as `pub mod keys;`
- `src/specification/js.rs` — `JsAction` enum and conversion logic unchanged; `PressKey` variant already existed
- `src/specification/actions.ts` — `Action` type unchanged; `PressKey` variant already existed
- `src/specification/defaults/actions.ts` — default actions unchanged; already used `keycodes().generate()`
- `src/runner.rs` — action timeout for `PressKey` already set at 50ms
- `Cargo.toml`, `package.json`, build configuration — no dependency or build changes
