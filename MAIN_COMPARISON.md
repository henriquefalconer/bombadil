# Code Comparison: `main` vs `antithesishq/main`

`main` is 7 commits ahead of `antithesishq/main`. `antithesishq/main` is the exact fork point (no divergent commits). The net diff touches 4 files: 1 modified, 2 added, 1 modified.

---

## Modified: `src/browser/instrumentation.rs`

Only the `FulfillRequestParams` builder chain changed. Everything else in this file is identical between the two branches.

### antithesishq/main (lines 158–176)

```rust
page.execute(
    fetch::FulfillRequestParams::builder()
        .request_id(event.request_id.clone())
        .body(BASE64_STANDARD.encode(body_instrumented))
        .response_code(200)
        .response_header(fetch::HeaderEntry {
            name: "etag".to_string(),
            value: format!("{}", source_id.0),
        })
        // TODO: forward headers
        .build()
```

Sends a single `etag` header. All original response headers (including `Content-Type`) are dropped.

### main (lines 158–176)

```rust
page.execute(
    fetch::FulfillRequestParams::builder()
        .request_id(event.request_id.clone())
        .body(BASE64_STANDARD.encode(body_instrumented))
        .response_code(200)
        .response_headers(
            event
                .response_headers
                .iter()
                .flatten()
                .filter(|h| {
                    !h.name.eq_ignore_ascii_case("etag")
                })
                .cloned()
                .chain(std::iter::once(fetch::HeaderEntry {
                    name: "etag".to_string(),
                    value: format!("{}", source_id.0),
                })),
        )
        .build()
```

Takes all original response headers from `event.response_headers` (`Option<Vec<HeaderEntry>>`), filters out the original `etag` (case-insensitive), clones the remaining headers, and appends a synthetic `etag` with the source ID. Uses `.response_headers()` (plural, takes `IntoIterator`) instead of `.response_header()` (singular, takes one entry).

When `event.response_headers` is `None`, `.iter().flatten()` yields nothing, so only the synthetic `etag` is sent — same behavior as antithesishq/main.

---

## Added: `tests/external-module-script/index.html`

Entirely new file. Does not exist in antithesishq/main.

```html
<!DOCTYPE html>
<html>
<body>
  <h1 id="result">WAITING</h1>
  <script type="module" src="/external-module-script/module.js"></script>
</body>
</html>
```

A minimal page that loads an ES module script via `<script type="module" src="...">`. The `#result` element starts as `"WAITING"` and should change to `"LOADED"` when the module executes. This is the reproduction case for the MIME type bug — without `Content-Type` forwarding, Chrome rejects the module script with an empty MIME type error.

---

## Added: `tests/external-module-script/module.js`

Entirely new file. Does not exist in antithesishq/main.

```js
document.getElementById("result").textContent = "LOADED";
```

The external module loaded by `index.html`. Sets `#result` to `"LOADED"` on execution.

---

## Modified: `tests/integration_tests.rs`

Only an appended test function at the end of the file. All existing tests are identical between the two branches.

### antithesishq/main

File ends at line 459 (after `test_counter_state_machine`).

### main (lines 461–486, appended)

```rust
/// Verifies that `<script type="module" src="...">` loads correctly.
///
/// When Bombadil intercepts a response and calls `Fetch.fulfillRequest`, it must
/// forward the original `Content-Type` header. Without it, Chrome rejects ES module
/// scripts with a MIME type error, silently preventing the module from running.
#[tokio::test]
async fn test_external_module_script() {
    run_browser_test(
        "external-module-script",
        Expect::Success,
        Duration::from_secs(10),
        Some(
            r#"
import { extract, eventually } from "@antithesishq/bombadil";
export { scroll } from "@antithesishq/bombadil/defaults";

const resultText = extract((state) => state.document.body?.querySelector("\#result")?.textContent ?? null);

export const module_script_loads = eventually(
  () => resultText.current === "LOADED"
).within(10, "seconds");
"#,
        ),
    )
    .await;
}
```

Uses `run_browser_test` (the existing test harness, unchanged) with:
- Fixture: `external-module-script/` (the two new files above)
- Expectation: `Expect::Success`
- Timeout: 10 seconds (shorter than the standard `TEST_TIMEOUT_SECONDS` of 120)
- Custom spec: extracts `#result` text content, asserts it `eventually` equals `"LOADED"` within 10 seconds
- Action: exports only `scroll` (no clicks needed — the module should load automatically)
