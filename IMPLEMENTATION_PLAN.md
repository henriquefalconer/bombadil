# Implementation Plan

## Completed

Core fix and tests are implemented. All 28 unit tests and 15 integration tests pass.

- `STRIPPED_RESPONSE_HEADERS` constant (etag, content-length, content-encoding, transfer-encoding, digest)
- `sanitize_csp` function with default-src fallback, strict-dynamic orphan removal, report directive stripping
- `build_response_headers` helper with resource-type-aware CSP handling (drop for Script, sanitize for Document, passthrough for others)
- `FulfillRequestParams` builder updated to use `build_response_headers`
- 19 unit tests for `sanitize_csp`, 9 unit tests for `build_response_headers`
- 4 integration tests: `test_external_module_script`, `test_compressed_script`, `test_csp_script`, `test_csp_document_directives_preserved`
- HTML fixtures, shared `tests/shared/script.js`, `make_csp_router` helper
- `run_browser_test` split into wrapper + `run_browser_test_with_router`
- `Cargo.toml`: `compression-gzip` feature for `tower-http`

## Remaining

- **Fix `compressed-script/index.html`**: Change `<script type="module" src="/shared/script.js">` to `<script src="/shared/script.js">`. PATTERNS.md explicitly prohibits mixing module MIME concerns into the compression test fixture — module loading is already covered by `test_external_module_script`. The test still fails on `antithesishq/main` because all headers are dropped (stale `content-encoding` after decompression would cause double-decompress / garbled script).
- **Remove stale debug line**: Delete `log::info!("just changing for CI");` on line 407 of `tests/integration_tests.rs` (leftover CI cache-bust in `test_browser_lifecycle`).
