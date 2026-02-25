# Security Analysis: Fundamental Problems and Risky Assumptions

This document covers every identified problem and assumption in the code added to `develop` (relative to `antithesishq/main`), the reasoning for whether each should be acted on, and the projected consequences of the ones that matter.

## Context

`antithesishq/main` dropped all response headers when fulfilling intercepted requests, adding only a synthetic `etag`. This was a known gap marked `// TODO: forward headers`. The `develop` branch replaces this with a header-forwarding pipeline that strips specific headers and applies resource-type-aware CSP sanitization. Four issues identified in a prior analysis (default-src fallback, strict-dynamic orphaning, report-uri/report-to noise, resource type wildcard) have been addressed in the current code. The analysis below evaluates the code as it stands now.

---

## Problem 1: Hash-only `script-src` removal widens the security model

### Description

When `sanitize_csp` strips all values from a `script-src` directive (because every value was a hash, nonce, or `strict-dynamic`), the directive is omitted entirely. With no `script-src` in the sanitized CSP, the browser falls back to `default-src`. If `default-src` is absent or permissive (e.g., `default-src *`), the page can load scripts from any source — a strictly weaker security model than the original hash-only policy.

### Why it cannot be disregarded

This is inherent to Bombadil's approach: instrumentation changes script bodies, invalidating all hash-based trust. There is no way to preserve hash-based CSP while also running instrumented code. The alternative — computing and injecting new hashes for instrumented output — would require Bombadil to know the final instrumented body at CSP-emission time, which is architecturally infeasible since the CSP header is on the Document response while scripts are intercepted separately.

### Why it was ultimately accepted

This is a design-level trade-off, not a bug. Bombadil is a testing tool, not a production proxy. The purpose of CSP sanitization is to prevent instrumentation from breaking the page, not to maintain production-equivalent security posture. Hash-only CSP sites are relatively uncommon (most use `'unsafe-inline'` or nonce-based policies). The weakening only applies during the test session.

### Consequence if deployed

- Pages with hash-only `script-src` and no `default-src` (or a permissive one) will have no script loading restrictions during Bombadil testing. Any XSS vulnerability that would be blocked by CSP in production could execute during testing. This does not affect the application under test — it only affects what Bombadil observes.

---

## Problem 2: `content-security-policy-report-only` treated identically to enforcing CSP

### Description

For Script resources, both `content-security-policy` and `content-security-policy-report-only` are dropped entirely. For Document resources, both are sanitized identically (hashes/nonces stripped, report directives stripped). The `report-only` header is designed to never block anything — it only sends violation reports. Dropping or sanitizing it prevents the application from collecting CSP violation data even during testing.

### Why it was considered

The `report-only` header, after sanitization, has its `report-uri`/`report-to` directives stripped. Without a reporting endpoint, the browser evaluates the policy but has nowhere to send results. The sanitized `report-only` header becomes effectively inert.

For Script resources, CSP headers are typically irrelevant because CSP is enforced per-document, not per-subresource. Dropping CSP from Script responses has no browser-observable effect in standard implementations.

### Why it was accepted

Sanitizing `report-only` the same way as the enforcing header is conservative and consistent. The alternative — passing `report-only` through unchanged — would cause false-positive violation reports (since instrumented scripts would fail hash checks in the report-only policy), which is the same noise problem that report-uri/report-to stripping was designed to prevent.

### Consequence if deployed

- Applications that rely on CSP violation report collection will not receive reports during Bombadil test sessions. This is a temporary gap that ends when testing stops.
- If anyone tests their CSP reporting infrastructure using Bombadil, they would get false results. This is an unusual use case for a fuzzing tool.

---

## Problem 3: Naive string-based CSP parsing

### Description

`sanitize_csp` parses CSP by splitting on `;` for directives and whitespace for values. It does not use a formal CSP parser. CSP values are case-insensitive for directive names and keywords but case-sensitive for URI-based sources and hash values (the base64 portion).

### Connected systems and variables

- **Directive name matching**: Uses `.to_lowercase()` then prefix comparison. This is correct per the CSP spec (directive names are case-insensitive).
- **Value matching**: Uses `.to_lowercase()` on values then checks prefixes like `'sha256-`. Hash algorithm names are case-insensitive per CSP spec, but the base64 hash itself is case-sensitive. Since the code only checks the prefix to decide whether to REMOVE a value (not to validate it), case sensitivity of the hash body is irrelevant.
- **Semicolons in values**: CSP values cannot contain unescaped semicolons. The `;`-split is spec-conformant.
- **Whitespace handling**: CSP directives use SP as separator. The `.split_whitespace()` call handles multiple spaces and leading/trailing whitespace.

### Why it was accepted

The parsing approach handles all standard CSP constructs correctly. Edge cases like `require-trusted-types-for`, `trusted-types`, and `upgrade-insecure-requests` are passed through unchanged because the code only modifies `script-src`, `script-src-elem`, `default-src`, `report-uri`, and `report-to`. A formal CSP parser would add a dependency without providing practical benefit for the subset of directives Bombadil needs to modify.

