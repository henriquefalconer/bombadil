# Implementation Plan: Fix External Module Script MIME Type Issue

## Problem

`<script type="module" src="...">` tags fail to load under Bombadil because the instrumentation intercept (`src/browser/instrumentation.rs`) drops all original response headers when calling `Fetch.fulfillRequest`. Chrome enforces strict MIME type checking for ES module scripts, and the missing `Content-Type` header causes a fatal error.

## Root Cause

In `src/browser/instrumentation.rs` lines 158-174, `FulfillRequestParams` is built with only a single `etag` header. The original response headers (available via `event.response_headers: Option<Vec<fetch::HeaderEntry>>`) are never forwarded. There is a `// TODO: forward headers` comment at line 167 acknowledging this.

## Changes

### 1. Forward original response headers (`src/browser/instrumentation.rs`)

Replace lines 162-167 (the single `.response_header(...)` call + TODO comment) with `.response_headers(...)` (plural):

```rust
.response_headers(
    event.response_headers.iter().flatten()
        .filter(|h| !h.name.eq_ignore_ascii_case("etag"))
        .cloned()
        .chain(std::iter::once(fetch::HeaderEntry {
            name: "etag".to_string(),
            value: format!("{}", source_id.0),
        }))
)
```

This forwards all original response headers (including `Content-Type`), replaces the original `etag` with the computed `source_id` value. Verified: `HeaderEntry` derives `Clone`, and `FulfillRequestParamsBuilder::response_headers()` accepts `I: IntoIterator<Item = S>, S: Into<HeaderEntry>` (chromiumoxide 0.8.0).

### 2. Add integration test

**Fixture files** (new directory `tests/external-module-script/`):

- `tests/external-module-script/index.html`:
  ```html
  <!DOCTYPE html>
  <html>
  <body>
    <h1 id="result">WAITING</h1>
    <script type="module" src="/external-module-script/module.js"></script>
  </body>
  </html>
  ```

- `tests/external-module-script/module.js`:
  ```js
  document.getElementById("result").textContent = "LOADED";
  ```

**Test function** in `tests/integration_tests.rs` (`test_external_module_script`):
- Custom spec with `eventually(() => state.document.body.querySelector("#result")?.textContent === "LOADED").within(10, "seconds")`
- Export a fallback action (scroll) since the page has nothing interactive to click
- `Expect::Success` — passes when the module loads and changes DOM text from "WAITING" to "LOADED"

Pattern follows existing tests like `test_random_text_input` (custom spec with `eventually().within()`, `Expect::Success`, short timeout).

### 3. No changes needed for inline module scripts

The existing HTML instrumentation in `src/instrumentation/html.rs` correctly skips inline `<script type="module">` tags (line 64-65: `is_inline_javascript` is false when `script_type == "module"`). This is correct behavior — inline module scripts pass through without instrumentation.
