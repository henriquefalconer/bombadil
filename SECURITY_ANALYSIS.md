# Security Analysis: Fundamental Problems and Risky Assumptions

This document evaluates every identified problem and assumption in the code added to `develop` relative to `antithesishq/main`. For each item: the problem is described, its origin is traced (direct result of added code vs pre-existing), a decision is made on whether it can be disregarded, and consequences are assessed for those that cannot.

## Context

`antithesishq/main` dropped all response headers when fulfilling intercepted requests, replacing them with a single synthetic `etag`. This was a known gap marked `// TODO: forward headers`. The `develop` branch replaces this with a header-forwarding pipeline that strips specific headers and applies resource-type-aware CSP sanitisation. The pipeline is encapsulated in `build_response_headers` with CSP logic in `sanitize_csp`.

---

## Problem 1: Hash-only `script-src` removal widens the script-loading model

**Description:** When `sanitize_csp` strips all values from a `script-src` directive (because every value was a hash, nonce, or `strict-dynamic`), the directive is omitted entirely. The browser falls back to `default-src`, which may be more permissive.

**Direct result of added code?** Yes. The specific mechanism of "omit directive when all values are stripped" is new.

**Can it be disregarded?** Yes. `antithesishq/main` dropped ALL headers including CSP, which was a total CSP bypass — strictly worse. The new code is an improvement: it preserves non-hash directives. The widening is inherent to any instrumentation approach that modifies script bodies (recomputing hashes is architecturally infeasible since CSP is on the Document response while scripts are intercepted separately). Bombadil is a testing tool, not a production proxy.

---

## Problem 2: `content-security-policy-report-only` treated identically to enforcing CSP

**Description:** Both `content-security-policy` and `content-security-policy-report-only` are dropped for Script resources and sanitised for Document resources. The `report-only` header never blocks anything — it only sends violation reports.

**Direct result of added code?** Yes.

**Can it be disregarded?** Yes. Passing `report-only` through unchanged would cause false-positive reports (instrumented scripts fail hash checks), generating noise at the application's reporting endpoint. After sanitisation removes `report-uri`/`report-to`, the report-only header becomes inert. This is conservative and consistent.

---

## Problem 3: CSP parsing is string-based, not via a formal parser

**Description:** `sanitize_csp` splits on `;` for directives and whitespace for values, without a formal CSP parser.

**Direct result of added code?** Yes.

**Can it be disregarded?** Yes. The approach handles all standard CSP constructs correctly:
- Directive names: case-insensitive via `.to_lowercase()` (correct per spec).
- Value matching: only checks prefixes to decide removal, so base64 case sensitivity is irrelevant.
- CSP values cannot contain unescaped semicolons; `;`-split is spec-conformant.
- Edge directives (`require-trusted-types-for`, `trusted-types`, `upgrade-insecure-requests`) pass through unchanged.
A formal CSP parser would add a dependency without practical benefit.

---

## Problem 4: `Vary` header forwarded after `content-encoding` removal

**Description:** `STRIPPED_RESPONSE_HEADERS` does not include `vary`. If the original response had `Vary: Accept-Encoding`, the forwarded response advertises it without having a `Content-Encoding` header.

**Direct result of added code?** Yes. `antithesishq/main` dropped all headers, so `Vary` was never forwarded.

**Can it be disregarded?** Yes. `Vary` affects HTTP cache key partitioning in proxies and CDNs. Bombadil creates fresh browser profiles per test session; intermediate caches are not in play.

---

## Problem 5: Headers now forwarded that were previously dropped

**Description:** The original code replaced ALL headers with a synthetic `etag` (fail-closed). The new code preserves all headers except those in `STRIPPED_RESPONSE_HEADERS` and CSP (fail-open by default). This means headers previously suppressed are now forwarded: `set-cookie`, `cache-control`, `x-frame-options`, CORS headers, HSTS, etc.

**Direct result of added code?** Yes. This is the core behavioural change.

**Can it be disregarded?** Yes — this is the intended fix. The original "drop everything" was a known bug. Specific headers:
- `set-cookie`: Desired in testing (sessions, CSRF tokens).
- `cache-control`: Could cause browser to cache instrumented responses, but Bombadil controls the browser lifecycle (fresh profiles per test). Within a test run, caching is beneficial (avoids re-instrumentation).
- `x-frame-options`: Harmless for scripts; correct for documents.
- CORS: Correct to forward — instrumented responses should have the same CORS policy.
- HSTS: Irrelevant for localhost testing; correct for real domains.

---

## Problem 6: `_ =>` wildcard in `build_response_headers` CSP match

**Description:** The CSP match on `resource_type` uses `_ => Some(h.clone())` for types other than `Script` and `Document`. Currently dead code since only those two types are intercepted.

**Direct result of added code?** Yes.

