# Code Changes: develop vs antithesishq/main

Common ancestor: `0e2913d` ("link to manual from README (#54)").
`antithesishq/main` has not moved since the ancestor. All changes below are additions or modifications made on the `develop` branch. Only altered code is described — unchanged code is not reproduced.

---

## `src/browser/instrumentation.rs`

### Change 1: New constant `STRIPPED_RESPONSE_HEADERS`

Inserted before `instrument_js_coverage`. Defines a `&[&str]` of five lowercase header names stripped from intercepted responses: `etag`, `content-length`, `content-encoding`, `transfer-encoding`, `digest`. Each entry has a `//` comment explaining why it is stripped. A `///` doc comment above the constant explains why `content-security-policy` and `content-security-policy-report-only` are NOT listed (they receive resource-type-aware handling instead of unconditional stripping).

No header strip list existed in `antithesishq/main`.

### Change 2: Replaced `FulfillRequestParams` header construction

**antithesishq/main (removed):**
```rust
.response_header(fetch::HeaderEntry {
    name: "etag".to_string(),
    value: format!("{}", source_id.0),
})
// TODO: forward headers
```
Used `.response_header()` (singular) to set only a synthetic ETag. All original response headers were silently dropped.

**develop (replacement):**

A new binding `let resource_type = event.resource_type.clone();` captures the resource type before the builder borrows `event`.

`.response_header()` was replaced with `.response_headers(build_response_headers(...))`, delegating to the new `build_response_headers` helper.

### Change 3: New function `sanitize_csp`

Private function appended after the existing `source_id` function. Parses a CSP header value by splitting on `;`, then for `script-src`, `script-src-elem`, and `default-src` (when no explicit `script-src`/`script-src-elem` is present) strips:
- `'sha256-…'`, `'sha384-…'`, `'sha512-…'` values
- `'nonce-…'` values
- `'strict-dynamic'` (meaningless without a trust anchor after hash/nonce removal)

Strips `report-uri` and `report-to` directives entirely. Preserves all other directives unchanged. Returns `None` when every directive was stripped (caller omits the header).

No CSP handling existed in `antithesishq/main`.

### Change 4: New function `build_response_headers`

Private function that encapsulates the header construction pipeline:
1. Iterates `response_headers` via `.iter().flatten()`
2. `.filter()` removes headers whose name case-insensitively matches any entry in `STRIPPED_RESPONSE_HEADERS`
3. `.flat_map()` applies resource-type-aware CSP handling: for `content-security-policy` and `content-security-policy-report-only`, `Script` resources drop the header (`None`), `Document` resources pass through `sanitize_csp()`, and a `_ =>` arm preserves the header unchanged for any other resource type
4. `.chain()` appends a synthetic ETag `fetch::HeaderEntry` (same value derivation as before)

Signature: `fn build_response_headers(response_headers: &Option<Vec<fetch::HeaderEntry>>, resource_type: &network::ResourceType, source_id: SourceId) -> Vec<fetch::HeaderEntry>`.

No equivalent existed in `antithesishq/main`.

### Change 5: New unit test module

`#[cfg(test)] mod tests` block with 26 tests total:
- 19 tests for `sanitize_csp`: SHA-256/384/512 removal, nonce removal, mixed directives, no-script-src passthrough, empty result, multiple hashes with safe value, only-hash directive with other directives, `script-src-elem`, `default-src` fallback stripping (3 cases), `strict-dynamic` removal (3 cases), `report-uri`/`report-to` stripping (3 cases).
- 7 tests for `build_response_headers`: stripped headers removed, content-type preserved, CSP dropped for Script, CSP sanitized for Document, synthetic etag appended, None headers produce only synthetic etag, non-CSP/non-stripped headers pass through.
- A `// ── build_response_headers ──…` section separator comment divides the two groups.

No unit tests existed in this file in `antithesishq/main`.

---

## `tests/integration_tests.rs`

### Change 6: Import additions

**antithesishq/main (removed):**
```rust
use axum::Router;
use tower_http::services::ServeDir;
```

**develop (replacement):**
```rust
use axum::{
    Router, extract::Request, http::HeaderValue, middleware, response::Response,
};
use tower_http::{compression::CompressionLayer, services::ServeDir};
```

New imports support middleware-based CSP injection and gzip compression in new tests.

### Change 7: `run_browser_test` split into wrapper + implementation

The original `run_browser_test` function body was moved into a new `run_browser_test_with_router` that accepts an additional `app: Router` parameter.

`run_browser_test` became a thin wrapper that constructs the default `ServeDir` router and delegates to `run_browser_test_with_router`.

