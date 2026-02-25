# Security Analysis — Fundamental Problems and Risky Assumptions

Analysis of the code added in `main` relative to `antithesishq/main`, covering header forwarding in `src/browser/instrumentation.rs` and supporting test infrastructure.

---

## Methodology

Each item below was evaluated through three lenses:
1. **Is it a direct result of the new code?** (vs. pre-existing in antithesishq/main)
2. **Does it contradict established patterns** in antithesishq/main?
3. **Can it cause real harm** if deployed as-is?

Items that are pre-existing, follow established patterns, or have negligible real-world impact are noted as such and not escalated.

---

## Fundamental Problem 1: Denylist Header Filtering

**Classification:** Direct result of new code. Cannot be disregarded.

### Description

The new code forwards all original response headers except a hardcoded list of 7 names (`etag`, `content-length`, `content-encoding`, `transfer-encoding`, `content-security-policy`, `content-security-policy-report-only`, `strict-transport-security`). This is a denylist (blocklist) approach.

### Assumption

That the 7 headers listed are the complete set of headers whose validity depends on body content or whose presence causes problems after instrumentation.

### Why This Is Risky

HTTP headers are an open namespace. Servers, CDNs, reverse proxies, and frameworks routinely add custom or newer standard headers that depend on body content. A denylist cannot anticipate these. Specific examples of headers not in the strip list:

- **`digest` / `repr-digest`** (RFC 9530): Contains a cryptographic hash of the response body. If a CDN or proxy adds this, the instrumented body won't match, and downstream integrity checks will fail silently or cause hard-to-diagnose errors.
- **`content-range`**: If a response were ever 206 Partial Content that slipped through (the 200-only guard helps but edge cases exist with some CDPs), the range would be wrong.
- **`x-content-type-options: nosniff`**: While not body-dependent, its interaction with MIME handling could matter when the instrumented response changes content characteristics.
- **Custom CDN headers** (e.g., `cf-cache-status`, `x-cache`, `x-amz-content-sha256`): Some of these carry body-dependent checksums.

### Consequences If Deployed

1. **Silent integrity failures**: A production site behind a CDN that adds `repr-digest` headers would see Bombadil forward stale digests. Downstream proxies or service workers that verify these would reject the response, causing scripts to fail to load with no clear error message pointing to Bombadil.
2. **Cache poisoning risk**: If `vary`, `age`, or CDN-specific cache headers interact with the stale `etag` and missing `content-length`, caches may store the instrumented version keyed incorrectly, serving it to non-Bombadil requests in shared cache scenarios.
3. **Growing maintenance burden**: Each new body-dependent header standard requires updating the denylist. Forgetting to do so creates a latent bug.

### Alternative Considered

An allowlist approach (only forward known-safe headers like `content-type`, `set-cookie`, `cache-control`, `access-control-*`) would be safer — unknown headers are dropped by default, failing closed rather than open. The tradeoff is that legitimate headers might be lost, but in Bombadil's context (testing tool, not production proxy), this is acceptable.

---

## Fundamental Problem 2: CSP Stripping Masks Application-Level CSP Issues

**Classification:** Direct result of new code. Should be evaluated carefully but may be acceptable with caveats.

### Description

The code strips `content-security-policy` and `content-security-policy-report-only` headers entirely. This is necessary because instrumentation changes script bodies, breaking CSP hash-based allowlists.

### Assumption

That stripping CSP entirely is acceptable because Bombadil is a testing tool and the CSP would only block Bombadil's instrumentation, not reveal real application bugs.

### Why This Is Risky

CSP is a defense-in-depth mechanism. By stripping it entirely:

1. **XSS vulnerabilities in the tested application become invisible during Bombadil testing.** If the app has a CSP that would block an XSS vector, Bombadil's testing won't surface the fact that removing CSP opens the app to XSS.
2. **The tested application's behavior changes.** Some applications use CSP `report-uri` or `report-to` to send violation reports. Stripping CSP means the application's violation reporting code path is never exercised during Bombadil testing.
3. **`script-src 'unsafe-inline'` detection is lost.** If an app relies on CSP to prevent inline script execution and Bombadil strips that, the app may behave differently (inline scripts that would normally be blocked now execute).

### Consequences If Deployed

For most Bombadil use cases (fuzzing UI interactions), this is acceptable — Bombadil's purpose is to find property violations, not CSP misconfigurations. However, if a user writes a property that depends on CSP behavior (e.g., testing that certain scripts are blocked), Bombadil would silently make that property untestable.

### Mitigation

