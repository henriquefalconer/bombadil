# Security Analysis: Fundamental Problems and Risky Assumptions

This document evaluates every identified problem and assumption in the code added on `develop` relative to `antithesishq/main`, determines which are direct results of the new code, which can be disregarded based on existing project patterns, and what consequences remain for those that cannot.

---

## Problem 1: Denylist approach assumes completeness

**Description**: `STRIPPED_RESPONSE_HEADERS` is a fixed list of 5 headers. Any header that should be stripped but is not listed will be forwarded with a stale value.

**Origin**: Direct result of the new code — the original code had no denylist because it dropped everything.

**Can it be disregarded?** Yes. The alternative (allowlist) would require enumerating every possible valid header, which is impractical and fragile. The denylist approach is the standard pattern for HTTP header proxying (nginx, Varnish, CDN proxies all use denylist-style header filtering). The five chosen headers exactly match the set invalidated by body rewriting. The previous behavior of dropping all headers was strictly worse because it silently broke CORS, HSTS, CSP, and all other security headers.

---

## Problem 2: content-type preservation is implicit

**Description**: The `content-type` header — critical for ES module MIME type enforcement — is preserved only because it is absent from `STRIPPED_RESPONSE_HEADERS`. There is no explicit positive protection.

**Origin**: Direct result of the new code's denylist design.

**Can it be disregarded?** No. This is the one problem that warrants ongoing attention.

**Consequence analysis**: If a future maintainer adds `content-type` to `STRIPPED_RESPONSE_HEADERS` (e.g., to fix a perceived issue with stale MIME types), all `<script type="module">` tags loading external scripts would silently break — browsers require a JavaScript MIME type for module scripts and reject responses without one. The failure mode is a console error with no obvious connection to the header change. Existing unit tests (`build_headers_preserves_content_type`) and integration tests (`test_external_module_script`) would catch this regression, but only if they are run. A code-level comment on `STRIPPED_RESPONSE_HEADERS` explicitly noting that `content-type` must NOT be added would provide defense in depth.

**Connected systems and variables**:
- The browser enforces MIME type for `<script type="module">` (strict MIME type checking per HTML spec). Without `content-type: text/javascript`, modules fail with a MIME type error.
- The denylist is the only mechanism protecting `content-type`. There is no allowlist, no assertion at the builder call site, and no runtime check.
- The unit test `build_headers_preserves_content_type` is the sole regression guard. If someone deletes or skips tests, the protection vanishes.
- Non-module scripts would still load (browsers are lenient about `content-type` for classic scripts), making the regression intermittent and hard to diagnose.
- The `test_external_module_script` integration test would catch this, but only if tests are run.

**Mitigation assessment**: The unit test provides adequate protection for normal development. The risk is low but not zero. A `debug_assert!` or compile-time check would be stronger, but given Bombadil's testing-tool context and the existing test coverage, the current approach is acceptable.

---

## Problem 3: CSP sanitization does not handle multiple CSP headers as a merged policy

**Description**: HTTP allows multiple `Content-Security-Policy` headers on a single response, and browsers intersect their policies. `sanitize_csp` processes a single header value. If the server sends two CSP headers, `build_response_headers` processes each independently via `flat_map`.

**Origin**: Direct result of the new code.

**Can it be disregarded?** Yes. The `flat_map` iteration over `response_headers` naturally handles multiple CSP headers independently. Browser intersection semantics mean that sanitizing each independently is correct — removing hashes from each header separately produces the same result as removing them from the intersection.

---

## Problem 4: CSP sanitization is string-based, not fully spec-compliant

**Description**: The CSP parser splits on `;` and whitespace. It does not use a formal grammar parser.

**Origin**: Direct result of the new code.

**Can it be disregarded?** Yes. CSP directive values are defined as space-separated tokens in the CSP spec (W3C CSP Level 3, Section 2.2). The semicolon-and-whitespace splitting approach matches the actual grammar. CSP values do not contain quoted strings or URL-encoding — they use single-quoted keywords (`'self'`, `'unsafe-inline'`, `'sha256-...'`). The approach is correct for the specification as written. A formal parser dependency would add complexity without practical benefit.

---

