# Implementation Plan

## TODO: Strip Stale Transport Headers After Instrumentation

**Problem:** The header forwarding fix in `src/browser/instrumentation.rs` (lines 163-176) forwards all original response headers (minus `etag`), but several headers describe properties of the **original** body that become incorrect after instrumentation transforms it:

- **`Content-Length`**: Original body size ≠ instrumented body size (instrumented is larger due to coverage hooks). Could cause truncated loads.
- **`Content-Encoding`**: CDP's `GetResponseBody` returns decoded (decompressed) content. Forwarding `Content-Encoding: gzip` tells Chrome the body is still compressed → Chrome tries to decompress plaintext → garbage. Affects any server using compression (i.e., most production servers).
- **`Transfer-Encoding`**: `fulfillRequest` delivers body as a single base64 blob. Forwarding `Transfer-Encoding: chunked` is incorrect.

**Fix:** Expand the filter predicate at `src/browser/instrumentation.rs:168-169` from:

```rust
.filter(|h| !h.name.eq_ignore_ascii_case("etag"))
```

to also strip `content-length`, `content-encoding`, and `transfer-encoding` (all case-insensitive).

**Test — Integration test with gzip compression:**

1. Add `compression-gzip` feature to `tower-http` in `[dev-dependencies]` in `Cargo.toml`.
2. Create test fixture `tests/compressed-script/index.html` — a page that loads an external `<script src="...">` which sets a visible flag (like the existing `external-module-script` test pattern).
3. Create `tests/compressed-script/script.js` — sets `#result` textContent to `"LOADED"`.
4. Add a new test function in `tests/integration_tests.rs` that:
   - Uses a custom Axum router with `tower_http::compression::CompressionLayer` to serve gzip-compressed responses.
   - This requires a variant of `run_browser_test` or a standalone test that sets up the compressed server.
   - Asserts `Expect::Success` with an `eventually(() => resultText.current === "LOADED")` spec.
   - **Before fix**: Chrome receives `Content-Encoding: gzip` but body is plaintext → script fails to parse → `#result` stays `"WAITING"` → test times out/fails.
   - **After fix**: `Content-Encoding` is stripped → Chrome treats body as plaintext → script loads → test passes.

## Completed

- Fix external module script MIME type issue (forwarding response headers in `Fetch.fulfillRequest`)
