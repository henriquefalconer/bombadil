# Code Changes: main vs antithesishq/main

Common ancestor: `0e2913d` ("link to manual from README (#54)").
`antithesishq/main` has not moved since the ancestor. All changes below are additions or modifications made on the local `main` branch.

## Files modified

### `src/browser/instrumentation.rs`

**Change 1: New constant `STRIPPED_RESPONSE_HEADERS` (inserted before `instrument_js_coverage`)**

```rust
/// Response headers that must be stripped after script instrumentation.
///
/// Each entry is lower-cased for case-insensitive matching.
///
/// Note: `content-security-policy` and `content-security-policy-report-only` are NOT
/// listed here. CSP stripping is resource-type-aware: for Script responses the whole
/// header is dropped (script body instrumentation invalidates hash-based `script-src`
/// values); for Document responses the header is sanitised via [`sanitize_csp`] instead
/// of being removed wholesale. See the `FulfillRequestParams` construction below.
const STRIPPED_RESPONSE_HEADERS: &[&str] = &[
    // Replaced with an instrumentation-stable source ID derived from the
    // original ETag or body hash, so the upstream value is always stale.
    "etag",
    // Body size changes when we rewrite the script, so the declared length
    // no longer matches the actual bytes sent.
    "content-length",
    // CDP already returns a decompressed body; re-advertising a compression
    // encoding would cause the browser to double-decompress.
    "content-encoding",
    // Same reason as content-encoding: the transfer framing is gone once CDP
    // hands us the raw bytes.
    "transfer-encoding",
    // The Digest header (RFC 3230 / RFC 9530) contains a hash of the response
    // body. After instrumentation that hash is wrong; a service worker
    // validating it would reject the instrumented script.
    "digest",
];
```

**Change 2: Replaced `FulfillRequestParams` header construction**

The original code (antithesishq/main) used `.response_header()` (singular) with a single synthetic ETag and a `// TODO: forward headers` comment:

```rust
// REMOVED from antithesishq/main:
.response_header(fetch::HeaderEntry {
    name: "etag".to_string(),
    value: format!("{}", source_id.0),
})
// TODO: forward headers
```

Replaced with a new line `let resource_type = event.resource_type.clone();` before the `page.execute(...)` block, and the entire header construction changed to `.response_headers()` (plural) with an iterator chain:

```rust
// ADDED in main:
let resource_type = event.resource_type.clone();

// ... inside the builder:
.response_headers(
    event
        .response_headers
        .iter()
        .flatten()
        .filter(|h| {
            !STRIPPED_RESPONSE_HEADERS.iter().any(|name| {
                h.name.eq_ignore_ascii_case(name)
            })
        })
        .flat_map(move |h| {
            let is_csp = h.name.eq_ignore_ascii_case(
                "content-security-policy",
            ) || h.name.eq_ignore_ascii_case(
                "content-security-policy-report-only",
            );
            if is_csp {
                match resource_type {
                    network::ResourceType::Script => None,
                    _ => sanitize_csp(&h.value).map(|v| fetch::HeaderEntry {
                        name: h.name.clone(),
                        value: v,
                    }),
                }
            } else {
                Some(h.clone())
            }
        })
        .chain(std::iter::once(fetch::HeaderEntry {
            name: "etag".to_string(),
            value: format!("{}", source_id.0),
        })),
)
```

**Change 3: New function `sanitize_csp` (appended after `source_id`)**

```rust
fn sanitize_csp(csp_value: &str) -> Option<String> {
    let mut result: Vec<String> = Vec::new();
    for directive in csp_value.split(';') {
        let directive = directive.trim();
        if directive.is_empty() {
            continue;
        }
        let lower = directive.to_lowercase();
        let is_script_src = lower.starts_with("script-src ")
            || lower == "script-src"
            || lower.starts_with("script-src-elem ")
            || lower == "script-src-elem";
        if is_script_src {
            let mut parts = directive.splitn(2, char::is_whitespace);
            let name = parts.next().unwrap_or("");
            let values_str = parts.next().unwrap_or("").trim();
            let filtered: Vec<&str> = values_str
                .split_whitespace()
                .filter(|v| {
                    let lv = v.to_lowercase();
                    !lv.starts_with("'sha256-")
                        && !lv.starts_with("'sha384-")
                        && !lv.starts_with("'sha512-")
                        && !lv.starts_with("'nonce-")
                })
                .collect();
            if !filtered.is_empty() {
                result.push(format!("{} {}", name, filtered.join(" ")));
            }
        } else {
            result.push(directive.to_string());
        }
    }
    if result.is_empty() {
        None
    } else {
        Some(result.join("; "))
    }
}
```

**Change 4: New unit test module (appended at end of file)**

