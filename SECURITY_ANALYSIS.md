# Security Analysis: Fundamental Problems and Risky Assumptions

This document covers every identified problem and assumption in the code added to `develop` (relative to `antithesishq/main`), the reasoning for whether each should be acted on, and the projected consequences of the ones that matter.

## Context

`antithesishq/main` dropped all response headers when fulfilling intercepted requests, adding only a synthetic `etag`. This was a known gap marked `// TODO: forward headers`. The `develop` branch replaces this with a header-forwarding pipeline that strips specific headers and applies resource-type-aware CSP sanitization. The pipeline is encapsulated in `build_response_headers` with CSP logic in `sanitize_csp`.

---

## Problem 1: Hash-only `script-src` removal widens the security model

### Description

When `sanitize_csp` strips all values from a `script-src` directive (because every value was a hash, nonce, or `strict-dynamic`), the directive is omitted entirely. With no `script-src` in the sanitized CSP, the browser falls back to `default-src`. If `default-src` is absent or permissive (e.g., `default-src *`), the page can load scripts from any source — a strictly weaker security model than the original hash-only policy.

### Is this a direct result of the added code?

Yes. This behavior is introduced by `sanitize_csp`. `antithesishq/main` dropped ALL headers including CSP, which was also a total CSP bypass — so the security model was already absent. The new code is strictly better (it preserves non-hash CSP directives), but the specific mechanism of "omit directive when all values are hashes" is new.

### Can it be disregarded?

Partially. The prior state (`antithesishq/main`) had no CSP at all after interception, so this is an improvement. The widening is inherent to any approach short of recomputing hashes for instrumented output, which is architecturally infeasible (CSP is on the Document response while scripts are intercepted separately). This is a design-level trade-off, not a bug.

### Consequence if deployed

Pages with hash-only `script-src` and no `default-src` (or a permissive one) will have no script loading restrictions during Bombadil testing. Any XSS vulnerability that would be blocked by CSP in production could execute during testing. This does not affect the application under test in production — it only affects what Bombadil observes during the test session. The prior behavior (`antithesishq/main`) had identical exposure since CSP was entirely absent.

---

## Problem 2: `content-security-policy-report-only` treated identically to enforcing CSP

### Description

For Script resources, both `content-security-policy` and `content-security-policy-report-only` are dropped entirely. For Document resources, both are sanitized identically (hashes/nonces stripped, report directives stripped). The `report-only` header is designed to never block anything — it only sends violation reports. Dropping or sanitizing it prevents the application from collecting CSP violation data during testing.

### Is this a direct result of the added code?

Yes. The `build_response_headers` function explicitly checks for both header names.

### Can it be disregarded?

Yes. Sanitizing `report-only` the same way as the enforcing header is conservative and consistent. Passing it through unchanged would cause false-positive violation reports (instrumented scripts fail hash checks), generating noise at the application's reporting endpoint. After sanitization removes `report-uri`/`report-to`, the report-only header becomes effectively inert, so the treatment is harmless.

---

## Problem 3: Naive string-based CSP parsing

### Description

`sanitize_csp` parses CSP by splitting on `;` for directives and whitespace for values. It does not use a formal CSP parser.

### Is this a direct result of the added code?

Yes.

### Can it be disregarded?

Yes. The parsing approach handles all standard CSP constructs correctly:
- Directive name matching uses `.to_lowercase()` then prefix comparison — correct per CSP spec (directive names are case-insensitive).
- Value matching uses `.to_lowercase()` on values then checks prefixes like `'sha256-` — since the code only checks prefixes to decide whether to REMOVE a value (not to validate its content), case sensitivity of the base64 body is irrelevant.
- CSP values cannot contain unescaped semicolons. The `;`-split is spec-conformant.
- `.split_whitespace()` correctly handles multiple spaces and leading/trailing whitespace.

Edge cases like `require-trusted-types-for`, `trusted-types`, and `upgrade-insecure-requests` are passed through unchanged because the code only modifies `script-src`, `script-src-elem`, `default-src`, `report-uri`, and `report-to`. A formal CSP parser would add a dependency without practical benefit.

---

## Problem 4: `Vary` header forwarded after `content-encoding` removal

### Description

`STRIPPED_RESPONSE_HEADERS` does not include `vary`. If the original response had `Vary: Accept-Encoding` (very common when `content-encoding` is present), the forwarded response advertises `Vary: Accept-Encoding` without actually having a `Content-Encoding` header.

### Is this a direct result of the added code?

Yes. `antithesishq/main` dropped all headers, so `Vary` was never forwarded.

### Can it be disregarded?

Yes. In a testing tool context, intermediate HTTP caches are not in play. Bombadil creates fresh browser profiles for each test session, so the browser cache starts empty. The `Vary` header primarily affects cache key partitioning in proxies and CDNs. An application's service worker could theoretically inspect `Vary` for caching decisions, but this is extremely uncommon.

---

## Problem 5: ETag replacement for non-instrumented content

### Description

When a Document response is non-HTML (XML, PDF, etc.), the body passes through unchanged (`body.clone()`), but the header pipeline still runs: `content-encoding` is stripped, and the ETag is replaced with `source_id.0`.

### Is this a direct result of the added code?

No. `antithesishq/main` already replaced the ETag for every intercepted response. The new code does not change this behavior.

### Can it be disregarded?

Yes — inherited from `antithesishq/main`, not introduced by this branch.

---

## Problem 6: SRI (`integrity` attribute) incompatibility

