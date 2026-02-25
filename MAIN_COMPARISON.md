# Main vs antithesishq/main — Changed Code Only

This document shows exactly what code was altered in `main` relative to `antithesishq/main`. Nothing from the original codebase is described here — only the additions, modifications, and their immediate context.

---

## Modified Files

### `src/browser/instrumentation.rs` (lines 158–206)

**What was there (antithesishq/main):**

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
        .map_err(|error| {
            anyhow!(
                "failed building FulfillRequestParams: {}",
                error
            )
        })?,
)
```

**What replaced it (main):**

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
                    ![
                        "etag",
                        "content-length",
                        "content-encoding",
                        "transfer-encoding",
                        "content-security-policy",
                        "content-security-policy-report-only",
                        "strict-transport-security",
                    ]
                    .iter()
                    .any(|name| h.name.eq_ignore_ascii_case(name))
                })
                .cloned()
                .chain(std::iter::once(fetch::HeaderEntry {
                    name: "etag".to_string(),
                    value: format!("{}", source_id.0),
                })),
        )
        .build()
        .map_err(|error| {
            anyhow!(
                "failed building FulfillRequestParams: {}",
                error
            )
        })?,
)
```

**Summary of change:** Replaced `.response_header()` (single etag) with `.response_headers()` (all original headers forwarded, minus a denylist of 7 header names, plus a fresh etag appended). The `// TODO: forward headers` comment was resolved.

---

### `tests/integration_tests.rs`

#### Import additions

```rust
// Was:
use axum::Router;
use tower_http::services::ServeDir;

// Now:
use axum::{
    Router, extract::Request, http::HeaderValue, middleware, response::Response,
};
use tower_http::{compression::CompressionLayer, services::ServeDir};
```

#### Helper refactoring

The original `run_browser_test` was split into two functions:

**New lower-level function (`run_browser_test_with_router`):**
```rust
/// See [`run_browser_test`].
async fn run_browser_test_with_router(
    name: &str,
    expect: Expect,
    timeout: Duration,
    spec: Option<&str>,
    app: Router,  // <-- accepts a custom Router
) {
    // Contains all the body that was previously in run_browser_test,
    // except Router construction is moved out.
    // `let app_other = app.clone();` replaces the inline Router::new().
}
```

**New higher-level wrapper (`run_browser_test`):**
```rust
/// Run a named browser test with a given expectation.
///
/// Spins up two web servers: one on a random port P, and one on port P + 1, in order to
/// facilitate multi-domain tests.
///                                   ^^^^^^^^^^
///             (typo fix: was "facitiliate" in antithesishq/main)
///
/// The test starts at:
///
///     http://localhost:{P}/tests/{name}.
///
/// Which means that every named test case directory should have an index.html file.
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

#### Three new test functions

**`test_external_module_script` (line 474):**
```rust
#[tokio::test]
async fn test_external_module_script() {
    run_browser_test(
        "external-module-script",
        Expect::Success,
        Duration::from_secs(20),
        Some(r#"
import { extract, eventually } from "@antithesishq/bombadil";
export { clicks } from "@antithesishq/bombadil/defaults";

const resultText = extract((state) => state.document.body?.querySelector("\#result")?.textContent ?? null);

export const module_script_loads = eventually(
  () => resultText.current === "LOADED"
).within(10, "seconds");
"#),
    ).await;
}
```

**`test_compressed_script` (line 497):**
```rust
#[tokio::test]
async fn test_compressed_script() {
    let app = Router::new()
        .fallback_service(ServeDir::new("./tests"))
        .layer(CompressionLayer::new());
    run_browser_test_with_router(
        "compressed-script",
        Expect::Success,
        Duration::from_secs(20),
        Some(r#"
import { extract, eventually } from "@antithesishq/bombadil";
export { clicks } from "@antithesishq/bombadil/defaults";

const resultText = extract((state) => state.document.body?.querySelector("\#result")?.textContent ?? null);

export const compressed_script_loads = eventually(
  () => resultText.current === "LOADED"
).within(10, "seconds");
"#),
        app,
    ).await;
}
```

**`test_csp_script` (line 522):**
```rust
#[tokio::test]
async fn test_csp_script() {
    let app = Router::new()
        .fallback_service(ServeDir::new("./tests"))
        .layer(middleware::from_fn(
            |req: Request, next: middleware::Next| async move {
                let mut response: Response = next.run(req).await;
                response.headers_mut().insert(
                    axum::http::header::HeaderName::from_static(
                        "content-security-policy",
                    ),
                    HeaderValue::from_static(
                        "script-src 'sha256-sRoPO3cqhmVEQTMEK66eATz8J/LJdrvqrNVuMKzGgSM='",
                    ),
                );
                response
            },
        ));
    run_browser_test_with_router(
        "csp-script",
        Expect::Success,
        Duration::from_secs(20),
        Some(r#"
import { extract, eventually } from "@antithesishq/bombadil";
export { clicks } from "@antithesishq/bombadil/defaults";

const resultText = extract((state) => state.document.body?.querySelector("\#result")?.textContent ?? null);

export const csp_script_loads = eventually(
  () => resultText.current === "LOADED"
).within(10, "seconds");
"#),
        app,
    ).await;
}
```

---

### `Cargo.toml` (dev-dependencies)

```diff
-tower-http = { version = "0.6.8", features = ["fs"] }
+tower-http = { version = "0.6.8", features = ["fs", "compression-gzip"] }
```

---

## Added Files

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

### `tests/csp-script/index.html`
```html
<!doctype html>
<html>
  <head><title>CSP Script Test</title></head>
  <body>
    <h1 id="result">WAITING</h1>
    <script src="/csp-script/script.js"></script>
  </body>
</html>
```

### `tests/csp-script/script.js`
```javascript
document.getElementById("result").textContent = "LOADED";
```

### `PATTERNS.md`
A documentation file with coding conventions derived from the upstream codebase. See the file itself for full content.

---

## Unchanged Files

All other files in the repository are byte-identical between `main` and `antithesishq/main`. The `Cargo.lock` changes are purely transitive (adding `async-compression`, `compression-codecs`, `compression-core` as dependencies of `tower-http`'s `compression-gzip` feature).
