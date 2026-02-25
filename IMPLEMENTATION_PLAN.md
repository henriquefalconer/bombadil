# Implementation Plan

## Completed

All planned items have been implemented and all tests pass (78 unit + 15 integration).

### Summary of completed work

- `STRIPPED_RESPONSE_HEADERS` constant (etag, content-length, content-encoding, transfer-encoding, digest)
- `sanitize_csp` function with default-src fallback, strict-dynamic orphan removal, report directive stripping
- `build_response_headers` helper with resource-type-aware CSP handling (drop for Script, sanitize for Document, passthrough for others)
- `FulfillRequestParams` builder updated to use `build_response_headers`
- 19 unit tests for `sanitize_csp`
- 9 unit tests for `build_response_headers` (including 2 for `content-security-policy-report-only`)
- 4 integration tests: `test_external_module_script`, `test_compressed_script`, `test_csp_script`, `test_csp_document_directives_preserved`
- HTML fixtures for all integration tests
- `run_browser_test` split into wrapper + `run_browser_test_with_router`
- `Cargo.toml`: `compression-gzip` feature for `tower-http`
- Shared `tests/shared/script.js`; `make_csp_router` helper
- `test_compressed_script` uses `<script type="module">` so it fails on `main` (no content-type → MIME check fails) and passes on `develop` (content-type preserved)
- `test_csp_script` restructured to verify both Script CSP stripping and Document CSP preservation: adds `img-src 'none'` to the CSP and checks for both "LOADED" (script ran) and "CSP_ACTIVE" (violation fired), so it fails on `main` (no doc CSP → no violation) and passes on `develop` (doc CSP sanitized, img-src remains)
- `build_headers_drops_report_only_csp_for_script_resources` and `build_headers_sanitizes_report_only_csp_for_document_resources` unit tests added

## Remaining

*(none)*
