# Implementation Plan

## Completed

- `STRIPPED_RESPONSE_HEADERS` constant (etag, content-length, content-encoding, transfer-encoding, digest)
- `sanitize_csp` function with default-src fallback, strict-dynamic orphan removal, report directive stripping
- `build_response_headers` helper with resource-type-aware CSP handling (drop for Script, sanitize for Document, passthrough for others)
- `FulfillRequestParams` builder updated to use `build_response_headers`
- 19 unit tests for `sanitize_csp` covering all SECURITY.md resolved issues
- 7 unit tests for `build_response_headers` covering header stripping, CSP per resource type, synthetic etag
- 4 integration tests: `test_external_module_script`, `test_compressed_script`, `test_csp_script`, `test_csp_document_directives_preserved`
- HTML fixtures for all new integration tests
- `run_browser_test` split into wrapper + `run_browser_test_with_router`
- `Cargo.toml`: `compression-gzip` feature for `tower-http`
- Section-separator comments removed from test module

## Remaining

- Fix test count in `COMPARISON.md` Change 5: "22 tests for sanitize_csp" → "19 tests for sanitize_csp"; "29 tests total" → "26 tests total"
