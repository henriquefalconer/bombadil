# Implementation Plan: Fix External Module Script MIME Type Issue

## Problem

`<script type="module" src="...">` tags fail to load under Bombadil because the instrumentation intercept (`src/browser/instrumentation.rs`) drops all original response headers when calling `Fetch.fulfillRequest`. Chrome enforces strict MIME type checking for ES module scripts, and the missing `Content-Type` header causes a fatal error.

## Root Cause

In `src/browser/instrumentation.rs` lines 158-174, `FulfillRequestParams` is built with only a single `etag` header. The original response headers (available via `event.response_headers`) are never forwarded. There is a `// TODO: forward headers` comment acknowledging this.

## Fix

### 1. Forward original response headers in `FulfillRequestParams` (`src/browser/instrumentation.rs`)

- Read `event.response_headers` (type `Option<Vec<fetch::HeaderEntry>>`)
- Build the response headers list by:
  1. Starting with the original headers (if present), filtering out the original `etag` header to avoid duplication
  2. Appending the computed `etag` header with the `source_id` value
- Pass the full headers list to the builder via `.response_headers(...)` instead of the single `.response_header(...)` call
- Remove the `// TODO: forward headers` comment

### 2. Add integration test for external module scripts (`tests/integration_tests.rs` + fixture)

- Create `tests/external-module-script/index.html`: an HTML page with `<script type="module" src="/tests/external-module-script/module.js"></script>` that sets a DOM element's text when loaded
- Create `tests/external-module-script/module.js`: a simple ES module that modifies the DOM (e.g., sets `textContent` on an element)
- Add `test_external_module_script` test in `integration_tests.rs` that:
  - Uses a custom spec with an `eventually(() => ...).within(10, "seconds")` property checking the module executed (DOM element changed)
  - Expects `Success`
- This test will fail before the fix and pass after

### 3. Verify inline module scripts still work

- The existing HTML instrumentation in `src/instrumentation/html.rs` correctly skips inline `<script type="module">` tags (line 64-65: `is_inline_javascript` is false when `script_type == "module"`)
- Inline module scripts are passed through without instrumentation, which is correct behavior
- No changes needed here, but the new integration test fixture could optionally include an inline module script to confirm no regression
