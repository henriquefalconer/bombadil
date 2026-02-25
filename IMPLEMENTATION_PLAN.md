# Implementation Plan

## Completed

- Forward response headers in `Fetch.fulfillRequest` (replacing the old single-etag approach)
- Strip stale transport headers (`content-length`, `content-encoding`, `transfer-encoding`) after instrumentation
- Test infrastructure: refactored `run_browser_test` into wrapper + `run_browser_test_with_router`
- Test for external module script (`test_external_module_script`)
- Test for compressed script (`test_compressed_script`)
- Test fixtures for both (`tests/external-module-script/`, `tests/compressed-script/`)

## TODO (priority order)

### 1. Strip CSP headers to prevent silent instrumentation failure (High — SECURITY.md)

**Problem:** `content-security-policy` and `content-security-policy-report-only` headers are forwarded unchanged. When a server uses hash-based or nonce-based CSP, the hash/nonce was computed against the original script body. After Bombadil instruments the script (changing its content), the hash no longer matches, and Chrome silently blocks execution. This means zero coverage is collected with no indication of the problem.

**Fix in `src/browser/instrumentation.rs`:**
- Add `"content-security-policy"` and `"content-security-policy-report-only"` to the header strip list (the array inside the `.filter()` closure)
- Add a comment explaining why (instrumentation changes script content, invalidating CSP hashes/nonces)

### 2. Strip HSTS header to prevent localhost pinning (Low — SECURITY.md)

**Problem:** `strict-transport-security` is forwarded, which could pin HSTS on localhost and break subsequent test runs.

**Fix in `src/browser/instrumentation.rs`:**
- Add `"strict-transport-security"` to the same header strip list
- Add a comment explaining why

### 3. Add CSP integration test (`test_csp_script`)

**Purpose:** Verify that scripts served with CSP `script-src` hash restrictions load correctly after Bombadil strips the CSP header. Before the fix, Chrome would block the instrumented script (hash mismatch). After the fix, the CSP header is stripped and the script executes normally.

**Implementation:**
- Create `tests/csp-script/index.html`: HTML page with `<script src="/csp-script/script.js">` and `<h1 id="result">WAITING</h1>`
- Create `tests/csp-script/script.js`: `document.getElementById("result").textContent = "LOADED";`
- Add test function `test_csp_script` using `run_browser_test_with_router` with a custom router that adds a `Content-Security-Policy` header containing a `script-src 'sha256-...'` directive (hash computed from the **original** script.js content)
- Spec: `eventually(() => resultText.current === "LOADED").within(10, "seconds")`
- Test timeout: 20s (2x the LTL bound, per PATTERNS.md)
- Export `clicks` (not `scroll`, per PATTERNS.md)
- No `///` doc comments on the test function (per PATTERNS.md)

### 4. Fix pattern violations in existing tests

**Problem:** `test_external_module_script` and `test_compressed_script` violate PATTERNS.md:

a. **Doc comments on test functions:** Both have `///` doc comments. Per PATTERNS.md, test functions should not carry doc comments.
   - **Fix:** Remove the `///` doc comments from both test functions.

b. **`scroll` instead of `clicks`:** Both export `scroll` as their action. Per PATTERNS.md, tests should use `clicks` as the baseline action unless specifically testing scroll behavior.
   - **Fix:** Change `export { scroll } from "@antithesishq/bombadil/defaults";` to `export { clicks } from "@antithesishq/bombadil/defaults";` in both test specs.

c. **Timeout-based false pass (Medium — SECURITY.md):** Both use a 10s test timeout with a 10s LTL `.within()` bound. The test harness treats `Timeout` as `Success`, so the test can pass vacuously if the LTL engine hasn't had time to produce a violation. Per PATTERNS.md, test timeout should be at least 2x the LTL bound.
   - **Fix:** Change test timeout from `Duration::from_secs(10)` to `Duration::from_secs(20)` in both tests.
