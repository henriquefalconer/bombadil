# Security Analysis: Fundamental Problems and Risky Assumptions

This document covers every identified problem and assumption in the code added to `develop` (relative to `antithesishq/main`), the reasoning for whether each should be acted on, and the projected consequences of the ones that matter.

## Context

`antithesishq/main` dropped all response headers when fulfilling intercepted requests, adding only a synthetic `etag`. This was a known gap marked `// TODO: forward headers`. The `develop` branch replaces this with a header-forwarding pipeline that strips specific headers and applies resource-type-aware CSP sanitization. The pipeline is encapsulated in `build_response_headers` with CSP logic in `sanitize_csp`. Four issues identified in a prior analysis (default-src fallback, strict-dynamic orphaning, report-uri/report-to noise, resource type wildcard) have been addressed. The analysis below evaluates the code as it stands now.

---

## Problem 1: Hash-only `script-src` removal widens the security model

### Description

When `sanitize_csp` strips all values from a `script-src` directive (because every value was a hash, nonce, or `strict-dynamic`), the directive is omitted entirely. With no `script-src` in the sanitized CSP, the browser falls back to `default-src`. If `default-src` is absent or permissive (e.g., `default-src *`), the page can load scripts from any source — a strictly weaker security model than the original hash-only policy.

### Why it cannot be disregarded

This is inherent to Bombadil's approach: instrumentation changes script bodies, invalidating all hash-based trust. There is no way to preserve hash-based CSP while also running instrumented code. The alternative — computing and injecting new hashes for instrumented output — would require Bombadil to know the final instrumented body at CSP-emission time, which is architecturally infeasible since the CSP header is on the Document response while scripts are intercepted separately.

### Why it was ultimately accepted

This is a design-level trade-off, not a bug. Bombadil is a testing tool, not a production proxy. The purpose of CSP sanitization is to prevent instrumentation from breaking the page, not to maintain production-equivalent security posture. The weakening only applies during the test session.

### Consequence if deployed

- Pages with hash-only `script-src` and no `default-src` (or a permissive one) will have no script loading restrictions during Bombadil testing. Any XSS vulnerability that would be blocked by CSP in production could execute during testing. This does not affect the application under test — it only affects what Bombadil observes.

---

## Problem 2: `content-security-policy-report-only` treated identically to enforcing CSP

### Description

For Script resources, both `content-security-policy` and `content-security-policy-report-only` are dropped entirely. For Document resources, both are sanitized identically (hashes/nonces stripped, report directives stripped). The `report-only` header is designed to never block anything — it only sends violation reports. Dropping or sanitizing it prevents the application from collecting CSP violation data during testing.

### Why it was accepted

Sanitizing `report-only` the same way as the enforcing header is conservative and consistent. The alternative — passing `report-only` through unchanged — would cause false-positive violation reports (since instrumented scripts would fail hash checks in the report-only policy), generating noise at the application's reporting endpoint. After sanitization removes `report-uri`/`report-to`, the report-only header becomes effectively inert, so the treatment is harmless.

### Consequence if deployed

- Applications that rely on CSP violation report collection will not receive reports during Bombadil test sessions. This is a temporary gap that ends when testing stops.
- If anyone tests their CSP reporting infrastructure using Bombadil, they would get false results. This is an unusual use case for a fuzzing tool.

---

## Problem 3: Naive string-based CSP parsing

### Description

`sanitize_csp` parses CSP by splitting on `;` for directives and whitespace for values. It does not use a formal CSP parser. CSP values are case-insensitive for directive names and keywords but case-sensitive for URI-based sources and hash values (the base64 portion).

### Connected systems and variables

- **Directive name matching**: Uses `.to_lowercase()` then prefix comparison. Correct per CSP spec (directive names are case-insensitive).
- **Value matching**: Uses `.to_lowercase()` on values then checks prefixes like `'sha256-`. Hash algorithm names are case-insensitive per CSP spec, and since the code only checks prefixes to decide whether to REMOVE a value (not to validate its content), case sensitivity of the base64 body is irrelevant.
- **Semicolons in values**: CSP values cannot contain unescaped semicolons. The `;`-split is spec-conformant.
- **Whitespace handling**: CSP directives use SP as separator. `.split_whitespace()` handles multiple spaces and leading/trailing whitespace.

### Why it was accepted