### Consequence if deployed

- No known CSP value can be mis-parsed by the current approach. The risk is theoretical: a future CSP extension that uses `;` inside a value would break the parser. No such extension exists or is proposed.

---

## Problem 4: `Vary` header forwarded after `content-encoding` removal

### Description

`STRIPPED_RESPONSE_HEADERS` does not include `vary`. If the original response had `Vary: Accept-Encoding` (very common when `content-encoding` is present), the forwarded response advertises `Vary: Accept-Encoding` without actually having a `Content-Encoding` header. This creates a semantic mismatch.

### Why it was disregarded

In a testing tool context, intermediate HTTP caches are not in play. Bombadil creates fresh browser profiles for each test session, so the browser cache starts empty. The `Vary` header primarily affects cache key partitioning in proxies and CDNs.

### Connected systems

- **Service workers**: An application under test could use service workers that inspect `Vary` for caching decisions. A mismatch could cause unexpected cache behavior. However, service workers would need to be explicitly programmed to inspect `Vary`, which is uncommon.

### Consequence if deployed

- Negligible. Browser caching during a short-lived Bombadil test session with a fresh profile is unaffected in practice.

---

## Problem 5: ETag replacement for non-instrumented content

### Description

When a Document response is non-HTML (XML, PDF, etc.), the body passes through unchanged (`body.clone()`), but the header pipeline still runs: `content-encoding` is stripped, and the ETag is replaced with `source_id.0`.

### Why it was disregarded

This behavior is inherited from `antithesishq/main`, which also replaced the ETag for every intercepted response. The new code does not change this — it existed before header forwarding was added. The `source_id` is deterministic (derived from body hash when no request ETag is present), so the synthetic ETag is at least stable for identical content.

---

## Problem 6: SRI (`integrity` attribute) incompatibility

### Description

If a `<script>` tag has an `integrity` attribute (Subresource Integrity), the browser verifies the response body against the hash. After Bombadil instruments the script, the body changes and the SRI check fails. The browser blocks the script.

### Why it was disregarded

This is a pre-existing limitation of Bombadil's interception approach, not a consequence of the header-forwarding changes. `antithesishq/main` had the same issue — any instrumented script would fail SRI verification. The header pipeline does not make this worse or better. Fixing SRI would require either stripping `integrity` attributes from HTML before the browser parses them (modifying the HTML instrumentation pass) or injecting correct hashes for instrumented bodies. Both are out of scope for the header-forwarding work.

---

## Problem 7: Inline iterator chain complexity

### Description

The `.response_headers()` builder argument is a ~40-line iterator chain with `.filter()`, `.flat_map()` containing a `match` with three arms, `.chain()`, and a closure capturing `resource_type`. This is inside the `FulfillRequestParams::builder()` call.

### Why it was noted

The codebase on `antithesishq/main` uses simple builder patterns with named values. The inline chain is hard to review, hard to modify, and hard to test in isolation. Extracting the header-construction logic into a named function would make the builder call read as a sequence of named values and would allow unit-testing the header filtering independently of CDP.

### Consequence if deployed

- No runtime impact. This is a maintainability and review-quality concern. Future modifications to header handling (e.g., adding new resource types or new header transformations) would be easier to get right if the logic is in a named, testable function.

---

## Problem 8: Missing `Content-MD5` in strip list

### Description

`Content-MD5` (RFC 1864) is a body hash similar to `Digest`. It is not in `STRIPPED_RESPONSE_HEADERS`.

### Why it was disregarded

`Content-MD5` was deprecated by RFC 7231 (June 2014) and removed from the HTTP specification. No modern server or CDN generates it. The `Digest` header (its successor via RFC 3230 / RFC 9530) IS in the strip list.

---

## Problem 9: Missing `Accept-Ranges` handling

### Description

If the original response advertised `Accept-Ranges: bytes` and the body was modified, range requests against the instrumented content would return wrong byte offsets.

### Why it was disregarded

Browsers do not make range requests for script or HTML document resources. `Accept-Ranges` is informational — its presence alone causes no breakage, only a subsequent range request would.

---

## Summary table

| # | Problem | Severity | Status |
|---|---------|----------|--------|
| 1 | Hash-only `script-src` removal widens security model | Medium | Accepted — inherent design trade-off |
| 2 | `report-only` treated same as enforcing CSP | Low | Accepted — conservative and consistent |
| 3 | Naive string-based CSP parsing | Low | Accepted — handles all standard CSP correctly |
| 4 | `Vary` header mismatch | Negligible | Disregarded — no practical impact in testing |
| 5 | ETag replacement for non-instrumented content | None | Disregarded — inherited from antithesishq/main |
| 6 | SRI incompatibility | Pre-existing | Not in scope — existed before header forwarding |
| 7 | Inline iterator chain complexity | Code quality | Should be addressed for maintainability |
| 8 | Missing `Content-MD5` | None | Deprecated header |
| 9 | Missing `Accept-Ranges` | None | Range requests on scripts not realistic |
