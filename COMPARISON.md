# Code Changes: develop vs antithesishq/main

Common ancestor: `0e2913d` ("link to manual from README (#54)").
`antithesishq/main` has not moved since the ancestor. All changes below are additions or modifications made on the `develop` branch.

---

## `src/browser/instrumentation.rs`

### Change 1: New constant `STRIPPED_RESPONSE_HEADERS`

Inserted before `instrument_js_coverage`. Defines a list of response header names (lowercase) that must be removed when fulfilling intercepted requests after instrumentation: `etag`, `content-length`, `content-encoding`, `transfer-encoding`, `digest`. Each entry has a doc comment explaining why it must be stripped. A longer doc comment on the constant explains why `content-security-policy` and `content-security-policy-report-only` are NOT in the list (they receive resource-type-aware handling instead).

This is entirely new code — `antithesishq/main` had no header strip list.

### Change 2: Replaced `FulfillRequestParams` header construction

**What was in antithesishq/main:**
```rust
.response_header(fetch::HeaderEntry {
    name: "etag".to_string(),
    value: format!("{}", source_id.0),
})
// TODO: forward headers
```
This used `.response_header()` (singular) to set only a synthetic ETag. All original response headers were silently dropped. The `// TODO: forward headers` comment acknowledged this was incomplete.

**What replaced it:**

A new line `let resource_type = event.resource_type.clone();` was added before the `page.execute(...)` block.

The `.response_header()` call was replaced with `.response_headers()` (plural) taking an iterator chain that:
1. Iterates the original response headers from `event.response_headers`
2. Filters out headers in `STRIPPED_RESPONSE_HEADERS` (case-insensitive match)
3. Applies resource-type-aware CSP handling via `flat_map`:
   - For `content-security-policy` and `content-security-policy-report-only`:
     - Script resources: drops the header entirely (returns `None`)
     - All other resources: passes through `sanitize_csp()`, which strips hash/nonce values from `script-src`/`script-src-elem` directives
4. Chains a synthetic ETag entry at the end (same as before, but now alongside forwarded headers)

### Change 3: New function `sanitize_csp`

Appended after the existing `source_id` function. A private function that parses a CSP header value, identifies `script-src` and `script-src-elem` directives, removes `'sha256-…'`, `'sha384-…'`, `'sha512-…'`, and `'nonce-…'` values from them, preserves all other directives unchanged, and returns `None` if all directives were stripped (so the caller can omit the header entirely).

This is entirely new code — `antithesishq/main` had no CSP handling.

### Change 4: New unit test module

10 unit tests for `sanitize_csp` appended at end of file in a `#[cfg(test)] mod tests` block. Tests cover: SHA-256/384/512 removal, nonce removal, mixed directives, no-script-src passthrough, empty result, multiple hashes with safe value, only-hash directive with other directives, `script-src-elem`.

No unit tests existed in this file in `antithesishq/main`.

---

## `tests/integration_tests.rs`

### Change 5: Import additions

```rust
// Removed:
use axum::Router;
use tower_http::services::ServeDir;

// Added:
use axum::{
    Router, extract::Request, http::HeaderValue, middleware, response::Response,
};
use tower_http::{compression::CompressionLayer, services::ServeDir};
```

New imports support the middleware-based CSP injection and gzip compression used by the new tests.

### Change 6: `run_browser_test` split into wrapper + implementation

The original `run_browser_test` body was moved into a new function `run_browser_test_with_router` that accepts an additional `app: Router` parameter. The original `run_browser_test` became a thin wrapper that constructs the default `ServeDir`-based router and delegates.

The `setup()` call was moved from `run_browser_test_with_router` to the start (it was already idempotent via `Once`). The router construction `let app = Router::new().fallback_service(ServeDir::new("./tests"))` moved from inside the function body to the wrapper.

Doc comment corrections: "facitiliate" → "facilitate", `http://localhost:{P}/tests/{name}` → `http://localhost:{P}/{name}`. The wrapper (`run_browser_test`) retains the full doc comment; the implementation (`run_browser_test_with_router`) has a brief `/// See [`run_browser_test`].` reference.

### Change 7: Four new integration tests

**`test_external_module_script`**: Default router, 20s timeout. HTML fixture uses `<script type="module" src="...">`. Spec checks `#result` text becomes `"LOADED"` within 10s. Verifies that ES module scripts are intercepted and instrumented.

**`test_compressed_script`**: Custom router with `CompressionLayer::new()`. 20s timeout. Verifies that gzip-compressed script responses are correctly decompressed by CDP and instrumented.

**`test_csp_script`**: Custom router with middleware injecting `content-security-policy: script-src 'sha256-sRoPO3cqhmVEQTMEK66eATz8J/LJdrvqrNVuMKzGgSM='`. 20s timeout. Verifies that CSP hash-only headers are stripped for Script resources so the instrumented script still loads.

**`test_csp_document_directives_preserved`**: Custom router with middleware injecting `content-security-policy: script-src 'unsafe-inline' 'self' 'sha256-AAAA...'; img-src 'self'`. 20s timeout. Verifies that non-script CSP directives (`img-src`) are preserved in Document responses. The page triggers a `securitypolicyviolation` event by loading a cross-origin image, confirming `img-src 'self'` is enforced.

### Change 8: Four new HTML fixture directories

`tests/external-module-script/` — `index.html` + `module.js`
`tests/compressed-script/` — `index.html` + `script.js`
`tests/csp-script/` — `index.html` + `script.js`
`tests/csp-document/` — `index.html`

All follow existing fixture structure: `<html>`, `<head>`, `<title>`, `<body>`, `<h1 id="result">WAITING</h1>`. Script fixtures set `#result` text to `"LOADED"`. CSP document fixture listens for `securitypolicyviolation` and sets `#result` to `"CSP_ACTIVE"`.

---

## `Cargo.toml`

### Change 9: Added `compression-gzip` feature

```toml
# antithesishq/main:
tower-http = { version = "0.6.8", features = ["fs"] }

# develop:
tower-http = { version = "0.6.8", features = ["fs", "compression-gzip"] }
```

This is a dev-dependency change (`[dev-dependencies]` section) — it only affects tests, not the production binary. Enables the `CompressionLayer` used by `test_compressed_script`.

### Change 10: `Cargo.lock` transitive additions

Three new crate entries: `async-compression`, `compression-codecs`, `compression-core`. All are transitive dependencies of `tower-http` with the `compression-gzip` feature enabled.

---

## `.cargo/config.toml`

### Change 11: New build configuration file

```toml
[target.aarch64-unknown-linux-gnu]
linker = "clang"
rustflags = ["-C", "link-arg=-fuse-ld=lld"]
```

Build environment configuration for aarch64 Linux targets. Uses clang as linker with lld for faster linking. This is a development environment concern, not a code change.

---

## Non-code files added (not present in antithesishq/main)

These files are workflow artifacts, not shipped code:

- `AGENTS.md` — Sandbox build environment section appended
- `PATTERNS.md` — Project conventions document
- `SECURITY.md` — Open issues in response header handling
- `SECURITY_ANALYSIS.md` — Detailed analysis of problems and assumptions
- `COMPARISON.md` — This file
- `IMPLEMENTATION_PLAN.md` — Implementation tracking
- `ISSUE.md` — Issue description
- `SYNTHESIS.md` — Synthesis notes
- `PROMPT_build.md`, `PROMPT_plan.md`, `PROMPT_security.md` — Prompt templates
- `loop.sh` — Build/test automation script
