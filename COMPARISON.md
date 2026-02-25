# Code Changes: develop vs antithesishq/main

Only altered code is listed. Unchanged code, documentation-only files (AGENTS.md, ISSUE.md, SYNTHESIS.md, prompts, plans), build configuration (.cargo/config.toml), and Cargo.lock are excluded.

---

## Change 1: `STRIPPED_RESPONSE_HEADERS` constant

**File:** `src/browser/instrumentation.rs`

**antithesishq/main:** No constant. All upstream response headers were silently dropped — the `FulfillRequestParams` builder used `.response_header()` (singular) with only a synthetic `etag`, replacing the entire header set due to CDP replacement semantics. A `// TODO: forward headers` comment acknowledged this gap.

**develop:** Adds a module-level `const STRIPPED_RESPONSE_HEADERS: &[&str]` containing five lowercase header names: `"etag"`, `"content-length"`, `"content-encoding"`, `"transfer-encoding"`, `"digest"`. Each entry has an inline `//` comment explaining why it must be removed. A `///` doc comment block above the constant explains why CSP headers are excluded (they receive resource-type-aware handling instead of blanket stripping).

---

## Change 2: `FulfillRequestParams` builder rewrite

**File:** `src/browser/instrumentation.rs`

**antithesishq/main:**
```rust
.response_header(fetch::HeaderEntry {
    name: "etag".to_string(),
    value: format!("{}", source_id.0),
})
// TODO: forward headers
```

**develop:**
```rust
let resource_type = event.resource_type.clone();

.response_headers(build_response_headers(
    &event.response_headers,
    &resource_type,
    source_id,
))
```

Changed from `.response_header()` (singular — one header replaces all) to `.response_headers()` (plural — full header list). Adds `let resource_type = event.resource_type.clone()` before the builder to avoid a borrow conflict when iterating `event.response_headers` inside `build_response_headers`.

---

## Change 3: `sanitize_csp` function

**File:** `src/browser/instrumentation.rs`

**antithesishq/main:** No CSP handling.

**develop:** Adds `fn sanitize_csp(csp_value: &str) -> Option<String>` (private, ~60 lines) which:
1. Splits on `;` into directives, trims whitespace, filters empties
2. Detects presence of `script-src` or `script-src-elem` (case-insensitive)
3. Strips `report-uri` and `report-to` directives entirely
4. For `script-src`, `script-src-elem`, and `default-src` (only when no explicit `script-src`/`script-src-elem` exists): removes `'sha256-…'`, `'sha384-…'`, `'sha512-…'`, `'nonce-…'`, and `'strict-dynamic'` values
5. Omits a directive if all its values were stripped
6. Returns `None` when no directives remain (caller should drop the header)

---

## Change 4: `build_response_headers` function

**File:** `src/browser/instrumentation.rs`

**antithesishq/main:** No equivalent. Header construction was a single `.response_header()` call.

**develop:** Adds `fn build_response_headers(response_headers: &Option<Vec<fetch::HeaderEntry>>, resource_type: &network::ResourceType, source_id: SourceId) -> Vec<fetch::HeaderEntry>` (private, ~30 lines) which:
1. `.iter().flatten()` over upstream headers
2. `.filter()` removes headers in `STRIPPED_RESPONSE_HEADERS` (case-insensitive)
3. `.flat_map()` applies resource-type-aware CSP:
   - `Script` → drop CSP entirely (`None`)
   - `Document` → sanitize via `sanitize_csp()`, drop if `None`
   - `_ =>` wildcard → forward unchanged
4. `.chain()` appends synthetic `etag` from `source_id`
5. `.collect()` into `Vec<fetch::HeaderEntry>`

Handles both `content-security-policy` and `content-security-policy-report-only`.

---

## Change 5: Unit test module

**File:** `src/browser/instrumentation.rs`

**antithesishq/main:** No unit tests in this file.

**develop:** Adds `#[cfg(test)] mod tests` with:
- 2 helper functions: `hdr(name, value) -> HeaderEntry` and `sid(n) -> SourceId`
- 19 tests for `sanitize_csp`: SHA-256/384/512 removal, nonce removal, mixed directives, no-script-src passthrough, empty result, multiple hashes, `script-src-elem`, `default-src` fallback when no script-src, `default-src` untouched when script-src present, `strict-dynamic` orphan removal (with nonce, with hash, keeping other values), `report-uri` stripping, `report-to` stripping, both report directives
- 7 tests for `build_response_headers`: stripped headers absent, `content-type` preserved, CSP dropped for Script, CSP sanitized for Document, report-only CSP for Script/Document, synthetic etag always present, `None` input yields only etag, non-stripped headers pass through

