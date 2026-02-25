# Comparison: develop vs antithesishq/main

Only altered code is documented below. Unchanged code is not reproduced. Non-code files (documentation, prompts, scripts, build config) are excluded.

---

## 1. `src/browser/instrumentation.rs`

### 1.1 New constant: `STRIPPED_RESPONSE_HEADERS`

Module-level `&[&str]` listing five lowercase header names stripped from every fulfilled response: `etag`, `content-length`, `content-encoding`, `transfer-encoding`, `digest`. Each entry has an inline `//` comment explaining why it must be removed. A `///` doc comment explains why CSP headers are NOT in this list (they receive resource-type-aware handling instead).

**antithesishq/main:** No header strip list. All headers were replaced by a single synthetic `etag`.

### 1.2 Changed: `FulfillRequestParams` builder call

**antithesishq/main:**
```rust
.response_header(fetch::HeaderEntry {
    name: "etag".to_string(),
    value: format!("{}", source_id.0),
})
// TODO: forward headers
```

Used `.response_header()` (singular), which due to CDP replacement semantics replaced the entire upstream header set with just the `etag`. All original headers — including `content-type` — were silently dropped.

**develop:**
```rust
let resource_type = event.resource_type.clone();

// ...inside builder:
.response_headers(build_response_headers(
    &event.response_headers,
    &resource_type,
    source_id,
))
```

Captures `resource_type` before the builder borrows `event`, then delegates to the new `build_response_headers` helper via `.response_headers()` (plural).

### 1.3 New function: `sanitize_csp`

Private function (`fn sanitize_csp(csp_value: &str) -> Option<String>`) that:
- Splits CSP on `;` into directives
- Detects whether `script-src` or `script-src-elem` is present
- For `script-src`, `script-src-elem`, and `default-src` (when no explicit script-src exists): strips `'sha256-…'`, `'sha384-…'`, `'sha512-…'`, `'nonce-…'`, and `'strict-dynamic'` values
- Strips `report-uri` and `report-to` directives entirely
- Omits a directive if all its values were stripped
- Returns `None` when no directives remain

**antithesishq/main:** No CSP handling.

### 1.4 New function: `build_response_headers`

Private function that encapsulates the header construction pipeline:
1. Iterates upstream `response_headers` via `.iter().flatten()`
2. `.filter()` removes headers matching `STRIPPED_RESPONSE_HEADERS` (case-insensitive)
3. `.flat_map()` applies resource-type-aware CSP handling: `Script` → drop CSP entirely; `Document` → sanitise via `sanitize_csp`; `_ =>` → preserve unchanged
4. `.chain()` appends a synthetic `etag` with `source_id`

**antithesishq/main:** No equivalent. Header construction was a single `.response_header()` call.

### 1.5 New unit test module

`#[cfg(test)] mod tests` with 26 tests:
- 19 for `sanitize_csp` (hash removal for SHA-256/384/512, nonce removal, mixed directives, empty results, `script-src-elem`, `default-src` fallback logic, `strict-dynamic` orphan removal, `report-uri`/`report-to` stripping)
- 7 for `build_response_headers` (stripped header removal, content-type preservation, CSP drop for Script, CSP sanitise for Document, synthetic etag, None input, passthrough)
- Two test helpers: `hdr(name, value)` and `sid(n)`

**antithesishq/main:** No unit tests in this file.

---

## 2. `tests/integration_tests.rs`

### 2.1 Import additions

**antithesishq/main:**
```rust
use axum::Router;
use tower_http::services::ServeDir;
```

**develop:**
```rust
use axum::{
    Router, extract::Request, http::HeaderValue, middleware, response::Response,
};
use tower_http::{compression::CompressionLayer, services::ServeDir};
```

### 2.2 `run_browser_test` split into wrapper + `run_browser_test_with_router`

The original function body moved into `run_browser_test_with_router(name, expect, timeout, spec, app)`. The wrapper `run_browser_test` creates the default `ServeDir` router and delegates.

Doc comment changes: "facitiliate" → "facilitate" (typo fix); URL pattern `http://localhost:{P}/tests/{name}.` → `http://localhost:{P}/{name}`.

`run_browser_test_with_router` has a one-liner doc comment: `/// See [`run_browser_test`].`

### 2.3 `make_csp_router` helper

```rust
fn make_csp_router(csp: &'static str) -> Router
```

Creates a router with `ServeDir` + middleware that injects `content-security-policy` on all responses. Shared by the two CSP integration tests.

### 2.4 Four new integration tests

All use `Duration::from_secs(30)` and `Expect::Success`. Each has a custom spec with `clicks` as the baseline action and an `eventually(...).within(10, "seconds")` property.

| Test | Router | Fixture | Verifies |
|------|--------|---------|----------|
| `test_external_module_script` | default | `external-module-script/` (`<script type="module">`) | Module scripts load after instrumentation |
| `test_compressed_script` | `CompressionLayer::new()` | `compressed-script/` | Gzip-compressed scripts load after instrumentation |
| `test_csp_script` | `make_csp_router(hash CSP)` | `csp-script/` | Scripts load when CSP hash is invalidated by instrumentation |
| `test_csp_document_directives_preserved` | `make_csp_router(mixed CSP)` | `csp-document/` | Document CSP: script hashes stripped, `img-src` preserved |

CSP tests have a single-line `//` comment summarising purpose.

### 2.5 New HTML fixture directories

| Directory | Content |
|-----------|---------|
| `tests/shared/script.js` | `document.getElementById("result").textContent = "LOADED";` — shared by three fixtures |
| `tests/external-module-script/` | `index.html` with `<script type="module" src="/shared/script.js">` |
| `tests/compressed-script/` | `index.html` with `<script src="/shared/script.js">` |
| `tests/csp-script/` | `index.html` with `<script src="/shared/script.js">` |
| `tests/csp-document/` | `index.html` with inline script that listens for `securitypolicyviolation` → sets `#result` to `"CSP_ACTIVE"` |

All follow existing fixture structure: `<html>`, `<head>`, `<title>`, `<body>`, `<h1 id="result">WAITING</h1>`.

---

## 3. `Cargo.toml`

**antithesishq/main:**
```toml
tower-http = { version = "0.6.8", features = ["fs"] }
```

**develop:**
```toml
tower-http = { version = "0.6.8", features = ["fs", "compression-gzip"] }
```

Dev-dependency only. Enables `CompressionLayer` for `test_compressed_script`. Adds three transitive crates to `Cargo.lock`: `async-compression`, `compression-codecs`, `compression-core`.
