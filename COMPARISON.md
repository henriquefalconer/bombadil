# Code Comparison: develop vs antithesishq/main

This document describes **only the code that was changed** on `develop` relative to `antithesishq/main`. Documentation-only files, build configuration, and lock files are excluded unless they contain behavioral changes.

---

## Change 1: STRIPPED_RESPONSE_HEADERS constant

**File**: `src/browser/instrumentation.rs` (lines 18-44)

**Before (antithesishq/main)**: No constant. All upstream response headers were silently dropped — the `FulfillRequestParams` builder used `.response_header()` (singular) with only a synthetic `etag`, replacing the entire header set. A `// TODO: forward headers` comment acknowledged this gap.

**After (develop)**: Adds a module-level `const STRIPPED_RESPONSE_HEADERS: &[&str]` containing five lowercase header names: `"etag"`, `"content-length"`, `"content-encoding"`, `"transfer-encoding"`, `"digest"`. Each entry has an inline comment explaining why it must be removed. A doc comment above the constant explains why CSP headers are excluded (they require resource-type-aware handling).

---

## Change 2: FulfillRequestParams builder rewrite

**File**: `src/browser/instrumentation.rs` (lines 186-198)

**Before**:
```rust
.response_header(fetch::HeaderEntry {
    name: "etag".to_string(),
    value: format!("{}", source_id.0),
})
// TODO: forward headers
```

**After**:
```rust
// Capture resource type before the iterator borrows `event`.
let resource_type = event.resource_type.clone();
// ...
.response_headers(build_response_headers(
    &event.response_headers,
    &resource_type,
    source_id,
))
```

Changed from `.response_header()` (singular — one header replaces all) to `.response_headers()` (plural — full header list). Adds `let resource_type = event.resource_type.clone()` before the builder to avoid a borrow conflict.

---

## Change 3: sanitize_csp function

**File**: `src/browser/instrumentation.rs` (lines 259-353)

**Before**: No CSP handling.

**After**: Adds `fn sanitize_csp(csp_value: &str) -> Option<String>` (private, ~60 lines) which:
1. Splits on `;` into directives, trims whitespace, filters empties
2. Detects presence of `script-src` or `script-src-elem` (case-insensitive)
3. Strips `report-uri` and `report-to` directives entirely
4. For `script-src`, `script-src-elem`, and `default-src` (only when no explicit `script-src`/`script-src-elem` exists): removes `'sha256-...'`, `'sha384-...'`, `'sha512-...'`, `'nonce-...'`, and `'strict-dynamic'` values
5. Omits a directive if all its values were stripped
6. Returns `None` when no directives remain (caller should drop the header)

---

## Change 4: build_response_headers function

**File**: `src/browser/instrumentation.rs` (lines 355-404)

**Before**: No equivalent. Header construction was a single `.response_header()` call.

**After**: Adds `fn build_response_headers(response_headers, resource_type, source_id) -> Vec<HeaderEntry>` (private, ~30 lines) which:
1. Iterates upstream headers, filtering out those in `STRIPPED_RESPONSE_HEADERS` (case-insensitive)
2. For CSP headers (`content-security-policy` and `content-security-policy-report-only`):
   - `Script` -> drops entirely
   - `Document` -> sanitizes via `sanitize_csp()`, drops if `None`
   - `_` wildcard -> forwards unchanged
3. Appends synthetic `etag` from `source_id`
4. Collects into `Vec<fetch::HeaderEntry>`

---

## Change 5: Unit test module

**File**: `src/browser/instrumentation.rs` (lines 406-756)

**Before**: No unit tests in this file.

**After**: Adds `#[cfg(test)] mod tests` with:
- 2 helper functions: `hdr(name, value) -> HeaderEntry` and `sid(n) -> SourceId`
- 19 tests for `sanitize_csp`: SHA-256/384/512 removal, nonce removal, mixed directives, no-script-src passthrough, empty result, multiple hashes, `script-src-elem`, `default-src` fallback, `default-src` untouched when script-src present, `strict-dynamic` orphan removal, `report-uri`/`report-to` stripping
- 9 tests for `build_response_headers`: stripped headers absent, `content-type` preserved, CSP dropped for Script, CSP sanitized for Document, report-only CSP for Script/Document, synthetic etag, `None` input, non-stripped headers pass through

