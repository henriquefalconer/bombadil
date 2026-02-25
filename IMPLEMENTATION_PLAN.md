# Implementation Plan: Fix External Module Script MIME Type Issue

## Problem

`<script type="module" src="...">` tags fail to load under Bombadil because the instrumentation intercept (`src/browser/instrumentation.rs`) drops all original response headers when calling `Fetch.fulfillRequest`. Chrome enforces strict MIME type checking for ES module scripts, and the missing `Content-Type` header causes a fatal error.

## Root Cause

In `src/browser/instrumentation.rs` lines 158-174, `FulfillRequestParams` is built with only a single `etag` header. The original response headers (available via `event.response_headers: Option<Vec<fetch::HeaderEntry>>`) are never forwarded. There is a `// TODO: forward headers` comment acknowledging this.

## Fix

### 1. Forward original response headers in `FulfillRequestParams` (`src/browser/instrumentation.rs`)

Replace the single `.response_header(...)` call (lines 163-167) with `.response_headers(...)` (plural), passing an iterator that:
1. Takes `event.response_headers` (via `.iter().flatten()`), filtering out the original `etag` to avoid duplication
2. Chains the computed `etag` header with the `source_id` value

Conceptual code:
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
// Remove the `// TODO: forward headers` comment
```

The `.response_headers()` method accepts `I: IntoIterator<Item = S>, S: Into<HeaderEntry>`, confirmed available in `chromiumoxide 0.8.0`.

### 2. Add integration test (`tests/integration_tests.rs` + fixture)

**Fixture files:**
- `tests/external-module-script/index.html`: HTML page with `<script type="module" src="/external-module-script/module.js"></script>` that has a `<h1 id="result">WAITING</h1>` element
- `tests/external-module-script/module.js`: ES module that sets `document.getElementById("result").textContent = "LOADED"`

**Test function** (`test_external_module_script`):
- Custom spec with `eventually(() => ...).within(10, "seconds")` checking `querySelector("#result").textContent === "LOADED"`
- Also export default actions (at minimum `clicks` or a fallback action like a scroll)
- `Expect::Success` — test passes when the module executes and changes the DOM

This test will fail before the fix (module never loads, stays "WAITING") and pass after.

### 3. No changes needed for inline module scripts

The existing HTML instrumentation in `src/instrumentation/html.rs` correctly skips inline `<script type="module">` tags (line 65: `is_inline_javascript` is false when `script_type == "module"`). Inline module scripts pass through without instrumentation, which is correct.
