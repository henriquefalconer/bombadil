# Security Analysis: Fundamental Problems and Risky Assumptions

Evaluates every identified problem and assumption in the code added to `develop` relative to `antithesishq/main`. For each: the problem is described, its origin is traced, a decision is made on whether it can be disregarded, and consequences are assessed for those that cannot.

## Context

`antithesishq/main` dropped all response headers when fulfilling intercepted requests, replacing them with a single synthetic `etag`. This was a known gap marked `// TODO: forward headers`. The `develop` branch replaces this with a header-forwarding pipeline that strips specific headers and applies resource-type-aware CSP sanitisation.

---

## Problem 1: Hash-only `script-src` removal widens the script-loading model

**Description:** When `sanitize_csp` strips all values from a `script-src` directive (because every value was a hash, nonce, or `strict-dynamic`), the directive is omitted. The browser then falls back to `default-src`, which may be more permissive than the original `script-src`.

**Direct result of added code?** Yes — the "omit directive when all values stripped" mechanism is new.

**Can it be disregarded?** Yes. `antithesishq/main` dropped ALL CSP headers — a total bypass, strictly worse. The new code preserves non-hash directives. This widening is inherent to any instrumentation approach that modifies script bodies (recomputing hashes is architecturally infeasible since CSP lives on the Document response while scripts are intercepted separately). Bombadil is a testing tool, not a production proxy.

---

## Problem 2: `content-security-policy-report-only` treated identically to enforcing CSP

**Description:** Both `content-security-policy` and `content-security-policy-report-only` are dropped for Script resources and sanitised for Document resources. `report-only` never blocks — it only sends violation reports.

**Direct result of added code?** Yes.

**Can it be disregarded?** Yes. Passing `report-only` through unchanged would generate false-positive reports at the application's endpoint (instrumented scripts fail hash checks). After sanitisation removes `report-uri`/`report-to`, `report-only` becomes inert. This is conservative and consistent.

---

## Problem 3: CSP parsing is string-based, not via a formal parser

**Description:** `sanitize_csp` splits on `;` for directives and whitespace for values.

**Direct result of added code?** Yes.

**Can it be disregarded?** Yes. The approach handles all standard CSP correctly:
- Directive names: case-insensitive via `.to_lowercase()` (correct per spec).
- Value matching: only prefix checks, so base64 case sensitivity is irrelevant.
- CSP values cannot contain unescaped semicolons; `;`-split is spec-conformant.
- Edge directives (`require-trusted-types-for`, `trusted-types`, `upgrade-insecure-requests`) pass through unchanged.
A formal CSP parser would add a dependency without practical benefit.

---

## Problem 4: `Vary` header forwarded after `content-encoding` removal

**Description:** `STRIPPED_RESPONSE_HEADERS` does not include `vary`. If the original response had `Vary: Accept-Encoding`, the forwarded response advertises content negotiation that no longer applies.

**Direct result of added code?** Yes. `antithesishq/main` dropped all headers, so `Vary` was never forwarded.

**Can it be disregarded?** Yes. `Vary` affects HTTP cache key partitioning in proxies and CDNs. Bombadil uses fresh browser profiles per test; intermediate caches are not in play.

---

## Problem 5: Headers previously dropped are now forwarded

**Description:** The original code replaced ALL headers (fail-closed). The new code preserves all except the denylist (fail-open by default). Headers like `set-cookie`, `cache-control`, `x-frame-options`, CORS, and HSTS are now forwarded.

**Direct result of added code?** Yes — this is the core behavioural change.

**Can it be disregarded?** Yes — this is the intended fix:
- `set-cookie`: Desired in testing (sessions, CSRF tokens).
- `cache-control`: Bombadil controls browser lifecycle (fresh profiles per test). Within a run, caching avoids re-instrumentation.
- `x-frame-options`: Harmless for scripts, correct for documents.
- CORS: Correct to forward — instrumented responses need the same CORS policy.
- HSTS: Irrelevant for localhost testing, correct for real domains.

---

## Problem 6: `_ =>` wildcard in `build_response_headers` CSP match

**Description:** The match on `resource_type` uses `_ => Some(h.clone())` for types other than `Script` and `Document`. Currently dead code since only those two types are intercepted.

**Direct result of added code?** Yes.

**Can it be disregarded?** Yes. The wildcard preserves CSP unchanged for unknown types — the safe default. If interception is later expanded, CSP for new types would pass through unchanged, which is conservative (not stripping is safer than stripping for non-instrumented resources).

---

## Problem 7: `content-type` preservation is implicit (not disregardable)

**Description:** `content-type` — critical for ES module MIME type checking — is preserved only by its absence from `STRIPPED_RESPONSE_HEADERS`. No explicit safeguard prevents its accidental addition to the strip list.

**Direct result of added code?** Yes.

**Can it be disregarded?** No. This was the original root cause bug that motivated the entire change set. Regression here would silently break all ES module scripts.

**Connected systems and variables:**
- The browser enforces MIME type for `<script type="module">` (strict MIME type checking per HTML spec). Without `content-type: text/javascript`, modules fail with a MIME type error.
- The denylist is the only mechanism protecting `content-type`. There is no allowlist, no assertion at the builder call site, and no runtime check.
- The unit test `build_headers_preserves_content_type` is the sole regression guard. If someone deletes or skips tests, the protection vanishes.
- PATTERNS.md documents that critical preservations should be noted near the strip list, but a comment is advisory — it cannot prevent a code change.

