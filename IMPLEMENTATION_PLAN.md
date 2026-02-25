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

## Remaining

**1. Remove section-separator comment in unit test module**
In `src/browser/instrumentation.rs` line 569, remove the `// ── build_response_headers ──...` separator comment. PATTERNS.md prohibits section-separator comments inside unit test modules.

**2. Change integration test timeouts from 20s to 30s**
All four new integration tests use `Duration::from_secs(20)`. PATTERNS.md requires using only existing timeout tiers (3s, 5s, 30s, 120s). Change all four to `Duration::from_secs(30)`.

**3. Remove multi-line comments from test functions**
- `test_csp_script` (lines 524-527): replace the 4-line comment block with a single line or remove it.
- `test_csp_document_directives_preserved` (lines 567-581): replace the 13-line comment block with a single line or remove it.
PATTERNS.md allows at most one line of comment for non-obvious setup.

**4. Deduplicate identical JS files across test fixtures**
`tests/compressed-script/script.js`, `tests/csp-script/script.js`, and `tests/external-module-script/module.js` are byte-identical. Move the file to one location (e.g., `tests/shared/script.js`) and update the HTML fixtures to reference it via a relative path. PATTERNS.md requires shared files to be referenced rather than duplicated.

**5. Extract shared CSP router setup into a helper**
`test_csp_script` and `test_csp_document_directives_preserved` duplicate the Router + middleware closure construction, differing only in the CSP string. Extract a named helper function (e.g., `make_csp_router`) that takes the CSP value as a parameter. PATTERNS.md requires shared setup logic to be extracted rather than duplicated inline.
