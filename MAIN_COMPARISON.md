# Code Changes: main vs antithesishq/main

This document describes only the code that was altered or added in `main`
relative to `antithesishq/main` (commit `0e2913d`). Nothing described below
existed in the upstream.

---

## src/browser/instrumentation.rs

### Added: `STRIPPED_RESPONSE_HEADERS` constant (lines 18–41)

A module-level constant listing 7 headers to strip from intercepted responses
before fulfillment:

```rust
const STRIPPED_RESPONSE_HEADERS: &[&str] = &[
    "etag",
    "content-length",
    "content-encoding",
    "transfer-encoding",
    "content-security-policy",
    "content-security-policy-report-only",
    "strict-transport-security",
];
```

Each entry has a per-line comment explaining the rationale for stripping.

### Changed: `FulfillRequestParams` builder call (lines 183–205)

**Before (upstream):**

```rust
.response_header(fetch::HeaderEntry {
    name: "etag".to_string(),
    value: format!("{}", source_id.0),
})
// TODO: forward headers
```

The upstream used `.response_header(single)` (singular form), which created a
`responseHeaders` array containing only the synthetic etag. All original
response headers were dropped. The `// TODO: forward headers` comment
acknowledged this was incomplete.

**After (fork):**

```rust
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
        .cloned()
        .chain(std::iter::once(fetch::HeaderEntry {
            name: "etag".to_string(),
            value: format!("{}", source_id.0),
        })),
)
```

The fork uses `.response_headers(iterator)` (plural form), which populates the
`responseHeaders` array from the filtered original headers plus the synthetic
etag. The `// TODO` comment is removed.

Key behavioral change: original response headers are now **forwarded** (minus
the denylist), whereas upstream **dropped all** of them. This applies to all
fulfilled responses — scripts, HTML documents, and non-HTML documents alike.

---

## tests/integration_tests.rs

### Changed: imports (lines 1–6)

**Before (upstream):**

```rust
use axum::Router;
use tower_http::services::ServeDir;
```

**After (fork):**

```rust
use axum::{
    Router, extract::Request, http::HeaderValue, middleware, response::Response,
};
use tower_http::{compression::CompressionLayer, services::ServeDir};
```

Added: `extract::Request`, `http::HeaderValue`, `middleware`, `response::Response`
from `axum`; `compression::CompressionLayer` from `tower_http`.

### Changed: `run_browser_test` refactored into two functions (lines 54–220)

**Before (upstream):** A single function `run_browser_test(name, expect,
timeout, spec)` that created a `Router` internally, called `setup()`, acquired
the semaphore, and ran the full test.

**After (fork):** Split into:

1. `run_browser_test_with_router(name, expect, timeout, spec, app)` — the
   lower-level function that accepts a pre-built `Router`. Contains all the
   original logic. Has a one-line doc: `/// See [`run_browser_test`].`

2. `run_browser_test(name, expect, timeout, spec)` — thin wrapper that creates
   the default `Router::new().fallback_service(ServeDir::new("./tests"))` and
   delegates. Retains the full doc comment.

The `setup()` call moved from the wrapper to the lower-level function. The
`let app = ...` and `let app_other = app.clone()` lines moved accordingly.

Doc comment changes:
- Fixed typo: "facitiliate" → "facilitate"
- Fixed URL path: `http://localhost:{P}/tests/{name}` → `http://localhost:{P}/{name}`
  (the latter matches the actual code, which was already `format!("http://localhost:{}/{}", port, name)`)

### Added: `test_external_module_script` (lines 473–491)

Tests that a `<script type="module" src="...">` loads correctly after
instrumentation. Uses default `run_browser_test` with `Expect::Success`, 20s
timeout. Spec checks `eventually(() => resultText.current === "LOADED").within(10, "seconds")`.

### Added: `test_compressed_script` (lines 493–519)

Tests that a server sending gzip-compressed responses works after
instrumentation. Creates a custom `Router` with `CompressionLayer::new()` and
uses `run_browser_test_with_router`. `Expect::Success`, 20s timeout. Same spec
pattern.

### Added: `test_csp_script` (lines 521–563)

Tests that a server sending a `content-security-policy` header with a sha256
hash of the original script works after instrumentation (because Bombadil strips
the CSP header). Creates a custom `Router` with `middleware::from_fn` that
inserts a CSP header on every response. Uses `run_browser_test_with_router`.
`Expect::Success`, 20s timeout. Same spec pattern.

---

## Cargo.toml

### Changed: `tower-http` dev-dependency features (line 53)

**Before:** `tower-http = { version = "0.6.8", features = ["fs"] }`

**After:** `tower-http = { version = "0.6.8", features = ["fs", "compression-gzip"] }`

Added `compression-gzip` feature to support the `CompressionLayer` used in
`test_compressed_script`. This is a dev-dependency only — it does not affect the
production binary.

---

## Cargo.lock

Added three new transitive dependencies required by `compression-gzip`:
- `async-compression` 0.4.40
- `compression-codecs` 0.4.37
- `compression-core` 0.4.31

---

## New test fixture files

All three follow the same pattern: minimal HTML with a `<h1 id="result">WAITING</h1>`
and a script that sets its text content to `"LOADED"`.

### tests/external-module-script/index.html

```html
<!DOCTYPE html>
<html>
<body>
  <h1 id="result">WAITING</h1>
  <script type="module" src="/external-module-script/module.js"></script>
</body>
</html>
```

### tests/external-module-script/module.js

```js
document.getElementById("result").textContent = "LOADED";
```

### tests/compressed-script/index.html

```html
<!DOCTYPE html>
<html>
<body>
  <h1 id="result">WAITING</h1>
  <script src="/compressed-script/script.js"></script>
</body>
</html>
```

### tests/compressed-script/script.js

```js
document.getElementById("result").textContent = "LOADED";
```

### tests/csp-script/index.html

```html
<!doctype html>
<html>
  <body>
    <h1 id="result">WAITING</h1>
    <script src="/csp-script/script.js"></script>
  </body>
</html>
```

### tests/csp-script/script.js

```js
document.getElementById("result").textContent = "LOADED";
```

---

## PATTERNS.md (new file)

A 38-line coding conventions document covering integration tests, doc comments,
constants, header handling, error handling, and imports. This file did not exist
in upstream and is entirely new.
