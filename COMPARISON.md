# Code Comparison: develop vs antithesishq/main

This document describes **only the code that was changed** on `develop` relative to `antithesishq/main`. Documentation-only files, prompt files, build scripts (`loop.sh`), and issue descriptions are excluded — only behavioral code and test changes are listed.

---

## Change 1: STRIPPED_RESPONSE_HEADERS constant

**File**: `src/browser/instrumentation.rs`

**Before (antithesishq/main)**: No constant. All upstream response headers were silently dropped — the `FulfillRequestParams` builder used `.response_header()` (singular) with only a synthetic `etag`, replacing the entire header set. A `// TODO: forward headers` comment acknowledged this gap.

**After (develop)**: Adds a module-level `const STRIPPED_RESPONSE_HEADERS: &[&str]` containing five lowercase header names: `"etag"`, `"content-length"`, `"content-encoding"`, `"transfer-encoding"`, `"digest"`. Each entry has an inline comment explaining why it must be removed. A doc comment above the constant explains the relationship to CSP handling (which is handled separately via resource-type-aware logic rather than the denylist).

---

## Change 2: FulfillRequestParams builder rewrite

**File**: `src/browser/instrumentation.rs`

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

Changed from `.response_header()` (singular — one header replaces all) to `.response_headers()` (plural — full header list from `build_response_headers`). Adds `let resource_type = event.resource_type.clone()` before the builder to avoid a borrow conflict when both `event.response_headers` and `event.resource_type` are used in the same expression.

---

## Change 3: sanitize_csp function

**File**: `src/browser/instrumentation.rs`

**Before**: No CSP handling existed.

**After**: Adds `fn sanitize_csp(csp_value: &str) -> Option<String>` (private, ~60 lines of logic) which:
1. Splits the header value on `;` into directives, trims whitespace, filters empty entries.
2. Detects presence of any `script-src` or `script-src-elem` directive (case-insensitive) to determine `default-src` fallback behavior.
3. Strips `report-uri` and `report-to` directives entirely.
4. For `script-src`, `script-src-elem`, and `default-src` (only when no explicit `script-src`/`script-src-elem` exists): removes `'sha256-...'`, `'sha384-...'`, `'sha512-...'`, `'nonce-...'`, and `'strict-dynamic'` token values.
5. Omits a directive entirely if all its values were stripped.
6. Returns `None` when no directives remain (caller should drop the entire header).

---

## Change 4: build_response_headers function

**File**: `src/browser/instrumentation.rs`

**Before**: No equivalent function. Header construction was a single `.response_header()` call with one hardcoded entry.

**After**: Adds `fn build_response_headers(response_headers, resource_type, source_id) -> Vec<HeaderEntry>` (private, ~30 lines of logic) which:
1. Iterates upstream headers, filtering out those in `STRIPPED_RESPONSE_HEADERS` (case-insensitive via `eq_ignore_ascii_case`).
2. Applies resource-type-aware CSP handling:
   - `Script` → drops CSP headers entirely (both enforcing and report-only).
   - `Document` → sanitizes via `sanitize_csp()`, drops if sanitization returns `None`.
   - `_` (other) → forwards CSP unchanged.
3. Appends a synthetic `etag` header derived from `source_id`.
4. Collects into `Vec<fetch::HeaderEntry>`.

---

## Change 5: Unit test module

**File**: `src/browser/instrumentation.rs`

**Before**: No unit tests existed in this file.

**After**: Adds `#[cfg(test)] mod tests` containing:
- 2 test helper functions: `hdr(name, value) -> HeaderEntry` and `sid(n) -> SourceId`.
- 19 tests for `sanitize_csp`: SHA-256/384/512 removal, nonce removal, mixed directives, no-script-src passthrough, empty result, multiple hashes with safe value, only-hash directive removed while others kept, `script-src-elem`, `default-src` hash stripping when no script-src present, `default-src` untouched when script-src IS present, `default-src` only-hashes omitted, `strict-dynamic` orphan removal (with nonce, with hash, keeping other values), `report-uri` stripping, `report-to` stripping, both report directives stripped.
- 9 tests for `build_response_headers`: all stripped headers absent, `content-type` preserved, CSP dropped for Script, CSP sanitized for Document, report-only CSP dropped for Script, report-only CSP sanitized for Document, synthetic etag always present, `None` input yields only synthetic etag, non-stripped non-CSP headers pass through.

---

## Change 6: run_browser_test split into two functions

**File**: `tests/integration_tests.rs`

**Before**: Single function `run_browser_test(name, expect, timeout, spec)` that created `Router::new().fallback_service(ServeDir::new("./tests"))` internally.