10 unit tests for `sanitize_csp` covering: SHA-256 removal, SHA-384 removal, SHA-512 removal, nonce removal, mixed directives, no-script-src passthrough, empty result, multiple hashes with safe value, only-hash directive with other directives, and `script-src-elem`.

### `tests/integration_tests.rs`

**Change 5: Import additions**

```rust
// REMOVED from antithesishq/main:
use axum::Router;
use tower_http::services::ServeDir;

// ADDED in main:
use axum::{
    Router, extract::Request, http::HeaderValue, middleware, response::Response,
};
use tower_http::{compression::CompressionLayer, services::ServeDir};
```

**Change 6: `run_browser_test` split into two functions**

The original `run_browser_test` function was split. Its body was moved into a new `run_browser_test_with_router` that accepts a `Router` parameter. The original `run_browser_test` became a thin wrapper:

```rust
// ADDED in main:
/// See [`run_browser_test`].
async fn run_browser_test_with_router(
    name: &str,
    expect: Expect,
    timeout: Duration,
    spec: Option<&str>,
    app: Router,
) {
    // ... body moved here from the original run_browser_test ...
}

// MODIFIED in main (was the full function, now a wrapper):
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

Inside the body, two incidental changes: the `let app = Router::new()...` line moved from inside the function to the wrapper (it now arrives as a parameter), and the `app_other` clone was added as `let app_other = app.clone();`.

The doc comment was also corrected: "facitiliate" → "facilitate", and `http://localhost:{P}/tests/{name}` → `http://localhost:{P}/{name}`.

**Change 7: Four new integration test functions**

`test_external_module_script` (line 475): Uses default router, 20s timeout. Spec checks `#result` text becomes `"LOADED"` within 10s.

`test_compressed_script` (line 497): Constructs a custom router with `CompressionLayer::new()`, calls `run_browser_test_with_router`. 20s timeout. Spec checks `#result` text becomes `"LOADED"` within 10s.

`test_csp_script` (line 523): Constructs a custom router with `middleware::from_fn()` that inserts a `content-security-policy` header containing `script-src 'sha256-sRoPO3cqhmVEQTMEK66eATz8J/LJdrvqrNVuMKzGgSM='`. 20s timeout. Spec checks `#result` text becomes `"LOADED"` within 10s.

`test_csp_document_directives_preserved` (line 566): Constructs a custom router with `middleware::from_fn()` that inserts a `content-security-policy` header containing `script-src 'unsafe-inline' 'self' 'sha256-AAAA...'; img-src 'self'`. 20s timeout. Spec checks `#result` text becomes `"CSP_ACTIVE"` within 10s.

### `Cargo.toml`

**Change 8: Added `compression-gzip` feature to `tower-http`**

```toml
# antithesishq/main:
tower-http = { version = "0.6.8", features = ["fs"] }

# main:
tower-http = { version = "0.6.8", features = ["fs", "compression-gzip"] }
```

### `Cargo.lock`

Three new transitive crate entries: `async-compression`, `compression-codecs`, `compression-core`. The `tower-http` entry gained `async-compression` as a dependency.

## Files added (not present in antithesishq/main)

### `PATTERNS.md`

39-line project conventions document covering integration tests, doc comments, constants, header handling, error handling, and imports.

### `tests/external-module-script/index.html`

```html
<html>
  <head>
    <title>External Module Script</title>
  </head>
  <body>
    <h1 id="result">WAITING</h1>
    <script type="module" src="/external-module-script/module.js"></script>
  </body>
</html>
```

### `tests/external-module-script/module.js`

```js
document.getElementById("result").textContent = "LOADED";
```

### `tests/compressed-script/index.html`

```html
<html>
  <head>
    <title>Compressed Script</title>
  </head>
  <body>
    <h1 id="result">WAITING</h1>
    <script src="/compressed-script/script.js"></script>
  </body>
</html>
```

### `tests/compressed-script/script.js`

```js
document.getElementById("result").textContent = "LOADED";
```

### `tests/csp-script/index.html`

```html
<html>
  <head>
    <title>CSP Script</title>
  </head>
  <body>
    <h1 id="result">WAITING</h1>
    <script src="/csp-script/script.js"></script>
  </body>
</html>
```

### `tests/csp-script/script.js`

```js
document.getElementById("result").textContent = "LOADED";
```

### `tests/csp-document/index.html`

```html
<html>
  <head>
    <title>CSP Document</title>
  </head>
  <body>
    <h1 id="result">WAITING</h1>
    <script>
      document.addEventListener('securitypolicyviolation', function() {
        document.getElementById('result').textContent = 'CSP_ACTIVE';
      });
      var img = document.createElement('img');
      img.src = 'https://external.invalid/x.png';
      document.body.appendChild(img);
    </script>
  </body>
</html>
```
