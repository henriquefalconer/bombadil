# Implementation Plan

## Completed

- Forward response headers in `Fetch.fulfillRequest` (replacing the old single-etag approach)
- Strip stale headers (`content-length`, `content-encoding`, `transfer-encoding`, `content-security-policy`, `content-security-policy-report-only`, `strict-transport-security`) after instrumentation, with rationale comment
- Refactor `run_browser_test` into wrapper + `run_browser_test_with_router`; doc comment on wrapper, one-liner on lower-level function
- Integration tests: `test_external_module_script`, `test_compressed_script`, `test_csp_script` with fixtures
- Fix pattern violations: no `///` on tests, export `clicks`, timeout ≥2× LTL `.within()` bound, typo fix

## TODO (priority order)

1. **Extract stripped-header denylist to a named constant** — The 7-element denylist in `src/browser/instrumentation.rs` is defined inline inside the `.filter()` closure. Per PATTERNS.md ("Values that control behavior should be defined as named `const` or `static` items"), extract it to a module-level `const STRIPPED_RESPONSE_HEADERS: &[&str]` (or similar).

2. **Add per-header documentation** — The current comment groups all 7 headers under a single rationale. Per PATTERNS.md ("document why each header is stripped… every entry and every omission should have a stated reason"), add a brief reason next to each header name in the constant definition. Logical groupings: (a) `etag` — replaced with instrumentation-stable source ID; (b) `content-length` — body size changes; (c) `content-encoding`, `transfer-encoding` — CDP returns decompressed body; (d) `content-security-policy`, `content-security-policy-report-only` — script hash digests no longer match; (e) `strict-transport-security` — prevents HSTS pinning on ephemeral test sessions.

3. **Fix `run_browser_test` doc comment URL** — Doc says `http://localhost:{P}/tests/{name}` but actual URL is `http://localhost:{P}/{name}` (ServeDir root is `./tests`, so URL path `/{name}` maps to `./tests/{name}/`). Per PATTERNS.md ("When a doc comment references a URL path… verify it matches the actual implementation").

4. **Clean up CSP test fixture** — `tests/csp-script/index.html` includes `<head><title>CSP Script Test</title></head>`. Per PATTERNS.md ("Test HTML fixtures should use the minimal structure needed… Do not add `<head>`, `<title>`, or other elements unless the test specifically exercises them"), remove the unnecessary `<head>` block.

5. **Run full test suite** — Verify all 14 integration tests pass, including the 3 new ones. Confirm `cargo clippy` and `cargo fmt` are clean.