**Consequences of regression:**
- All `<script type="module">` tags would silently fail — the exact bug that prompted this work.
- Non-module scripts would still load (browsers are lenient about `content-type` for classic scripts), making the regression intermittent and hard to diagnose.
- The `test_external_module_script` integration test would catch this, but only if tests are run.

**Mitigation assessment:** The unit test provides adequate protection for normal development. The risk is low but not zero. A `debug_assert!` or compile-time check that `content-type` is not in the strip list would be stronger, but given Bombadil's testing-tool context and the existing test coverage, the current approach is acceptable.

---

## Problem 8: Missing `content-md5` in strip list

**Description:** `Content-MD5` (RFC 1864) is a body hash like `Digest` but is not in `STRIPPED_RESPONSE_HEADERS`.

**Can it be disregarded?** Yes. Deprecated by RFC 7231 (2014) and removed from the HTTP specification. No modern server generates it. Its successor `Digest` IS in the strip list.

---

## Problem 9: SRI (`integrity` attribute) incompatibility

**Description:** `<script>` tags with `integrity` attributes will fail SRI checks after body instrumentation.

**Direct result of added code?** No. Pre-existing limitation — `antithesishq/main` had the identical issue.

**Can it be disregarded?** Yes — not in scope for header forwarding work.

---

## Problem 10: ETag replacement for non-instrumented content

**Description:** Non-HTML Document responses pass through with body unchanged but still receive a synthetic ETag, invalidating conditional request caching.

**Direct result of added code?** No. `antithesishq/main` already replaced ETag for every intercepted response.

---

## Problem 11: `source_id` reads request headers for ETag, not response headers

**Description:** The `source_id` function receives headers from `event.request.headers` (the browser's request), not `event.response_headers` (the server's response). ETag is a response header, so the request-header lookup always misses.

**Direct result of added code?** No. Unchanged from `antithesishq/main`.

**Can it be disregarded?** Yes. Fallback hashes the body deterministically. Source ID is used for coverage deduplication, not security.

---

## Problem 12: Debug writes to `/tmp/` use unsanitised filenames

**Description:** Instrumented scripts are written to `/tmp/{safe_filename}` where `safe_filename` only replaces `?#&=`.

**Direct result of added code?** No. Unchanged from `antithesishq/main`.

**Can it be disregarded?** Yes. `split('/').next_back()` takes only the last URL segment, preventing path traversal. The `/tmp/` prefix confines writes. In a testing context, writing debug files to `/tmp/` is standard practice.

---

## Problem 13: Multiple CSP headers handled independently

**Description:** HTTP allows multiple CSP headers; `build_response_headers` processes each independently via `flat_map`.

**Direct result of added code?** Yes.

**Can it be disregarded?** Yes. Processing independently is correct. The browser enforces the intersection of all CSP headers. Stripping hashes from each header independently is semantically equivalent to stripping them from a merged header.

---

## Problem 14: CSP sanitisation does not account for `style-src` or other `-src` directives with hashes

**Description:** `sanitize_csp` only strips hashes/nonces from `script-src`, `script-src-elem`, and (fallback) `default-src`. Other directives like `style-src` can also contain hashes. If Bombadil ever instruments CSS or other resources, these would become stale.

**Direct result of added code?** Yes — the selective stripping logic is new.

**Can it be disregarded?** Yes. Bombadil only instruments JavaScript (scripts and inline scripts in HTML documents). CSS, images, fonts, and other resources are not modified, so their hash-based CSP values remain valid. If instrumentation scope expands, the CSP logic would need revisiting, but that is a future concern outside the current change set.

---

## Summary

| # | Problem | Introduced? | Disregardable? | Notes |
|---|---------|-------------|----------------|-------|
| 1 | Hash-only script-src removal | Yes | Yes | Inherent trade-off, strictly better than main |
| 2 | Report-only CSP same as enforcing | Yes | Yes | Conservative, prevents false reports |
| 3 | String-based CSP parsing | Yes | Yes | Handles standard CSP correctly |
| 4 | Vary header mismatch | Yes | Yes | No caches in testing context |
| 5 | Previously-dropped headers forwarded | Yes | Yes | Intended fix |
| 6 | Wildcard in CSP match | Yes | Yes | Safe conservative default |
| 7 | content-type preservation implicit | Yes | **No** | Mitigated by unit test; regression risk is low but non-zero |
| 8 | Missing content-md5 | Yes | Yes | Deprecated header |
| 9 | SRI incompatibility | No | Yes | Pre-existing |
| 10 | ETag for non-instrumented content | No | Yes | Pre-existing |
| 11 | source_id reads request headers | No | Yes | Pre-existing |
| 12 | Debug /tmp/ writes | No | Yes | Pre-existing |
| 13 | Multiple CSP headers independent | Yes | Yes | Correct behaviour |
| 14 | Non-script -src hashes untouched | Yes | Yes | Only JS is instrumented |

**One non-disregardable problem identified (Problem 7).** It is adequately mitigated by the existing unit test and integration test, but the implicit nature of the protection warrants awareness during future maintenance.