**Can it be disregarded?** Yes, with a note. The wildcard preserves CSP unchanged for unknown types — the safe default. If interception is later expanded, CSP for new types would pass through unchanged without explicit consideration, which is conservative (not stripping is safer than stripping for non-instrumented resources). The `STRIPPED_RESPONSE_HEADERS` doc block documents that CSP handling is resource-type-aware, providing guidance for future changes.

---

## Problem 7: `content-type` preservation is implicit

**Description:** `content-type` — critical for ES module MIME type checking — is preserved only by its absence from `STRIPPED_RESPONSE_HEADERS`. No explicit safeguard prevents its accidental addition.

**Direct result of added code?** Yes. The strip list design is new.

**Can it be disregarded?** Partially. No runtime issue exists. The risk is future regression: a maintainer editing the strip list without understanding the dependency could silently reintroduce the original module script bug. The unit test `build_headers_preserves_content_type` provides a regression guard, and PATTERNS.md documents that critical preservations should be called out near the strip list.

**Consequence if deployed:** No immediate issue. The regression risk is mitigated by the unit test but could benefit from a comment in the strip list itself.

---

## Problem 8: Missing `content-md5` in strip list

**Description:** `Content-MD5` (RFC 1864) is a body hash like `Digest` but is not in `STRIPPED_RESPONSE_HEADERS`.

**Can it be disregarded?** Yes. Deprecated by RFC 7231 (2014) and removed from the HTTP specification. No modern server generates it. Its successor `Digest` IS in the strip list.

---

## Problem 9: SRI (`integrity` attribute) incompatibility

**Description:** `<script>` tags with `integrity` attributes will fail SRI checks after body instrumentation.

**Direct result of added code?** No. Pre-existing limitation of Bombadil's interception approach. `antithesishq/main` had the identical issue.

**Can it be disregarded?** Yes — not in scope for header forwarding work.

---

## Problem 10: ETag replacement for non-instrumented content

**Description:** Non-HTML Document responses pass through with body unchanged but still receive a synthetic ETag.

**Direct result of added code?** No. `antithesishq/main` already replaced ETag for every intercepted response.

**Can it be disregarded?** Yes — inherited, not introduced.

---

## Problem 11: `source_id` reads request headers for ETag, not response headers

**Description:** The `source_id` function receives headers from `event.request.headers` (the browser's request), not `event.response_headers` (the server's response). ETag is a response header.

**Direct result of added code?** No. The develop branch did not modify this function.

**Can it be disregarded?** Yes. Impact is low: (a) if no ETag is found, fallback hashes the body deterministically; (b) source ID is used for coverage deduplication, not security.

---

## Problem 12: Debug writes to `/tmp/` use unsanitised filenames

**Description:** Instrumented scripts are written to `/tmp/{safe_filename}` where `safe_filename` only replaces `?#&=`.

**Direct result of added code?** No. Unchanged from `antithesishq/main`.

**Can it be disregarded?** Yes. `split('/').next_back()` takes only the last URL segment, preventing path traversal. The `/tmp/` prefix confines writes.

---

## Problem 13: Multiple CSP headers handled independently

**Description:** HTTP allows multiple CSP headers; `build_response_headers` processes each independently via `flat_map`.

**Direct result of added code?** Yes.

**Can it be disregarded?** Yes. Processing independently is correct. The browser enforces the intersection of all CSP headers. Stripping hashes from one header doesn't weaken another. If anything, it makes the intersection less restrictive, which is the desired effect for instrumentation.

---

## Summary

| # | Problem | Introduced by develop? | Status |
|---|---------|----------------------|--------|
| 1 | Hash-only `script-src` removal widens security model | Yes | Disregarded — inherent trade-off, strictly better than antithesishq/main |
| 2 | `report-only` CSP treated same as enforcing | Yes | Disregarded — conservative, prevents false-positive reports |
| 3 | String-based CSP parsing | Yes | Disregarded — handles all standard CSP correctly |
| 4 | `Vary` header mismatch | Yes | Disregarded — no practical impact in testing context |
| 5 | Previously-dropped headers now forwarded | Yes | Disregarded — intended fix, all forwarded headers are correct |
| 6 | `_ =>` wildcard in CSP match | Yes | Disregarded — safe conservative default |
| 7 | `content-type` preservation implicit | Yes | Noted — mitigated by unit test, low regression risk |
| 8 | Missing `content-md5` | Yes | Disregarded — deprecated header |
| 9 | SRI incompatibility | No | Pre-existing |
| 10 | ETag for non-instrumented content | No | Pre-existing |
| 11 | `source_id` reads request headers | No | Pre-existing |
| 12 | Debug writes to `/tmp/` | No | Pre-existing |
| 13 | Multiple CSP headers independent | Yes | Disregarded — correct behaviour |