This is a known tradeoff inherent to instrumentation-based tools. The current approach is pragmatic. A future improvement could selectively modify CSP (e.g., add Bombadil's instrumentation script hashes to the allowlist rather than stripping the entire header), but this is significantly more complex.

---

## Fundamental Problem 3: HSTS Stripping Rationale Is Test-Specific

**Classification:** Direct result of new code. Acceptable but the code comment overstates the scope.

### Description

`strict-transport-security` is stripped with the comment "prevent pinning HTTPS on localhost, which would break subsequent test runs."

### Assumption

That HSTS only matters in the test context (localhost) and stripping it has no consequences for non-test usage.

### Why This Merits Attention

The stripping code runs in production code (`src/browser/instrumentation.rs`), not in test infrastructure. When Bombadil is used against real sites:

1. The browser won't pin HSTS for the target domain during testing. This is generally fine (Bombadil doesn't persist browser state), but if `--user-data-directory` is shared across runs, HSTS state from the real site would normally accumulate.
2. More importantly, the comment's rationale ("localhost") is specific to tests but the code runs for all targets. The comment should reflect the general case: "HSTS is stripped because Bombadil replaces responses via CDP and the HSTS directive would apply to Bombadil's interception endpoint, not the real server."

### Consequences If Deployed

Negligible real-world impact. HSTS stripping in a testing tool is harmless. The only concern is comment accuracy for maintainability.

---

## Fundamental Problem 4: Test Timeout / LTL `.within()` Ratio

**Classification:** Direct result of new code. Pattern deviation from antithesishq/main.

### Description

The three new tests all use `Duration::from_secs(20)` as the test timeout and `.within(10, "seconds")` as the LTL bound. This is a 2.0x ratio.

### Assumption

That 2x is sufficient headroom between the LTL bound and the test timeout.

### Why This Is Risky

The test harness treats `Timeout` as `Success` for `Expect::Success` tests. If the test times out before the LTL engine can fully evaluate the property, the test passes vacuously — it never actually verified anything.

In antithesishq/main:
- `test_back_from_non_html`: 30s test / 20s LTL = 1.5x (the lowest ratio upstream)
- `test_random_text_input`: 120s test / 10s LTL = 12x

The new tests at 2.0x are within the existing range but on the tighter side. Under CI resource contention (slow I/O, CPU throttling), browser startup alone can take 5-10 seconds, leaving minimal time for the LTL engine to cycle.

### Consequences If Deployed

Tests could pass in CI without actually exercising the property. This wouldn't cause production bugs but would create false confidence in test coverage. The fix is simple: use `TEST_TIMEOUT_SECONDS` (120s) as the test timeout, giving a 12x ratio.

---

## Fundamental Problem 5: Hardcoded CSP Hash Couples Test Fixture to Test Code

**Classification:** Direct result of new code. Minor but worth noting.

### Description

`test_csp_script` hardcodes the SHA-256 hash of `tests/csp-script/script.js` in the middleware:

```rust
"script-src 'sha256-sRoPO3cqhmVEQTMEK66eATz8J/LJdrvqrNVuMKzGgSM='"
```

### Assumption

That the content of `script.js` won't change.

### Why This Is Risky

If someone modifies `script.js` (even adding a newline), the hash no longer matches. The test would still pass because Bombadil strips CSP regardless, but it would no longer be testing the intended scenario (CSP hash blocking an instrumented script). The test becomes a tautology.

### Consequences If Deployed

No production impact. The test would silently lose its diagnostic value if the fixture changes.

---

## Risky Assumption 1: `GetResponseBody` Always Returns Decompressed Content

**Classification:** Pre-existing assumption from antithesishq/main, but newly relevant.

### Description

The code calls `Fetch.getResponseBody` which, per CDP documentation, returns the body after content decoding (i.e., decompressed). The new code strips `content-encoding` based on this assumption.

### Why This Merits Attention

If a future Chromium version or a CDP protocol change altered this behavior (returning compressed content), the instrumentation would try to parse compressed bytes as JavaScript, fail, and fall through to the error handler (which continues the request uninstrumented). This would silently disable coverage instrumentation for compressed scripts.

The `test_compressed_script` test would catch this regression, which is good.

### Assessment

The assumption is well-founded (CDP has documented this behavior since Chrome 64) and the test provides a safety net. Acceptable.

---

## Risky Assumption 2: `response_headers` in `EventRequestPaused` Contains All Response Headers

**Classification:** Pre-existing structure, newly relied upon.

### Description

The new code reads `event.response_headers` to forward original headers. This field is `Option<Vec<HeaderEntry>>`.

### Why This Merits Attention

If `response_headers` is `None` (which the code handles via `.iter().flatten()`), the response gets only the appended `etag` — losing all headers including `content-type`. This regression path matches the old behavior (antithesishq/main only sent `etag`), so it's not worse than before, but it means the fix is not effective for those cases.

### Assessment

`response_headers` being `None` is unlikely for responses that reach the fulfillment path (they must have a 200 status code, implying a complete response). Acceptable.

---

## Items Evaluated and Disregarded

| Item | Reason for disregarding |
|------|------------------------|
| Hardcoded `response_code(200)` | Pre-existing in antithesishq/main; non-200 responses are already handled by early return |
| No explicit `content-type` header set | The new code actually fixes this by forwarding the original `content-type` |
| `app.clone()` in `run_browser_test_with_router` | Follows the same pattern as antithesishq/main (which cloned inline); Router cloning is documented as cheap in Axum |
| Etag duplication risk | The filter strips ALL matching `etag` headers before appending a fresh one; no duplication possible |
| Header stripping on non-HTML pass-through documents | Stripping `content-encoding` on an already-decompressed body is correct; `content-length` absence is handled by CDP |
