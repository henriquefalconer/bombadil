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
- Shared `tests/shared/script.js`; `make_csp_router` helper

## Remaining

### 1. Unit tests for `content-security-policy-report-only` in `build_response_headers`

The code handles `content-security-policy-report-only` identically to enforcing CSP (line 382–384 of `instrumentation.rs`), but no unit test exercises this path. Per PATTERNS.md, each critical header path needs a test enforcing it against regressions.

Add two tests:
- `build_headers_drops_report_only_csp_for_script_resources`
- `build_headers_sanitizes_report_only_csp_for_document_resources`

### 2. Make `test_compressed_script` clearly fail without the fix

Currently this test likely passes on both `main` and `develop` because regular `<script>` tags don't enforce MIME checking. Change the fixture to `<script type="module">` so the test fails on `main` (no content-type → module MIME check fails) while passing on `develop` (content-type preserved). This makes the test demonstrate both the content-encoding fix and the content-type fix.

### 3. Make `test_csp_script` clearly fail without the fix

Currently passes on both branches because `main` drops all headers including CSP (so no CSP blocks the script). Restructure to also verify CSP enforcement for non-script directives (e.g., add `img-src 'none'` and check for a violation event), so the test fails on `main` (no CSP at all → no violation) while passing on `develop` (CSP sanitized but present). Alternatively, merge into `test_csp_document_directives_preserved` if the test becomes redundant.

### 4. Run full test suite

After all changes, verify `cargo test` passes for unit and integration tests.
