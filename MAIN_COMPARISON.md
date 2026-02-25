# Comparison: `main` vs `antithesishq/main`

This document describes **only the code that was altered** in `main` relative to `antithesishq/main`. Nothing that was already present in the upstream is described here.

---

## Overview

- `antithesishq/main` is fully contained within `main` (zero commits in upstream that aren't in main)
- `main` has 15 additional commits on top of the merge base `0e2913d`
- No files were deleted
- 4 files were added, 4 files were modified

| Status | File |
|--------|------|
| Added | `tests/compressed-script/index.html` |
| Added | `tests/compressed-script/script.js` |
| Added | `tests/external-module-script/index.html` |
| Added | `tests/external-module-script/module.js` |
| Modified | `src/browser/instrumentation.rs` |
| Modified | `tests/integration_tests.rs` |
| Modified | `Cargo.toml` |
| Modified | `Cargo.lock` |

---

## `src/browser/instrumentation.rs` — Header Forwarding Fix

### What was removed

```rust
.response_header(fetch::HeaderEntry {
    name: "etag".to_string(),
    value: format!("{}", source_id.0),
})
// TODO: forward headers
```

The `.response_header(...)` call (singular) set a single `etag` header. The `// TODO: forward headers` comment acknowledged that all original response headers were being dropped.

### What was added in its place

```rust
.response_headers(
    event
        .response_headers
        .iter()
        .flatten()
        .filter(|h| {
            ![
                "etag",
                "content-length",
                "content-encoding",
                "transfer-encoding",
            ]
            .iter()
            .any(
                |name| {
                    h.name.eq_ignore_ascii_case(name)
                },
            )
        })
        .cloned()
        .chain(std::iter::once(fetch::HeaderEntry {
            name: "etag".to_string(),
            value: format!("{}", source_id.0),
        })),
)
```

The `.response_headers(...)` call (plural) forwards all original response headers from `event.response_headers`, filtering out four headers by name (case-insensitive), then appends a synthetic `etag` with the computed `source_id`.

### Why each header is stripped

- **`etag`**: Replaced with the synthetic `source_id` value for coverage tracking.
- **`content-length`**: The instrumented body has a different size than the original.
- **`content-encoding`**: CDP's `GetResponseBody` returns the already-decompressed body. Forwarding `Content-Encoding: gzip` would cause Chrome to try decompressing plaintext.
- **`transfer-encoding`**: Not applicable for `Fetch.fulfillRequest` responses (CDP delivers the body directly, not over HTTP).

---

## `tests/integration_tests.rs` — Test Infrastructure and New Tests

### Change 1: Import added

```rust
// Was:
use tower_http::services::ServeDir;

// Now:
use tower_http::{compression::CompressionLayer, services::ServeDir};
```

`CompressionLayer` is imported for the compressed-script test.

### Change 2: `run_browser_test` refactored into two functions

The original `run_browser_test` function was renamed to `run_browser_test_with_router` with an additional `app: Router` parameter. A new `run_browser_test` wrapper was created that passes the default router.

**`run_browser_test_with_router`** (renamed from `run_browser_test`):
- Signature: `async fn run_browser_test_with_router(name, expect, timeout, spec, app)`
- Takes a custom `Router` instead of creating one internally.
- Doc comment was modified: "expectation." became "expectation and a custom router."
- Body unchanged except: the line `let app = Router::new().fallback_service(ServeDir::new("./tests"));` was removed (now passed as parameter).

**`run_browser_test`** (new wrapper):
```rust
async fn run_browser_test(
    name: &str,
    expect: Expect,
    timeout: Duration,
    spec: Option<&str>,
) {
    let app = Router::new().fallback_service(ServeDir::new("./tests"));
    run_browser_test_with_router(name, expect, timeout, spec, app).await;
}
```

All existing tests continue to call `run_browser_test` with no changes to their call sites.

### Change 3: `test_external_module_script` added

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

Tests that ES module scripts (`<script type="module">`) load correctly when Bombadil intercepts and instruments the response. Uses the default router (no compression).

### Change 4: `test_compressed_script` added

```rust
/// Verifies that scripts served with `Content-Encoding: gzip` load correctly after
/// Bombadil intercepts and instruments them.
///
/// When Bombadil intercepts a gzip-compressed response, CDP's `GetResponseBody` returns
/// the already-decoded body. If Bombadil then forwards the original `Content-Encoding:
/// gzip` header, Chrome treats the plaintext as gzip-compressed data and fails to parse
/// the script. The fix strips stale transport headers (`Content-Encoding`,
/// `Content-Length`, `Transfer-Encoding`) before calling `Fetch.fulfillRequest`.
#[tokio::test]
async fn test_compressed_script() {
    let app = Router::new()
        .fallback_service(ServeDir::new("./tests"))
        .layer(CompressionLayer::new());
    run_browser_test_with_router(
        "compressed-script",
        Expect::Success,
        Duration::from_secs(10),
        Some(
            r#"
import { extract, eventually } from "@antithesishq/bombadil";
export { scroll } from "@antithesishq/bombadil/defaults";

const resultText = extract((state) => state.document.body?.querySelector("\#result")?.textContent ?? null);

export const compressed_script_loads = eventually(
  () => resultText.current === "LOADED"
).within(10, "seconds");
"#,
        ),
        app,
    )
    .await;
}
```

Tests that gzip-compressed scripts load correctly after instrumentation. Uses a custom router with `CompressionLayer` to serve gzip-compressed responses. This is the only test that uses `run_browser_test_with_router` directly.

---

## New Test Fixtures

### `tests/external-module-script/index.html`

```html
<!DOCTYPE html>
<html>
<body>
  <h1 id="result">WAITING</h1>
  <script type="module" src="/external-module-script/module.js"></script>
</body>
</html>
```

### `tests/external-module-script/module.js`

```javascript
document.getElementById("result").textContent = "LOADED";
```

### `tests/compressed-script/index.html`

```html
<!DOCTYPE html>
<html>
<body>
  <h1 id="result">WAITING</h1>
  <script src="/compressed-script/script.js"></script>
</body>
</html>
```

### `tests/compressed-script/script.js`

```javascript
document.getElementById("result").textContent = "LOADED";
```

Both fixture pairs follow the same pattern: start with `WAITING` in the DOM, script changes it to `LOADED`. The test spec uses `eventually(() => resultText.current === "LOADED").within(10, "seconds")`.

---

## `Cargo.toml` — Dependency Feature Addition

```toml
# Was:
tower-http = { version = "0.6.8", features = ["fs"] }

# Now:
tower-http = { version = "0.6.8", features = ["fs", "compression-gzip"] }
```

The `compression-gzip` feature enables `tower_http::compression::CompressionLayer`, used only in `test_compressed_script`.

## `Cargo.lock` — Transitive Dependencies

Three new crates were added as transitive dependencies of `tower-http`'s `compression-gzip` feature:

- `async-compression` 0.4.40 (depends on `compression-codecs`, `compression-core`, `pin-project-lite`, `tokio`)
- `compression-codecs` 0.4.37 (depends on `compression-core`, `flate2`, `memchr`)
- `compression-core` 0.4.31 (no dependencies)

`tower-http` gained `async-compression` as a dependency.