---

## Change 6: `run_browser_test` split

**File:** `tests/integration_tests.rs`

**antithesishq/main:** Single function `run_browser_test(name, expect, timeout, spec)` that created `Router::new().fallback_service(ServeDir::new("./tests"))` internally.

**develop:** Splits into two:
- `run_browser_test_with_router(name, expect, timeout, spec, app)` — contains the full implementation, takes a caller-supplied `Router`. Has brief doc comment `/// See [`run_browser_test`].`
- `run_browser_test(name, expect, timeout, spec)` — creates the default router and delegates. Retains the full doc comment.

Doc comment changes: typo "facitiliate" → "facilitate"; URL pattern updated from `http://localhost:{P}/tests/{name}.` to `http://localhost:{P}/{name}`.

---

## Change 7: `make_csp_router` helper

**File:** `tests/integration_tests.rs`

**antithesishq/main:** No equivalent.

**develop:** Adds `fn make_csp_router(csp: &'static str) -> Router` which builds a `ServeDir`-backed router with an `axum::middleware::from_fn` layer that injects a `content-security-policy` header on every response. Used by two CSP integration tests.

---

## Change 8: Four new integration tests

**File:** `tests/integration_tests.rs`

**antithesishq/main:** 11 tests.

**develop:** Adds 4 tests (total 15), all with `Expect::Success` and `Duration::from_secs(30)`:

| Test | Router | Fixture | Verifies |
|------|--------|---------|----------|
| `test_external_module_script` | default | `external-module-script/` | `<script type="module" src="...">` loads after instrumentation (`content-type` preserved) |
| `test_compressed_script` | `CompressionLayer::new()` | `compressed-script/` | Gzip-compressed scripts work (stale `content-encoding` stripped) |
| `test_csp_script` | `make_csp_router(hash + img-src)` | `csp-script/` | Script loads (CSP dropped for Script) AND `img-src 'none'` enforced on Document |
| `test_csp_document_directives_preserved` | `make_csp_router(mixed CSP)` | `csp-document/` | Script hashes stripped, non-script directives preserved on Document |

Each test has a custom spec using `extract` + `eventually(...).within(10, "seconds")` + `clicks` export.

---

## Change 9: New test fixtures

**antithesishq/main:** No `external-module-script/`, `compressed-script/`, `csp-script/`, `csp-document/`, or `shared/` directories.

**develop:**

| Path | Content |
|------|---------|
| `tests/shared/script.js` | `document.getElementById("result").textContent = "LOADED";` |
| `tests/external-module-script/index.html` | Loads `/shared/script.js` as `<script type="module">` |
| `tests/compressed-script/index.html` | Loads `/shared/script.js` as `<script>` (classic, not module) |
| `tests/csp-script/index.html` | Loads `/shared/script.js` as `<script>` (classic), adds inline script for CSP violation detection via `securitypolicyviolation` event + test image |
| `tests/csp-document/index.html` | Inline script only: CSP violation detection via `securitypolicyviolation` event + test image |

All fixtures follow existing structure: `<html>`, `<head>`, `<title>`, `<body>`, `<h1 id="result">WAITING</h1>`.

---

## Change 10: Dev-dependency feature addition

**File:** `Cargo.toml`

**antithesishq/main:**
```toml
tower-http = { version = "0.6.8", features = ["fs"] }
```

**develop:**
```toml
tower-http = { version = "0.6.8", features = ["fs", "compression-gzip"] }
```

Adds `compression-gzip` feature to the `tower-http` dev-dependency. Required for `CompressionLayer` in `test_compressed_script`.

---

## Change 11: Stale debug log removal

**File:** `tests/integration_tests.rs`

**antithesishq/main:**
```rust
    log::info!("just changing for CI");
    browser.terminate().await.unwrap();
```

**develop:**
```rust
    browser.terminate().await.unwrap();
```

Removes `log::info!("just changing for CI");` from `test_browser_lifecycle`. This line was a CI cache-bust artifact with no diagnostic value.

---

## Change 12: New imports in integration tests

**File:** `tests/integration_tests.rs`

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

Adds `Request`, `HeaderValue`, `middleware`, `Response` from axum and `CompressionLayer` from tower-http. All are used by the new test helpers (`make_csp_router`, `test_compressed_script`).
