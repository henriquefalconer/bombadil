# Implementation Plan

## Completed

All core fix code and tests implemented and passing (28 unit tests, 4 integration tests).

- `STRIPPED_RESPONSE_HEADERS` constant (etag, content-length, content-encoding, transfer-encoding, digest)
- `sanitize_csp` function with default-src fallback, strict-dynamic orphan removal, report directive stripping
- `build_response_headers` helper with resource-type-aware CSP handling (drop for Script, sanitize for Document, passthrough for others)
- `FulfillRequestParams` builder updated to use `build_response_headers`
- 19 unit tests for `sanitize_csp`, 9 unit tests for `build_response_headers`
- 4 integration tests: `test_external_module_script`, `test_compressed_script`, `test_csp_script`, `test_csp_document_directives_preserved`
- HTML fixtures, shared `tests/shared/script.js`, `make_csp_router` helper
- `run_browser_test` split into wrapper + `run_browser_test_with_router`
- `Cargo.toml`: `compression-gzip` feature for `tower-http`
- `test_compressed_script` fixture updated to use `<script type="module">` so the test distinguishes old (drops `content-type` → module MIME check fails) vs. new code (`content-type` preserved, `content-encoding` stripped → test passes); PATTERNS.md rule updated accordingly

## Remaining

(none)