---

## Change 6: run_browser_test split into two functions

**File**: `tests/integration_tests.rs`

**Before**: Single function `run_browser_test(name, expect, timeout, spec)` that created `Router::new().fallback_service(ServeDir::new("./tests"))` internally.

**After**: Splits into two:
- `run_browser_test_with_router(name, expect, timeout, spec, app)` — contains the full implementation, takes a caller-supplied `Router`. Brief doc comment `/// See [`run_browser_test`].`
- `run_browser_test(name, expect, timeout, spec)` — creates the default router and delegates. Retains the full doc comment.

Doc comment changes: typo "facitiliate" corrected to "facilitate". URL pattern updated from `http://localhost:{P}/tests/{name}.` to `http://localhost:{P}/{name}`.

---

## Change 7: make_csp_router helper

**File**: `tests/integration_tests.rs`

**Before**: No equivalent.

**After**: Adds `fn make_csp_router(csp: &'static str) -> Router` which builds a `ServeDir`-backed router with axum middleware that injects a `content-security-policy` header on every response. Used by two CSP integration tests.

---

## Change 8: Four new integration tests

**File**: `tests/integration_tests.rs`

**Before**: 11 integration tests.

**After**: Adds 4 tests (total 15), all `Expect::Success` with `Duration::from_secs(30)`:

| Test | Router | Fixture | Verifies |
|------|--------|---------|----------|
| `test_external_module_script` | default | `external-module-script/` | `<script type="module" src="...">` loads after instrumentation (content-type preserved) |
| `test_compressed_script` | `CompressionLayer::new()` | `compressed-script/` | Gzip-compressed scripts work (stale content-encoding stripped) |
| `test_csp_script` | `make_csp_router(hash + img-src)` | `csp-script/` | Script loads (CSP dropped for Script) AND img-src 'none' enforced on Document |
| `test_csp_document_directives_preserved` | `make_csp_router(mixed CSP)` | `csp-document/` | Script hashes stripped, non-script directives preserved on Document |

Each test has a custom spec using `extract` + `eventually(...).within(10, "seconds")` + `clicks` export.

---

## Change 9: New test fixtures

**Before**: No `external-module-script/`, `compressed-script/`, `csp-script/`, `csp-document/`, or `shared/` directories.

**After**:

| Path | Content |
|------|---------|
| `tests/shared/script.js` | `document.getElementById("result").textContent = "LOADED";` |
| `tests/external-module-script/index.html` | Loads `/shared/script.js` as `<script type="module">` |
| `tests/compressed-script/index.html` | Loads `/shared/script.js` as `<script>` (classic) |
| `tests/csp-script/index.html` | Loads `/shared/script.js` as `<script>`, adds inline script for CSP violation detection via `securitypolicyviolation` event + test image |
| `tests/csp-document/index.html` | Inline script only: CSP violation detection via `securitypolicyviolation` event + test image |

All fixtures follow existing structure: `<html>`, `<head>`, `<title>`, `<body>`, `<h1 id="result">WAITING</h1>`.

---

## Change 10: Dev-dependency feature addition

**File**: `Cargo.toml`

**Before**: `tower-http = { version = "0.6.8", features = ["fs"] }`

**After**: `tower-http = { version = "0.6.8", features = ["fs", "compression-gzip"] }`

Adds `compression-gzip` feature to the `tower-http` dev-dependency. Required for `CompressionLayer` in `test_compressed_script`.

---

## Change 11: Stale debug log removal

**File**: `tests/integration_tests.rs`

**Before**: `log::info!("just changing for CI");` line in `test_browser_lifecycle` before `browser.terminate()`.

**After**: Line removed. It was a CI cache-bust artifact with no diagnostic value.

---

## Change 12: New imports in integration tests

**File**: `tests/integration_tests.rs`

**Before**:
```rust
use axum::Router;
use tower_http::services::ServeDir;
```

**After**:
```rust
use axum::{
    Router, extract::Request, http::HeaderValue, middleware, response::Response,
};
use tower_http::{compression::CompressionLayer, services::ServeDir};
```

Adds `Request`, `HeaderValue`, `middleware`, `Response` from axum and `CompressionLayer` from tower-http. All used by the new test helpers.