## Problem 5: script-src-attr is not handled

**Description**: `sanitize_csp` strips hashes from `script-src` and `script-src-elem` but not `script-src-attr`. If a page uses `script-src-attr` with hashes, those hashes will be forwarded unchanged.

**Origin**: Direct result of the new code.

**Can it be disregarded?** Yes. `script-src-attr` governs inline event handler attributes (`onclick`, `onload`, etc.), not `<script>` element sources. Bombadil's instrumentation modifies script bodies, not event handler attributes. Hash values in `script-src-attr` remain valid after instrumentation, so forwarding them unchanged is correct behavior.

---

## Problem 6: Stripping report-uri/report-to is aggressive

**Description**: All `report-uri` and `report-to` directives are removed from sanitized CSP headers, even though only script-hash violations would be false positives.

**Origin**: Direct result of the new code.

**Can it be disregarded?** Yes. During testing, instrumentation can trigger many kinds of mutations (DOM changes, network timing differences, different script execution order) that could cause CSP violations beyond just hash mismatches. Suppressing all CSP reporting during instrumentation prevents noise in the application's violation logs. The alternative (selectively suppressing only hash-related reports) is not possible because CSP reporting does not distinguish violation causes at the directive level.

---

## Problem 7: No handling of Strict-Transport-Security (HSTS) interaction

**Description**: HSTS headers are forwarded unchanged. If the test server runs on HTTP, the browser might cache an HSTS policy and refuse subsequent HTTP connections.

**Origin**: Not a direct result of the new code — the original code accidentally prevented this by dropping all headers.

**Can it be disregarded?** Yes. Bombadil uses a fresh browser profile (via `user_data_directory` in `TempDir`) for each test run, so HSTS state does not persist across runs. Within a single run, HSTS would only apply if the test server used HTTPS, which the local axum test server does not. In production testing against real HTTPS origins, HSTS forwarding is correct behavior.

---

## Problem 8: Digest header stripping may not cover all variants

**Description**: Only `digest` is stripped. RFC 9530 introduces `repr-digest` and `content-digest` as successors. These are not in the denylist.

**Origin**: Direct result of the new code.

**Can it be disregarded?** Yes. RFC 9530 headers are extremely rare in practice (the RFC was published in 2024 and adoption is negligible). The `digest` header covers the legacy RFC 3230 case. If `repr-digest` or `content-digest` become common, adding them to the denylist is a one-line change. The failure mode (a service worker rejecting an instrumented script) would be immediately visible and easy to diagnose.

---

## Problem 9: The denylist uses ASCII-only case matching

**Description**: `STRIPPED_RESPONSE_HEADERS` entries are lowercase and compared with `eq_ignore_ascii_case`. HTTP headers are defined as ASCII tokens (RFC 9110, Section 5.1), so this is correct.

**Origin**: Direct result of the new code.

**Can it be disregarded?** Yes. The HTTP specification requires header field names to be ASCII. `eq_ignore_ascii_case` is the correct comparison function. Non-ASCII header names would be a protocol violation by the server.

---

## Problem 10: build_response_headers always appends etag even for documents

**Description**: The synthetic `etag` (derived from `source_id`) is appended to every response, including documents. This could cause caching issues if a proxy uses the etag.

**Origin**: Partially from the original code (which also set etag) and partially from the new code (which extends it to all resource types).

**Can it be disregarded?** Yes. The original code already set a synthetic etag on every fulfilled response. The etag is needed for instrumentation source ID tracking. Caching proxies between CDP and the page are not a realistic scenario — CDP operates at the browser level.

---

## Problem 11: resource_type clone may be unnecessary

**Description**: `event.resource_type.clone()` is called before `build_response_headers`. The comment says "before the iterator borrows event" but `resource_type` is a small enum.

**Origin**: Direct result of the new code.

**Can it be disregarded?** Yes. `network::ResourceType` is a small enum. The clone is at worst a no-op copy. This is a code style issue, not a security or correctness issue. The Rust compiler would reject the code if the borrow were actually invalid.

---

## Problem 12: CSP sanitization strips strict-dynamic even when other trust anchors remain

