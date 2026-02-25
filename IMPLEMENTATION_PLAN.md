# Implementation Plan

All items have been implemented and tests pass.

## Completed

### Fix External Module Script MIME Type Issue (DONE)

**Problem:** `<script type="module" src="...">` failed because the instrumentation intercept dropped all original response headers when calling `Fetch.fulfillRequest`. Chrome enforced strict MIME type checking for ES modules, causing fatal load errors.

**Fix:** In `src/browser/instrumentation.rs`, replaced single `.response_header(etag)` with `.response_headers(...)` that forwards all original response headers (including `Content-Type`) while replacing the etag with the computed source ID. Added integration test `test_external_module_script` with fixture in `tests/external-module-script/`.

**Test:** `cargo test --test integration_tests test_external_module_script` — passes (module text changes from "WAITING" to "LOADED").
