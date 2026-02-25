# Implementation Plan

## Completed

All items implemented and tests passing.

- `STRIPPED_RESPONSE_HEADERS` constant (etag, content-length, content-encoding, transfer-encoding, digest)
- `sanitize_csp` function with default-src fallback, strict-dynamic orphan removal, report directive stripping
- `build_response_headers` helper with resource-type-aware CSP handling (drop for Script, sanitize for Document, passthrough for others)
- `FulfillRequestParams` builder updated to use `build_response_headers`
- 19 unit tests for `sanitize_csp`, 9 unit tests for `build_response_headers`
- 4 integration tests: `test_external_module_script`, `test_compressed_script`, `test_csp_script`, `test_csp_document_directives_preserved`
- HTML fixtures, shared `tests/shared/script.js`, `make_csp_router` helper
- `run_browser_test` split into wrapper + `run_browser_test_with_router`
- `Cargo.toml`: `compression-gzip` feature for `tower-http`
- Fixed `compressed-script/index.html`: removed `type="module"` from script tag
- Removed stale `log::info!("just changing for CI")` debug line from `test_browser_lifecycle`

## Remaining

None.
