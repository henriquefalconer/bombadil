# Implementation Plan

## Completed

- Forward response headers in `Fetch.fulfillRequest` (replacing the old single-etag approach)
- Strip stale transport headers (`content-length`, `content-encoding`, `transfer-encoding`) after instrumentation
- Test infrastructure: refactored `run_browser_test` into wrapper + `run_browser_test_with_router`
- Test for external module script (`test_external_module_script`)
- Test for compressed script (`test_compressed_script`)
- Strip CSP and HSTS headers (`content-security-policy`, `content-security-policy-report-only`, `strict-transport-security`) in `src/browser/instrumentation.rs` with rationale comment
- Add CSP integration test (`test_csp_script`) with fixture files in `tests/csp-script/`
- Fix pattern violations in `test_external_module_script` and `test_compressed_script`: removed `///` doc comments, changed `export { scroll }` to `export { clicks }`, raised timeout from 10s to 20s (≥2× LTL `.within()` bound)
- Fix doc comment placement: moved full doc comment to `run_browser_test` (the wrapper tests call), added one-liner on `run_browser_test_with_router`, fixed typo "facitiliate" → "facilitate", removed "and a custom router" from first line

## TODO (priority order)

(none)
