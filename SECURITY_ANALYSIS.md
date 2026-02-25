# Security Analysis: Fundamental Problems and Risky Assumptions

This document evaluates every identified problem and assumption in the code added on `develop` relative to `antithesishq/main`. For each, it determines: whether the problem is a direct result of the new code, whether it can be disregarded based on existing project patterns and the tool's purpose, and what consequences remain for problems that cannot be disregarded.

---

## Problem 1: Denylist approach assumes completeness

**Description**: `STRIPPED_RESPONSE_HEADERS` is a fixed list of 5 headers. Any header whose validity depends on body content but is not listed will be forwarded with a stale value.

**Origin**: Direct result of the new code — the original code had no denylist because it dropped everything.

**Disregardable?** Yes. The denylist approach is the standard pattern for HTTP header proxying (used by nginx, Varnish, CDN proxies). The alternative — an allowlist enumerating every valid header — is impractical and fragile. The five chosen headers exactly match the set invalidated by body rewriting. The previous behavior (dropping all headers) was strictly worse, silently breaking CORS, HSTS, CSP, and every other security header.

---

## Problem 2: content-type preservation is implicit

**Description**: The `content-type` header — critical for ES module MIME type enforcement (the original bug's root cause) — is preserved only because it is absent from `STRIPPED_RESPONSE_HEADERS`. There is no explicit positive assertion in the production code path.

**Origin**: Direct result of the denylist design.

**Disregardable?** No. This warrants ongoing attention.

**Full consequence analysis**:

- **Failure mode**: If a future maintainer adds `content-type` to `STRIPPED_RESPONSE_HEADERS`, all `<script type="module">` tags loading external scripts silently break. Browsers require a JavaScript MIME type for module scripts (HTML specification mandate) and reject responses without one. The error appears in the browser console with no obvious connection to the header change.
- **Selective impact**: Classic `<script>` tags are lenient about missing content-type, so a regression would only affect module scripts — making it intermittent and hard to diagnose across different applications.
- **Single point of failure**: The denylist is the sole mechanism protecting `content-type`. There is no allowlist, no assertion at the `FulfillRequestParams` builder call site, and no runtime check.
- **CDP replacement semantics**: CDP's `Fetch.fulfillRequest` replaces the entire header set when `responseHeaders` is provided. Omitting `content-type` is equivalent to actively removing it — there is no fallback to original headers.
- **Downstream effects**: Content-type removal would also break `X-Content-Type-Options: nosniff` enforcement, CORS content negotiation, and any framework relying on MIME type for security decisions.
- **Existing guards**: The unit test `build_headers_preserves_content_type` and integration test `test_external_module_script` catch regression. If tests are skipped or deleted, the protection vanishes.
- **Mitigation**: The existing tests provide adequate protection for normal development. A code-level comment on `STRIPPED_RESPONSE_HEADERS` explicitly noting `content-type` must NOT be added would provide defense in depth. Risk is low but not zero.

---

## Problem 3: CSP sanitization does not merge multiple CSP headers

**Description**: HTTP allows multiple `Content-Security-Policy` headers. `sanitize_csp` processes a single header value. If the server sends two, `build_response_headers` processes each independently.

**Origin**: Direct result of the new code.

**Disregardable?** Yes. Processing each CSP header independently then passing all to the browser is correct: browsers intersect multiple CSP headers. Removing hashes from each header separately produces the same effective policy as merging first then removing. The intersection of (A minus hashes) and (B minus hashes) equals (A intersect B) minus hashes.

---

## Problem 4: CSP sanitization is string-based, not formally parsed

**Description**: The CSP parser splits on `;` and whitespace rather than using a formal grammar parser or library.

**Origin**: Direct result of the new code.

**Disregardable?** Yes. CSP directive values are defined as space-separated tokens (W3C CSP Level 3, Section 2.2). The splitting exactly matches the grammar. CSP values do not contain quoted strings, URL-encoding, or escape sequences — they use single-quoted keywords (`'self'`, `'sha256-...'`). A parser dependency would add complexity without benefit.

---

## Problem 5: script-src-attr is not handled

**Description**: Hashes in `script-src-attr` are forwarded unchanged. Only `script-src` and `script-src-elem` are sanitized.

**Origin**: Direct result of the new code.

**Disregardable?** Yes. `script-src-attr` governs inline event handler attributes (`onclick`, `onload`), not `<script>` elements or external scripts. Bombadil's instrumentation modifies script bodies, not event handler attributes. Hash values in `script-src-attr` remain valid after instrumentation.

---

## Problem 6: Stripping report-uri/report-to is aggressive

**Description**: All reporting directives are removed from sanitized CSP, even though only script-hash violations would be instrumentation-induced false positives.

**Origin**: Direct result of the new code.

**Disregardable?** Yes. During testing, instrumentation triggers many mutations beyond hash mismatches (DOM changes, network timing, injected coverage code). Suppressing all CSP reporting during instrumentation prevents noise in violation logs. Selectively suppressing only hash-related reports is not possible because CSP reporting does not distinguish violation causes at the needed granularity.

---

## Problem 7: HSTS interaction with HTTP test servers

**Description**: HSTS headers are forwarded unchanged. If the test server runs on HTTP, the browser might cache HSTS and refuse subsequent HTTP connections.

**Origin**: Not a direct result of the new code — the original code accidentally prevented this by dropping all headers.

**Disregardable?** Yes. Bombadil creates a fresh browser profile (`TempDir`) for each test run, so HSTS state does not persist. Local test servers don't send HSTS. In production HTTPS testing, forwarding HSTS is correct.

---

## Problem 8: Digest header variants not fully covered

**Description**: Only `digest` (RFC 3230) is stripped. RFC 9530's `repr-digest` and `content-digest` are not in the denylist.

**Origin**: Direct result of the new code.

**Disregardable?** Yes. RFC 9530 adoption is negligible (RFC finalized 2024). The failure mode (service worker rejecting response) would be immediately visible and is a one-line fix.

---

## Problem 9: `'unsafe-hashes'` left orphaned when hashes are stripped

**Description**: When hashes are removed from `script-src`, the `'unsafe-hashes'` keyword (if present) is not removed. Without accompanying hashes, `'unsafe-hashes'` is semantically meaningless.

**Origin**: Direct result of the new code.

**Disregardable?** Yes. `'unsafe-hashes'` is primarily for event handler attributes and `javascript:` URLs, not `<script>` elements. When left without hashes, it has no effect on script loading — it does not open or restrict any new attack surface. It is dead CSP syntax, not a security risk.

---

## Problem 10: Inline script hash invalidation in Document responses

**Description**: For Document responses, `instrument_inline_scripts` modifies inline `<script>` bodies, which invalidates any CSP hash values for those scripts. `sanitize_csp` strips the hashes, but if the CSP had only hash values (e.g., `script-src 'sha256-abc'`), the entire directive is omitted, potentially allowing scripts that the original policy would have blocked.

**Origin**: Direct result of the new code.

**Disregardable?** Yes, for the instrumentation use case. Bombadil is a testing/fuzzing tool, not a production proxy. Its purpose is to inject coverage instrumentation into scripts. If a CSP policy blocks instrumented scripts from running, testing becomes impossible. The widening of CSP during testing is an intentional and necessary trade-off — the same trade-off every browser DevTools extension makes when injecting scripts into CSP-protected pages. The original code achieved the same widening more aggressively (by dropping all headers entirely).

---

## Problem 11: strict-dynamic removal widens the effective CSP

**Description**: When hashes/nonces are stripped, `'strict-dynamic'` is also removed. This makes `'unsafe-inline'` and host-based sources active again (they are normally ignored when `'strict-dynamic'` is present).

**Origin**: Direct result of the new code.

**Disregardable?** Yes. Without trust anchors (hashes/nonces), `'strict-dynamic'` would block all dynamically inserted scripts, making testing impossible. Removing it allows fallback sources to take effect. More permissive than production, but necessarily so for instrumentation. The alternative is all scripts being blocked.

---

## Problem 12: The `_` wildcard in CSP resource-type match is unreachable

**Description**: In `build_response_headers`, the match has `Script`, `Document`, and `_ => Some(h.clone())`. Only Script and Document are registered for Fetch interception, so `_` is dead code.

**Origin**: Direct result of the new code.

**Disregardable?** Yes. The arm acts as a safe future-proof default. If someone adds more resource types to interception, CSP passes through unchanged (correct, since instrumentation only modifies Script and Document bodies). However, the arm lacks an explanatory comment, which is a minor pattern deviation.

---

## Problem 13: build_response_headers always appends etag to documents

**Description**: The synthetic etag is appended to every fulfilled response, including HTML documents.

**Origin**: Partially original (old code also set synthetic etag on every response), partially new.

**Disregardable?** Yes. CDP operates within the browser process, not over a network link. Caching proxies between CDP and the page are not realistic. The etag serves instrumentation source ID tracking.

---

## Problem 14: Duplicate CSP directives in a single header

**Description**: If a server sends `script-src 'sha256-a'; script-src 'nonce-b'` (invalid per spec, but servers do it), both directives are processed independently.

**Origin**: Direct result of the new code.

**Disregardable?** Yes. Per the CSP specification, only the first matching directive applies; duplicates are ignored by the browser. Processing both is harmless — at worst, one extra directive appears in the sanitized output, but the browser ignores it.

---

## Problem 15: Compression test only covers gzip

**Description**: `test_compressed_script` uses `CompressionLayer::new()` which defaults to gzip. Brotli and zstd are not tested.

**Origin**: Direct result of the new code.

**Disregardable?** Yes. The logic being tested is `content-encoding` header stripping, which is independent of the specific algorithm. CDP decompresses the body before handing it to the Fetch handler regardless of algorithm.

---

## Summary

| # | Problem | Origin | Disregardable? | Risk |
|---|---------|--------|----------------|------|
| 1 | Denylist completeness | New code | Yes | Low |
| 2 | Implicit content-type preservation | New code | **No** | Medium |
| 3 | Multiple CSP headers | New code | Yes | None |
| 4 | String-based CSP parsing | New code | Yes | None |
| 5 | script-src-attr not handled | New code | Yes | None |
| 6 | Aggressive report stripping | New code | Yes | Low |
| 7 | HSTS interaction | Pre-existing | Yes | None |
| 8 | Digest variants | New code | Yes | Low |
| 9 | Orphaned unsafe-hashes | New code | Yes | None |
| 10 | Inline script hash invalidation | New code | Yes | None |
| 11 | strict-dynamic removal | New code | Yes | Low |
| 12 | Unreachable `_` arm | New code | Yes | Low |
| 13 | Etag on documents | Both | Yes | None |
| 14 | Duplicate CSP directives | New code | Yes | None |
| 15 | Only gzip tested | New code | Yes | None |

**One non-disregardable problem identified (Problem 2).** It is adequately mitigated by existing tests, but the implicit protection warrants awareness during future maintenance. A code-level comment on `STRIPPED_RESPONSE_HEADERS` noting that `content-type` must not be added would strengthen the defense.