**After**: Splits into:
- `run_browser_test_with_router(name, expect, timeout, spec, app)` — contains the full implementation, accepts a caller-supplied `Router`. Brief doc comment: `/// See [`run_browser_test`].`
- `run_browser_test(name, expect, timeout, spec)` — creates the default router and delegates. Retains the full doc comment.

Doc comment changes: typo "facitiliate" corrected to "facilitate". URL pattern updated from `http://localhost:{P}/tests/{name}.` to `http://localhost:{P}/{name}`.

---

## Change 7: make_csp_router helper

**File**: `tests/integration_tests.rs`

**Before**: No equivalent.

**After**: Adds `fn make_csp_router(csp: &'static str) -> Router` which builds a `ServeDir`-backed router with axum middleware that injects a `content-security-policy` header on every response. Used by `test_csp_script` and `test_csp_document_directives_preserved`.

---

## Change 8: Four new integration tests

**File**: `tests/integration_tests.rs`

**Before**: 11 integration tests.

**After**: 15 integration tests (4 added). All new tests use `Expect::Success` with `Duration::from_secs(30)`:

| Test | Router | Fixture | Verifies |
|------|--------|---------|----------|
| `test_external_module_script` | default | `external-module-script/` | `<script type="module" src="...">` loads after instrumentation (content-type preserved) |
| `test_compressed_script` | `CompressionLayer::new()` | `compressed-script/` | Gzip-compressed script works after instrumentation (stale content-encoding stripped). Uses `type="module"` to also serve as a content-type regression test. |
| `test_csp_script` | `make_csp_router(hash + img-src)` | `csp-script/` | Script loads despite hash-based CSP (CSP dropped for Script) AND `img-src 'none'` enforced on Document (CSP sanitized, non-script directives preserved) |
| `test_csp_document_directives_preserved` | `make_csp_router(mixed CSP)` | `csp-document/` | Document CSP sanitized: script hashes stripped, `img-src 'self'` preserved and enforced |

Each test provides a custom spec using `extract` + `eventually(...).within(10, "seconds")` + `clicks` action export.

---

## Change 9: New test fixtures

**Before**: No `external-module-script/`, `compressed-script/`, `csp-script/`, `csp-document/`, or `shared/` directories existed.

**After**:

| Path | Content |
|------|---------|
| `tests/shared/script.js` | `document.getElementById("result").textContent = "LOADED";` |
| `tests/external-module-script/index.html` | Loads `/shared/script.js` as `<script type="module">` |
| `tests/compressed-script/index.html` | Loads `/shared/script.js` as `<script type="module">` (module type makes this a content-type regression test) |
| `tests/csp-script/index.html` | Loads `/shared/script.js` as classic `<script>`, plus inline script for CSP violation detection via `securitypolicyviolation` event + external image trigger |
| `tests/csp-document/index.html` | Inline script only: CSP violation detection via `securitypolicyviolation` event + external image trigger |

All fixtures follow the existing structure: `<html>`, `<head>`, `<title>`, `<body>`, `<h1 id="result">WAITING</h1>`.

---

## Change 10: Dev-dependency feature addition

**File**: `Cargo.toml`

**Before**: `tower-http = { version = "0.6.8", features = ["fs"] }`

**After**: `tower-http = { version = "0.6.8", features = ["fs", "compression-gzip"] }`

Adds `compression-gzip` feature to the `tower-http` dev-dependency. Required for `CompressionLayer` used in `test_compressed_script`. Pulls in `async-compression`, `compression-codecs`, `compression-core`, and `flate2` as transitive dev-dependencies.

---

## Change 11: Stale debug log removal

**File**: `tests/integration_tests.rs`

**Before**: `log::info!("just changing for CI");` in `test_browser_lifecycle` before `browser.terminate()`.

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

Adds `Request`, `HeaderValue`, `middleware`, `Response` from axum and `CompressionLayer` from tower-http. All used by the new test helpers and tests.

---

## Change 13: .cargo/config.toml addition

**File**: `.cargo/config.toml`

**Before**: File did not exist.

**After**: Adds linker configuration for `aarch64-unknown-linux-gnu` target specifying `clang` as the linker with `-fuse-ld=lld`. This is a build environment configuration for the sandbox, not a behavioral change.

---

## Change 14: AGENTS.md sandbox build notes

**File**: `AGENTS.md`

**Before**: Ended after the Testing section.

**After**: Adds a "Sandboxed Build Environment" section with notes on `CARGO_TARGET_DIR`, `RUST_MIN_STACK`, `.cargo/config.toml`, `esbuild`, and Chromium path. These are documentation for sandbox CI environments.
