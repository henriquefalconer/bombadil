# Comparison: develop vs antithesishq/main

This document describes only the code that was altered or added in the `develop` branch relative to `antithesishq/main`. Nothing from the original codebase is described here.

---

## Change 1: Response Header Forwarding (`src/browser/instrumentation.rs`)

### What Was Changed

The original code in `instrument_js_coverage` used `FulfillRequestParams` with a single `.response_header()` call that set only a synthetic `etag`. This replaced **all** upstream response headers with just that one header. There was a `// TODO: forward headers` comment acknowledging this was incomplete.

The new code replaces this with:

1. **`STRIPPED_RESPONSE_HEADERS` constant** (lines 18–44): A denylist of 5 header names (`etag`, `content-length`, `content-encoding`, `transfer-encoding`, `digest`) that are stripped because instrumentation invalidates them. Each entry has an inline comment explaining why.

2. **`build_response_headers` function** (lines 367–404): Takes the original response headers, resource type, and source ID. Filters out stripped headers, applies resource-type-aware CSP handling, and appends a synthetic `etag`. Called from the `FulfillRequestParams` builder via `.response_headers(...)` instead of the old `.response_header(...)`.

3. **`sanitize_csp` function** (lines 279–353): Parses CSP header values and strips only instrumentation-sensitive values (`'sha256-…'`, `'sha384-…'`, `'sha512-…'`, `'nonce-…'`, `'strict-dynamic'`) from `script-src`, `script-src-elem`, and (when no explicit script-src exists) `default-src`. Also strips `report-uri` and `report-to` directives. Returns `None` when all directives are stripped.

4. **Resource-type-aware CSP handling in `build_response_headers`**: For `Script` resources, CSP headers are dropped entirely. For `Document` resources, CSP headers are sanitized via `sanitize_csp`. For other resource types (currently unreachable), CSP headers pass through unchanged.

5. **`resource_type` capture** (line 187): `let resource_type = event.resource_type.clone();` added before the builder to avoid borrow conflicts.

### Original Code Replaced

```rust
.response_header(fetch::HeaderEntry {
    name: "etag".to_string(),
    value: format!("{}", source_id.0),
})
// TODO: forward headers
```

### New Code

```rust
let resource_type = event.resource_type.clone();
// ...
.response_headers(build_response_headers(
    &event.response_headers,
    &resource_type,
    source_id,
))
```

---

## Change 2: Unit Tests for Header/CSP Logic (`src/browser/instrumentation.rs`)

### What Was Added

A `#[cfg(test)] mod tests` block (lines 406–756) containing:

- **20 `sanitize_csp_*` tests**: Cover SHA-256/384/512 hash removal, nonce removal, mixed directives, empty results, `script-src-elem`, `default-src` fallback when no `script-src` present, `default-src` untouched when `script-src` present, `strict-dynamic` removal, `report-uri`/`report-to` stripping.

- **9 `build_headers_*` tests**: Cover stripped header removal, `content-type` preservation, CSP dropped for Script resources, CSP sanitized for Document resources, report-only CSP handling, synthetic etag appended, `None` headers yield only etag, non-stripped headers pass through.

- **2 helper functions**: `hdr(name, value)` and `sid(n)` for test convenience.

---

## Change 3: Integration Test Refactoring (`tests/integration_tests.rs`)

### What Was Changed

1. **`run_browser_test` split into two functions**: The original `run_browser_test` created the `Router` internally. Now `run_browser_test_with_router` accepts an `app: Router` parameter with the full implementation, and `run_browser_test` is a thin wrapper that creates the default `ServeDir` router and delegates.

2. **Doc comment moved**: Full doc comment on the wrapper (`run_browser_test`). One-liner on the inner function referencing the wrapper.

3. **Doc comment fixed**: URL pattern corrected from `http://localhost:{P}/tests/{name}.` to `http://localhost:{P}/{name}`. Typo `facitiliate` corrected to `facilitate`.

4. **Removed CI cache-buster log**: `log::info!("just changing for CI");` removed from `test_browser_lifecycle`.

5. **Import additions**: `axum::{extract::Request, http::HeaderValue, middleware, response::Response}` and `tower_http::compression::CompressionLayer`.

---

## Change 4: Four New Integration Tests (`tests/integration_tests.rs`)

All new tests use `Expect::Success` with `Duration::from_secs(30)`:

| Test | Router | Fixture | Verifies |
|------|--------|---------|----------|
| `test_external_module_script` | default | `external-module-script/` | `<script type="module" src="...">` loads after instrumentation (content-type preserved) |
| `test_compressed_script` | `CompressionLayer` | `compressed-script/` | Compressed script works after instrumentation (stale content-encoding stripped). Uses `type="module"` as content-type regression guard. |
| `test_csp_script` | `make_csp_router(hash + img-src)` | `csp-script/` | Script loads despite hash-based CSP (CSP dropped for Script) AND `img-src 'none'` enforced on Document (CSP sanitized, non-script directives preserved) |
| `test_csp_document_directives_preserved` | `make_csp_router(mixed CSP)` | `csp-document/` | Document CSP sanitized: script hashes stripped, `img-src 'self'` preserved and enforced |

### New Helper

`make_csp_router(csp: &'static str) -> Router`: Creates a router with axum middleware that injects a `content-security-policy` header on every response.

---

## Change 5: Test Fixture Files

| Path | Content |
|------|---------|
| `tests/shared/script.js` | `document.getElementById("result").textContent = "LOADED";` |
| `tests/external-module-script/index.html` | Loads `/shared/script.js` as `<script type="module">` |
| `tests/compressed-script/index.html` | Same structure, `<title>Compressed Script</title>` |
| `tests/csp-script/index.html` | Loads `/shared/script.js` as classic `<script>`, plus inline CSP violation detection |
| `tests/csp-document/index.html` | Inline-only CSP violation detection |

All fixtures follow existing structure: `<html>`, `<head>`, `<title>`, `<body>`, `<h1 id="result">WAITING</h1>`.

---

## Change 6: Build Configuration

### `Cargo.toml`

`tower-http` dev-dependency gained `compression-gzip` feature (required for `CompressionLayer` in `test_compressed_script`).

### `Cargo.lock`

Added transitive dev-dependencies: `async-compression`, `compression-codecs`, `compression-core`.

### `.cargo/config.toml` (new file)

Linker configuration for `aarch64-unknown-linux-gnu` (sandbox/CI build environment, not a behavioral change).

### `AGENTS.md`

Added "Sandboxed Build Environment" section documenting sandbox-specific build requirements.

---

## Change 7: Non-Code Files

Development artifacts added in `develop` that are not part of the runtime behavior:

- `ISSUE.md`, `IMPLEMENTATION_PLAN.md`: Bug description and progress tracking
- `PROMPT_build.md`, `PROMPT_plan.md`, `PROMPT_security.md`: AI assistant prompts
- `SYNTHESIS.md`: Development analysis
- `SECURITY.md`, `SECURITY_ANALYSIS.md`, `PATTERNS.md`: Analysis documents
- `loop.sh`: Shell script for iterative development
