# Implementation Plan

## Status

**Complete.** All fix code and tests implemented and verified passing.

- 78 unit tests pass (including 28 for the header/CSP fix)
- 15 integration tests pass (including 4 new ones for the fix)
- `cargo fmt` clean, `cargo clippy` clean

## What Was Fixed

The `instrument_js_coverage` function in `src/browser/instrumentation.rs` dropped **all** upstream response headers when fulfilling intercepted requests via CDP's `Fetch.fulfillRequest`, replacing them with a single synthetic `etag`. This silently removed every security and functional header (CORS, HSTS, CSP, content-type, etc.) from Script and Document responses.

## Fix Summary

- `STRIPPED_RESPONSE_HEADERS` denylist: only 5 headers invalidated by instrumentation are stripped
- `sanitize_csp`: resource-type-aware CSP handling (drop for Script, sanitize for Document)
- `build_response_headers`: assembles forwarded headers with CSP logic + synthetic etag
- 28 unit tests + 4 integration tests that each fail on the old code and pass on the fix