Moved code: `setup()`, semaphore acquisition, server creation, runner creation, event loop, timeout, outcome checking — all moved unchanged into `run_browser_test_with_router`. The only structural change is that `let app = Router::new().fallback_service(ServeDir::new("./tests"))` moved from inside the body to the wrapper.

Doc comment on `run_browser_test_with_router`: `/// See [`run_browser_test`].`
Doc comment corrections on `run_browser_test`: "facitiliate" → "facilitate", `http://localhost:{P}/tests/{name}` → `http://localhost:{P}/{name}`.

### Change 8: Four new integration tests

All four use `Duration::from_secs(20)` as the test timeout — a tier not present in `antithesishq/main` (which uses 3s, 5s, 30s, and 120s).

**`test_external_module_script`**: Default router, `Duration::from_secs(20)`. HTML fixture uses `<script type="module" src="...">`. Spec checks `#result` text becomes `"LOADED"` within 10s.

**`test_compressed_script`**: Custom router with `CompressionLayer::new()`. `Duration::from_secs(20)`. Calls `run_browser_test_with_router`. Spec checks `#result` text becomes `"LOADED"` within 10s.

**`test_csp_script`**: Custom router with `middleware::from_fn` injecting `content-security-policy: script-src 'sha256-sRoPO3cqhmVEQTMEK66eATz8J/LJdrvqrNVuMKzGgSM='` on all responses. `Duration::from_secs(20)`. Multi-line `//` comment block (7 lines) explaining the test rationale. Spec checks `#result` text becomes `"LOADED"` within 10s.

**`test_csp_document_directives_preserved`**: Custom router with `middleware::from_fn` injecting a mixed CSP (`script-src 'unsafe-inline' 'self' 'sha256-AAAA…'; img-src 'self'`) on all responses. `Duration::from_secs(20)`. Multi-line `//` comment block (11 lines) explaining the test rationale, expected outcomes with and without the fix, and the mechanism. HTML fixture listens for `securitypolicyviolation` and sets `#result` to `"CSP_ACTIVE"`. Spec checks `#result` text becomes `"CSP_ACTIVE"` within 10s.

### Change 9: Four new HTML fixture directories

`tests/external-module-script/` — `index.html` + `module.js`
`tests/compressed-script/` — `index.html` + `script.js`
`tests/csp-script/` — `index.html` + `script.js`
`tests/csp-document/` — `index.html`

All follow existing fixture structure: `<html>`, `<head>`, `<title>`, `<body>`, `<h1 id="result">WAITING</h1>`. Script fixtures set `#result` to `"LOADED"`. CSP document fixture listens for `securitypolicyviolation` and sets `#result` to `"CSP_ACTIVE"`.

---

## `Cargo.toml`

### Change 10: Added `compression-gzip` feature to `tower-http`

```toml
# antithesishq/main:
tower-http = { version = "0.6.8", features = ["fs"] }

# develop:
tower-http = { version = "0.6.8", features = ["fs", "compression-gzip"] }
```

Dev-dependency only. Enables `CompressionLayer` for `test_compressed_script`.

### Change 11: `Cargo.lock` transitive additions

Three new crate entries: `async-compression`, `compression-codecs`, `compression-core`. Transitive dependencies of `tower-http/compression-gzip`.

---

## `.cargo/config.toml`

### Change 12: New build configuration file

```toml
[target.aarch64-unknown-linux-gnu]
linker = "clang"
rustflags = ["-C", "link-arg=-fuse-ld=lld"]
```

Development environment configuration for aarch64 Linux. Not a code change — sandbox-specific.

---

## `AGENTS.md`

### Change 13: Appended sandbox build environment section

Added a "Sandboxed Build Environment" section with instructions for building outside Nix: `CARGO_TARGET_DIR`, `RUST_MIN_STACK`, `.cargo/config.toml`, `esbuild`, and Chromium path.

No other content in `AGENTS.md` was modified.

---

## Non-code files added

Workflow artifacts not shipped with the binary:

- `PATTERNS.md` — Project conventions
- `SECURITY.md` — Security issue tracking
- `SECURITY_ANALYSIS.md` — Detailed problem analysis
- `COMPARISON.md` — This file
- `IMPLEMENTATION_PLAN.md` — Implementation tracking
- `ISSUE.md` — Original issue description
- `SYNTHESIS.md` — Architecture notes
- `PROMPT_build.md`, `PROMPT_plan.md`, `PROMPT_security.md` — Agent prompt templates
- `loop.sh` — Build/test automation script
