# Implementation Plan

## TODO: Strip Stale Transport Headers After Instrumentation

**Problem:** The header forwarding in `src/browser/instrumentation.rs` (lines 163-176) forwards all original response headers (minus `etag`), but several headers describe properties of the **original** body that become incorrect after instrumentation transforms it:

- **`Content-Length`**: Original body size ≠ instrumented body size → truncated loads
- **`Content-Encoding`**: CDP's `GetResponseBody` returns decoded content → forwarding `gzip` causes garbage
- **`Transfer-Encoding`**: `fulfillRequest` delivers body as a single blob → `chunked` is incorrect

### Changes Required

#### 1. Fix: `src/browser/instrumentation.rs` (line 168-169)

Expand the filter from:
```rust
.filter(|h| !h.name.eq_ignore_ascii_case("etag"))
```
to:
```rust
.filter(|h| {
    !["etag", "content-length", "content-encoding", "transfer-encoding"]
        .iter()
        .any(|name| h.name.eq_ignore_ascii_case(name))
})
```

#### 2. Add `compression-gzip` feature to `tower-http` in `Cargo.toml`

```toml
tower-http = { version = "0.6.8", features = ["fs", "compression-gzip"] }
```

#### 3. Create test fixture `tests/compressed-script/index.html`

```html
<!DOCTYPE html>
<html>
<body>
  <h1 id="result">WAITING</h1>
  <script src="/compressed-script/script.js"></script>
</body>
</html>
```

#### 4. Create test fixture `tests/compressed-script/script.js`

```js
document.getElementById("result").textContent = "LOADED";
```

#### 5. Refactor test harness in `tests/integration_tests.rs`

Extract a `run_browser_test_with_router` function that takes a `Router` parameter. The existing `run_browser_test` calls it with the default `ServeDir` router. This lets the compression test pass a router with `CompressionLayer`.

Specifically:
- Rename current `run_browser_test` to `run_browser_test_with_router`, adding a `router: Router` parameter (replacing the internal `Router::new().fallback_service(ServeDir::new("./tests"))`)
- Create new `run_browser_test` that calls `run_browser_test_with_router` with the default router
- All existing tests remain unchanged

#### 6. Add `test_compressed_script` integration test

```rust
use tower_http::compression::CompressionLayer;

#[tokio::test]
async fn test_compressed_script() {
    let app = Router::new()
        .fallback_service(ServeDir::new("./tests"))
        .layer(CompressionLayer::new());
    run_browser_test_with_router(
        "compressed-script",
        Expect::Success,
        Duration::from_secs(10),
        Some(r#"
import { extract, eventually } from "@antithesishq/bombadil";
export { scroll } from "@antithesishq/bombadil/defaults";

const resultText = extract((state) => state.document.body?.querySelector("\#result")?.textContent ?? null);

export const compressed_script_loads = eventually(
  () => resultText.current === "LOADED"
).within(10, "seconds");
"#),
        app,
    ).await;
}
```

**Validation:** Before the fix, Chrome receives `Content-Encoding: gzip` with plaintext body → script fails → `#result` stays `"WAITING"` → timeout. After the fix, `Content-Encoding` is stripped → script loads → `#result` becomes `"LOADED"` → success.

## Completed

- Fix external module script MIME type issue (forwarding response headers in `Fetch.fulfillRequest`)
- Test for external module script (`test_external_module_script`)
