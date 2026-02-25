# Security Considerations

## Header Forwarding in `Fetch.fulfillRequest`

### Context

`src/browser/instrumentation.rs` intercepts script and document responses via CDP, instruments the JavaScript body for coverage, and calls `Fetch.fulfillRequest` to deliver the modified response to the browser. The current implementation forwards all original response headers except a blocklist of four (`etag`, `content-length`, `content-encoding`, `transfer-encoding`) and appends a synthetic `etag`.

CDP's `Fetch.fulfillRequest` uses **replacement semantics**: providing `responseHeaders` replaces the entire header set. There is no merge with the original headers.

### Risk: Silent Instrumentation Failure on CSP-Protected Applications

**Severity: High**

When a server sends a `Content-Security-Policy` header containing `script-src` directives with **hash-based** (`sha256-...`) or **nonce-based** (`nonce-...`) restrictions:

1. The hash/nonce was computed against the **original** script body.
2. Bombadil replaces the body with an **instrumented** version (different content, different hash).
3. The original CSP header is forwarded unchanged.
4. Chrome enforces CSP, finds the hash mismatch, and **blocks the instrumented script**.

**Consequences:**
- Instrumented scripts silently fail to execute.
- No JavaScript coverage is collected.
- Bombadil appears to run normally (navigates, takes actions, records traces).
- A user could run a full fuzz campaign against a CSP-protected app with zero meaningful coverage and no indication of the problem.
- CSP violations appear in the browser console but may not trigger Bombadil's `noUncaughtExceptions` or `noConsoleErrors` default properties (CSP violations are not JS exceptions or `console.error` calls by default).

**Note:** The upstream code (which dropped all headers) accidentally avoided this by not forwarding CSP headers at all. The current implementation is a regression for CSP-protected apps specifically.

### Risk: HSTS Pinning on Localhost

**Severity: Low**

If the origin server sends `Strict-Transport-Security`, forwarding it in the fulfilled response could cause Chrome to pin HSTS for `localhost`. This would affect subsequent test runs or local development by forcing HTTPS on localhost connections.

### Mitigation Options

1. **Allowlist instead of blocklist:** Forward only headers known to be safe and necessary (`Content-Type`, `Cache-Control`, `Access-Control-*`, etc.) rather than forwarding everything minus a blocklist. This is safer but risks missing app-specific headers.

2. **Extend the blocklist:** Add `content-security-policy`, `content-security-policy-report-only`, and `strict-transport-security` to the stripped headers. This addresses the known risks but remains fragile against future problematic headers.

3. **Strip and re-derive CSP:** Remove CSP headers entirely, matching the upstream's accidental behavior. Since Bombadil modifies script content by design, CSP hash enforcement is fundamentally incompatible with instrumentation.

## Test Reliability

### Risk: Timeout-Based False Pass

**Severity: Medium**

`test_external_module_script` and `test_compressed_script` both use a test timeout (`Duration::from_secs(10)`) equal to the LTL property timeout (`.within(10, "seconds")`). The test harness treats `Timeout` as `Success`. If the tokio timer fires before the LTL engine produces a violation, the test passes vacuously — without ever verifying the script loaded.

This means a regression in the header fix could go undetected in CI, especially on slow machines where the LTL clock drifts behind wall-clock time.

**Mitigation:** Set the test timeout strictly greater than the LTL `.within()` bound (e.g., 20s test timeout with 10s LTL bound), matching the pattern used by existing tests like `test_back_from_non_html` (30s test / 20s LTL).
