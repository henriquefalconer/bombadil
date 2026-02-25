# Implementation Plan

## Completed

- Forward response headers in `Fetch.fulfillRequest` (replacing the old single-etag approach)
- Strip stale transport headers (`content-length`, `content-encoding`, `transfer-encoding`) after instrumentation
- Test infrastructure: refactored `run_browser_test` into wrapper + `run_browser_test_with_router`
- Test for external module script (`test_external_module_script`)
- Test for compressed script (`test_compressed_script`)
- Test fixtures for both (`tests/external-module-script/`, `tests/compressed-script/`)

## TODO (priority order)

### 1. Strip CSP and HSTS headers in `src/browser/instrumentation.rs`

Add to the header strip list at line 169:
- `"content-security-policy"` — instrumentation changes script content, invalidating CSP hashes/nonces; Chrome silently blocks instrumented scripts (High — SECURITY.md)
- `"content-security-policy-report-only"` — same reason as above
- `"strict-transport-security"` — could pin HSTS on localhost, breaking subsequent test runs (Low — SECURITY.md)

Add a comment above the array explaining why these headers are stripped (body-dependent headers become stale after instrumentation).

### 2. Add CSP integration test (`test_csp_script`)

Verify that scripts served with CSP `script-src` hash restrictions load correctly after Bombadil strips the CSP header. Before the fix, Chrome blocks the instrumented script (hash mismatch → `eventually` violation). After the fix, the CSP header is stripped and the script executes normally.

- Create `tests/csp-script/index.html`: HTML page with `<script src="/csp-script/script.js">` and `<h1 id="result">WAITING</h1>`
- Create `tests/csp-script/script.js`: `document.getElementById("result").textContent = "LOADED";`
- Add test function `test_csp_script` using `run_browser_test_with_router` with a custom Axum router that adds a `Content-Security-Policy` response header containing a `script-src 'sha256-...'` directive (hash computed from the **original** script.js content)
- Spec: `eventually(() => resultText.current === "LOADED").within(10, "seconds")`
- Test timeout: 20s (2× the LTL bound, per PATTERNS.md)
- Export `clicks` (not `scroll`, per PATTERNS.md)
- No `///` doc comments on the test function (per PATTERNS.md)

### 3. Fix pattern violations in `test_external_module_script` and `test_compressed_script`

Both tests violate PATTERNS.md in three ways:
- **Doc comments:** Remove `///` doc comments from both test functions (lines 471–475 and 498–505)
- **Action export:** Change `export { scroll }` to `export { clicks }` in both specs
- **Timeout ratio:** Change `Duration::from_secs(10)` to `Duration::from_secs(20)` in both tests (test timeout must be ≥2× the LTL `.within()` bound to prevent vacuous pass)
