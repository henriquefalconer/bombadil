# Implementation Plan

## TODO: Strip Stale Transport Headers After Instrumentation

**Problem:** The header forwarding fix in `src/browser/instrumentation.rs` (lines 163-176) forwards all original response headers (minus `etag`), but several headers describe properties of the **original** body that become incorrect after instrumentation transforms it:

- **`Content-Length`**: Original body size ≠ instrumented body size. Could cause truncated loads.
- **`Content-Encoding`**: CDP's `GetResponseBody` returns decoded content. Forwarding `Content-Encoding: gzip` tells Chrome the body is still compressed → garbage.
- **`Transfer-Encoding`**: `fulfillRequest` delivers body as a single base64 blob. Forwarding `Transfer-Encoding: chunked` is incorrect.

### Code Change

In `src/browser/instrumentation.rs:168-169`, expand the filter from:

```rust
.filter(|h| !h.name.eq_ignore_ascii_case("etag"))
```

to also strip `content-length`, `content-encoding`, and `transfer-encoding` (all case-insensitive). For example:

```rust
.filter(|h| {
    !["etag", "content-length", "content-encoding", "transfer-encoding"]
        .iter()
        .any(|name| h.name.eq_ignore_ascii_case(name))
})
```

### Test: Integration test with gzip compression

The test must demonstrate that **before the fix** the problem occurs, and **after the fix** it doesn't.

1. **Add `compression-gzip` feature** to `tower-http` in `[dev-dependencies]` in `Cargo.toml`:
   ```toml
   tower-http = { version = "0.6.8", features = ["fs", "compression-gzip"] }
   ```

2. **Create test fixture** `tests/compressed-script/index.html`:
   ```html
   <!DOCTYPE html>
   <html>
   <body>
     <h1 id="result">WAITING</h1>
     <script src="/compressed-script/script.js"></script>
   </body>
   </html>
   ```

3. **Create test fixture** `tests/compressed-script/script.js`:
   ```js
   document.getElementById("result").textContent = "LOADED";
   ```

4. **Refactor `run_browser_test`** to accept an optional custom `Router` parameter. Extract the router creation so the new test can pass a router with `CompressionLayer` while existing tests continue using the default `ServeDir` router unchanged. Alternatively, extract a helper `run_browser_test_with_router` that takes a `Router` argument, and have the existing `run_browser_test` call it with the default router.

5. **Add test function** `test_compressed_script` in `tests/integration_tests.rs`:
   - Builds a router: `Router::new().fallback_service(ServeDir::new("./tests")).layer(CompressionLayer::new())`
   - Uses a custom spec that extracts `#result` textContent and asserts `eventually(() => resultText.current === "LOADED").within(10, "seconds")`
   - Expects `Expect::Success`
   - **Before fix**: Chrome receives `Content-Encoding: gzip` with plaintext body → script fails → `#result` stays `"WAITING"` → timeout
   - **After fix**: `Content-Encoding` is stripped → script loads → `#result` becomes `"LOADED"` → success

## Completed

- Fix external module script MIME type issue (forwarding response headers in `Fetch.fulfillRequest`)