The parsing approach handles all standard CSP constructs correctly. Edge cases like `require-trusted-types-for`, `trusted-types`, and `upgrade-insecure-requests` are passed through unchanged because the code only modifies `script-src`, `script-src-elem`, `default-src`, `report-uri`, and `report-to`. A formal CSP parser would add a dependency without providing practical benefit for the subset of directives Bombadil needs to modify.

### Consequence if deployed

- No known CSP value can be mis-parsed by the current approach. The risk is theoretical: a future CSP extension that uses `;` inside a value would break the parser. No such extension exists or is proposed.

---

## Problem 4: `Vary` header forwarded after `content-encoding` removal

### Description

`STRIPPED_RESPONSE_HEADERS` does not include `vary`. If the original response had `Vary: Accept-Encoding` (very common when `content-encoding` is present), the forwarded response advertises `Vary: Accept-Encoding` without actually having a `Content-Encoding` header. This creates a semantic mismatch.

### Why it was disregarded

In a testing tool context, intermediate HTTP caches are not in play. Bombadil creates fresh browser profiles for each test session, so the browser cache starts empty. The `Vary` header primarily affects cache key partitioning in proxies and CDNs. An application's service worker could theoretically inspect `Vary` for caching decisions, but this is extremely uncommon.

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

## Problem 7: `_ =>` wildcard in resource type matching for CSP

### Description

`build_response_headers` uses `_ => Some(h.clone())` for the CSP match on resource type. Only `Script` and `Document` are registered as interception targets in `instrument_js_coverage`, so the wildcard arm should never execute. The existing pattern in the body instrumentation code uses `bail!` for unexpected resource types rather than a silent passthrough.

### Why it was accepted

The wildcard arm preserves CSP headers unchanged for unknown types, which is the safe default (failing closed would mean dropping CSP, which is less safe). Unlike body instrumentation — which cannot proceed without knowing the resource type — header handling can safely pass through unmodified headers. The two code paths have different correctness requirements: body instrumentation needs to choose a strategy (JS vs HTML vs passthrough), while header handling has a universally safe default (forward unchanged).

### Consequence if deployed

- If the interception registration is later expanded to include other resource types (e.g., `Stylesheet`), CSP headers for those types would be forwarded unchanged without any explicit consideration. This is safe but could miss an opportunity to strip CSP when needed.

---

## Problem 8: `content-type` preservation is implicit, not explicit

### Description

The fix for the module script issue (ISSUE.md) was to forward all response headers including `content-type`. But `content-type` preservation is guaranteed only by its absence from `STRIPPED_RESPONSE_HEADERS`. There is no positive assertion or comment in the strip list documenting that `content-type` MUST NOT be stripped. If someone later adds `content-type` to the strip list (e.g., thinking the body type changed), the module script bug silently returns.

### Connected systems

- **ES modules**: Browsers enforce strict MIME type checking for `<script type="module">`. If `content-type` is absent, the browser rejects the script with "Expected a JavaScript-or-Wasm module script but the server responded with a MIME type of ''." This was the original bug that motivated the entire header-forwarding effort.
- **HTML documents**: Browsers use `content-type` to decide whether to parse as HTML, XML, or plain text. Stripping it would cause content sniffing, which may produce different results.
- **CSS, images, fonts**: All enforced MIME type checking for their respective resource types.

### Consequence if deployed as-is

- No immediate runtime impact — `content-type` is correctly forwarded. The risk is future regression if the strip list is modified without understanding this dependency.

---

## Problem 9: Missing `Content-MD5` in strip list

### Description

`Content-MD5` (RFC 1864) is a body hash similar to `Digest`. It is not in `STRIPPED_RESPONSE_HEADERS`.

### Why it was disregarded

`Content-MD5` was deprecated by RFC 7231 (June 2014) and removed from the HTTP specification. No modern server or CDN generates it. The `Digest` header (its successor via RFC 3230 / RFC 9530) IS in the strip list.

---

## Problem 10: Missing `Accept-Ranges` handling

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
| 7 | `_ =>` wildcard in CSP resource type match | Low | Accepted — safe default, different from body instrumentation |
| 8 | `content-type` preservation is implicit | Low | Noted — regression risk if strip list is modified blindly |
| 9 | Missing `Content-MD5` | None | Deprecated header |
| 10 | Missing `Accept-Ranges` | None | Range requests on scripts not realistic |