**Description**: `'strict-dynamic'` is removed whenever hashes/nonces are stripped. But if the directive also contains `'unsafe-inline'` or host sources, `'strict-dynamic'` would override them in a real browser and its removal changes the effective policy.

**Origin**: Direct result of the new code.

**Can it be disregarded?** Yes, for the instrumentation use case. When Bombadil removes script hashes (because instrumentation invalidates them), `'strict-dynamic'` without a trust anchor (hash or nonce) would block all dynamically inserted scripts. Removing it allows `'unsafe-inline'` or host sources to take effect, which is more permissive than the original policy — but the alternative is all scripts being blocked, making testing impossible. The goal of CSP sanitization is to allow instrumented scripts to run while preserving as much of the original policy as possible.

---

## Problem 13: No handling of CSP sandbox directive

**Description**: The CSP `sandbox` directive can restrict script execution. `sanitize_csp` does not modify it.

**Origin**: Direct result of the new code.

**Can it be disregarded?** Yes. If a page uses `sandbox` without `allow-scripts`, scripts cannot run regardless of `script-src`. Modifying `sandbox` would fundamentally change the page's security boundary, which is beyond the scope of script instrumentation. The correct behavior is to forward it unchanged.

---

## Problem 14: Compression layer test only covers gzip

**Description**: The `test_compressed_script` integration test uses `CompressionLayer::new()` which defaults to gzip. Brotli and zstd are not tested.

**Origin**: Direct result of the new code.

**Can it be disregarded?** Yes. The `content-encoding` header is stripped regardless of the compression algorithm. CDP decompresses the body before handing it to the interception handler regardless of algorithm. The specific algorithm is irrelevant to the logic being tested.

---

## Problem 15: test_csp_script uses a hardcoded hash that doesn't match the script

**Description**: The CSP header uses a sha256 hash that may not match `shared/script.js`. This is intentional — the test verifies that Bombadil strips the hash, allowing the script to load despite the mismatch.

**Origin**: Direct result of the new code.

**Can it be disregarded?** Yes. Using a non-matching hash makes the test meaningful: if Bombadil fails to strip the CSP, the browser blocks the script and the test fails.

---

## Problem 16: make_csp_router requires 'static lifetime for CSP string

**Description**: `make_csp_router` takes `csp: &'static str`, which is restrictive.

**Origin**: Direct result of the new code.

**Can it be disregarded?** Yes. This is test-only code. All callers use string literals. The `'static` requirement is a natural consequence of the axum middleware closure.

---

## Problem 17: New dev-dependency feature increases build surface

**Description**: Adding `compression-gzip` to `tower-http` dev-dependencies pulls in `async-compression`, `compression-codecs`, `compression-core`, and `flate2`.

**Origin**: Direct result of the new code.

**Can it be disregarded?** Yes. These are dev-dependencies only — they do not affect the production binary. The crates are well-maintained (tower-http ecosystem).

---

## Summary

| # | Problem | Origin | Disregardable? | Risk Level |
|---|---------|--------|----------------|------------|
| 1 | Denylist completeness | New code | Yes | Low |
| 2 | Implicit content-type preservation | New code | **No** | Medium |
| 3 | Multiple CSP headers | New code | Yes | None |
| 4 | String-based CSP parsing | New code | Yes | None |
| 5 | script-src-attr not handled | New code | Yes | None |
| 6 | Aggressive report stripping | New code | Yes | Low |
| 7 | HSTS interaction | Pre-existing | Yes | None |
| 8 | Digest variants | New code | Yes | Low |
| 9 | ASCII case matching | New code | Yes | None |
| 10 | Etag on documents | Both | Yes | None |
| 11 | Unnecessary clone | New code | Yes | None |
| 12 | strict-dynamic removal | New code | Yes | Low |
| 13 | sandbox directive | New code | Yes | None |
| 14 | Only gzip tested | New code | Yes | None |
| 15 | Hardcoded CSP hash | New code | Yes | None |
| 16 | 'static lifetime | New code | Yes | None |
| 17 | New dev-dependencies | New code | Yes | None |

**One non-disregardable problem identified (Problem 2).** It is adequately mitigated by existing unit and integration tests, but the implicit nature of the protection warrants awareness during future maintenance.
