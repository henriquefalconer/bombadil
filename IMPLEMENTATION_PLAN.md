# Implementation Plan

## Completed

- `STRIPPED_RESPONSE_HEADERS` constant (etag, content-length, content-encoding, transfer-encoding, digest)
- `sanitize_csp` function with default-src fallback, strict-dynamic orphan removal, report directive stripping
- `build_response_headers` helper with resource-type-aware CSP handling (drop for Script, sanitize for Document, passthrough for others)
- `FulfillRequestParams` builder updated to use `build_response_headers`
- 19 unit tests for `sanitize_csp`
- 7 unit tests for `build_response_headers`
- 4 integration tests: `test_external_module_script`, `test_compressed_script`, `test_csp_script`, `test_csp_document_directives_preserved`
- HTML fixtures for all new integration tests
- `run_browser_test` split into wrapper + `run_browser_test_with_router`
- `Cargo.toml`: `compression-gzip` feature for `tower-http`
- Removed section-separator comment from unit test module in `instrumentation.rs`
- Changed all four new integration test timeouts from 20s to 30s (valid tier per PATTERNS.md)
- Replaced multi-line block comments in `test_csp_script` and `test_csp_document_directives_preserved` with single-line summaries
- Consolidated byte-identical JS files into `tests/shared/script.js`; updated HTML fixtures to reference `/shared/script.js`
- Extracted `make_csp_router` helper to eliminate duplicated Router+middleware construction in CSP tests

## Remaining

(none)