### Description

If a `<script>` tag has an `integrity` attribute (Subresource Integrity), the browser verifies the response body against the hash. After Bombadil instruments the script, the body changes and the SRI check fails.

### Is this a direct result of the added code?

No. This is a pre-existing limitation of Bombadil's interception approach. `antithesishq/main` had the same issue.

### Can it be disregarded?

Yes — not in scope for the header-forwarding work.

---

## Problem 7: `_ =>` wildcard in resource type matching for CSP

### Description

`build_response_headers` uses `_ => Some(h.clone())` for the CSP match on resource type. Only `Script` and `Document` are registered as interception targets in `instrument_js_coverage`, so the wildcard arm should never execute.

### Is this a direct result of the added code?

Yes. The match expression is entirely new.

### Can it be disregarded?

Yes, with a note. The wildcard arm preserves CSP headers unchanged for unknown types, which is the safe default (failing closed would mean dropping CSP, which is less safe). The existing body instrumentation code uses `bail!` for unexpected resource types, but the two code paths have different correctness requirements: body instrumentation needs to choose a strategy, while header handling has a universally safe default (forward unchanged). If interception registration is later expanded, CSP headers for new types would be forwarded unchanged without explicit consideration — safe but potentially missing an opportunity.

---

## Problem 8: `content-type` preservation is implicit, not explicit

### Description

`content-type` preservation is guaranteed only by its absence from `STRIPPED_RESPONSE_HEADERS`. There is no positive assertion or comment in the strip list documenting that `content-type` MUST NOT be stripped.

### Is this a direct result of the added code?

Yes. The strip list design is new. In `antithesishq/main` all headers were dropped, so there was no strip list to mismanage — but `content-type` was also not preserved.

### Can it be disregarded?

Partially. There is no immediate runtime impact — `content-type` is correctly forwarded. The risk is future regression if the strip list is modified without understanding that `content-type` is critical for ES module MIME type checking, HTML parsing, and CSS/image/font type enforcement. The existing `STRIPPED_RESPONSE_HEADERS` doc comment explains what IS stripped but does not call out what must NOT be stripped.

### Consequence if deployed as-is

No immediate issue. Future maintainers editing the strip list without understanding this dependency could silently reintroduce the original module script bug (ISSUE.md).

---

## Problem 9: Missing `Content-MD5` in strip list

### Description

`Content-MD5` (RFC 1864) is a body hash similar to `Digest`. It is not in `STRIPPED_RESPONSE_HEADERS`.

### Can it be disregarded?

Yes. `Content-MD5` was deprecated by RFC 7231 (June 2014) and removed from the HTTP specification. No modern server or CDN generates it. The `Digest` header (its successor) IS in the strip list.

---

## Problem 10: Missing `Accept-Ranges` handling

### Description

If the original response advertised `Accept-Ranges: bytes` and the body was modified, range requests against the instrumented content would return wrong byte offsets.

### Can it be disregarded?

Yes. Browsers do not make range requests for script or HTML document resources. `Accept-Ranges` is informational — its presence alone causes no breakage.

---

## Problem 11: Section separator comment in unit test module

### Description

The unit test module in `instrumentation.rs` contains `// ── build_response_headers ──…` — a section separator comment. The project's PATTERNS.md explicitly states: "Do not use section-separator comments inside unit test modules to organize tests by topic."

### Is this a direct result of the added code?

Yes.

### Can it be disregarded?

Yes for security purposes. This is a style issue, not a security concern. It is documented in PATTERNS.md deviation analysis instead.

---

## Problem 12: New timeout tier in integration tests

### Description

All four new integration tests use `Duration::from_secs(20)` — a timeout value not present in any existing test in `antithesishq/main`. Existing tests use 3s, 5s, 30s, or 120s (`TEST_TIMEOUT_SECONDS`).

### Is this a direct result of the added code?

Yes.

### Can it be disregarded?

For security purposes, yes. This is a consistency concern, not a security one. However, the test harness treats `Timeout` as `Success` for `Expect::Success` tests, and the specs use `.within(10, "seconds")`. A 20s timeout gives only a 2x margin over the `.within()` bound, which matches the existing 5s/3s pairings but introduces a new tier that may confuse future maintainers.

---

## Summary table

| # | Problem | Severity | Status |
|---|---------|----------|--------|
| 1 | Hash-only `script-src` removal widens security model | Medium | Accepted — inherent design trade-off, strictly better than antithesishq/main |
| 2 | `report-only` treated same as enforcing CSP | Low | Accepted — conservative and consistent |
| 3 | Naive string-based CSP parsing | Low | Accepted — handles all standard CSP correctly |
| 4 | `Vary` header mismatch | Negligible | Disregarded — no practical impact in testing |
| 5 | ETag replacement for non-instrumented content | None | Inherited from antithesishq/main |
| 6 | SRI incompatibility | Pre-existing | Not in scope — existed before header forwarding |
| 7 | `_ =>` wildcard in CSP resource type match | Low | Accepted — safe default, different from body instrumentation |
| 8 | `content-type` preservation is implicit | Low | Noted — regression risk if strip list is modified blindly |
| 9 | Missing `Content-MD5` | None | Deprecated header |
| 10 | Missing `Accept-Ranges` | None | Range requests on scripts not realistic |
| 11 | Section separator comment in test module | None | Style issue, not security |
| 12 | New timeout tier (20s) in integration tests | None | Consistency issue, not security |
