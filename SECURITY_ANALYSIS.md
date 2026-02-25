# Security Analysis: Fundamental Problems and Risky Assumptions

This document evaluates every identified problem and assumption in the code added on `develop` relative to `antithesishq/main`, determines which are direct results of the new code, which can be disregarded based on existing project patterns, and what consequences remain for those that cannot.

---

## Problem 1: Denylist approach assumes completeness

**Description**: `STRIPPED_RESPONSE_HEADERS` is a fixed list of 5 headers. Any header that should be stripped but is not listed will be forwarded with a stale value after body instrumentation.

**Origin**: Direct result of the new code — the original code had no denylist because it dropped everything.

**Can it be disregarded?** Yes. The alternative (allowlist) would require enumerating every possible valid header, which is impractical and fragile. The denylist approach is the standard pattern for HTTP header proxying (nginx, Varnish, CDN proxies all use denylist-style header filtering). The five chosen headers exactly match the set invalidated by body rewriting. The previous behavior of dropping all headers was strictly worse because it silently broke CORS, HSTS, CSP, and all other security headers.

---

## Problem 2: content-type preservation is implicit

**Description**: The `content-type` header — critical for ES module MIME type enforcement — is preserved only because it is absent from `STRIPPED_RESPONSE_HEADERS`. There is no explicit positive protection in the production code path.

**Origin**: Direct result of the new code's denylist design.

**Can it be disregarded?** No. This is the one problem that warrants ongoing attention.

**Consequence analysis**: If a future maintainer adds `content-type` to `STRIPPED_RESPONSE_HEADERS` (e.g., to fix a perceived issue with stale MIME types after instrumentation), all `<script type="module">` tags loading external scripts would silently break — browsers require a JavaScript MIME type for module scripts and reject responses without one. The failure mode is a console error with no obvious connection to the header change.

**Connected systems and variables**:
- The HTML specification mandates strict MIME type checking for module scripts. Without `content-type: text/javascript` (or equivalent), the browser rejects the response with a MIME type error. This is the exact bug that motivated the entire change.
- Classic `<script>` tags are lenient about missing content-type, so a regression would only affect module scripts — making it intermittent and hard to diagnose across different applications.
- The denylist is the sole mechanism protecting `content-type`. There is no allowlist, no assertion at the `FulfillRequestParams` builder call site, and no runtime check that content-type is present in the output.
- The unit test `build_headers_preserves_content_type` is the primary regression guard. The integration test `test_external_module_script` is the secondary guard. If tests are skipped or deleted, the protection vanishes entirely.
- CDP's `Fetch.fulfillRequest` uses replacement semantics: providing `responseHeaders` replaces the entire original header set. Omitting content-type is equivalent to actively removing it — there is no fallback to the original headers.
- Any web framework that relies on content-type for CORS preflight, content negotiation, or security decisions (e.g., `X-Content-Type-Options: nosniff`) would be affected by its removal.

**Mitigation assessment**: The unit and integration tests provide adequate protection for normal development workflows. A code-level comment on `STRIPPED_RESPONSE_HEADERS` explicitly noting that `content-type` must NOT be added would provide defense in depth. The risk is low but not zero.

---

## Problem 3: CSP sanitization does not merge multiple CSP headers

**Description**: HTTP allows multiple `Content-Security-Policy` headers on a single response, and browsers intersect their policies. `sanitize_csp` processes a single header value. If the server sends two CSP headers, `build_response_headers` processes each independently via `flat_map`.

**Origin**: Direct result of the new code.

**Can it be disregarded?** Yes. Processing each CSP header independently and then passing them all to the browser is correct because the browser will intersect them. Removing hashes from each header separately produces the same effective policy as removing them from a merged policy then splitting back. The intersection of (A minus hashes) and (B minus hashes) equals (A intersect B) minus hashes.

---

## Problem 4: CSP sanitization is string-based, not formally parsed

**Description**: The CSP parser splits on `;` and whitespace rather than using a formal grammar parser or CSP parsing library.

**Origin**: Direct result of the new code.

**Can it be disregarded?** Yes. CSP directive values are defined as space-separated tokens (W3C CSP Level 3, Section 2.2). The semicolon-and-whitespace splitting exactly matches the actual grammar. CSP values do not contain quoted strings, URL-encoding, or escape sequences — they use single-quoted keywords (`'self'`, `'unsafe-inline'`, `'sha256-...'`). A formal parser dependency would add complexity without practical benefit.

---

## Problem 5: script-src-attr is not handled

**Description**: `sanitize_csp` strips hashes from `script-src` and `script-src-elem` but not `script-src-attr`. If a page uses `script-src-attr` with hashes, those hashes will be forwarded unchanged.

**Origin**: Direct result of the new code.

**Can it be disregarded?** Yes. `script-src-attr` governs inline event handler attributes (`onclick`, `onload`, etc.), not `<script>` element sources or external script files. Bombadil's instrumentation modifies script bodies, not event handler attributes. Hash values in `script-src-attr` remain valid after instrumentation, so forwarding them unchanged is correct behavior.

---

## Problem 6: Stripping report-uri/report-to is aggressive

**Description**: All `report-uri` and `report-to` directives are removed from sanitized CSP headers, even though only script-hash violations would be instrumentation-induced false positives.

**Origin**: Direct result of the new code.

**Can it be disregarded?** Yes. During testing, instrumentation can trigger many kinds of mutations beyond hash mismatches (DOM changes, network timing differences, different script execution order, injected coverage code triggering CSP violations). Suppressing all CSP reporting during instrumentation prevents noise in the application's violation logs. The alternative (selectively suppressing only hash-related reports) is not possible because CSP reporting does not distinguish violation causes at the granularity needed.

---

## Problem 7: No handling of HSTS interaction

**Description**: HSTS headers are forwarded unchanged. If the test server runs on HTTP, the browser might cache an HSTS policy and refuse subsequent HTTP connections to the same host.

**Origin**: Not a direct result of the new code — the original code accidentally prevented this by dropping all headers.

**Can it be disregarded?** Yes. Bombadil creates a fresh browser profile (via `user_data_directory` in `TempDir`) for each test run, so HSTS state does not persist across runs. Within a single run, HSTS would only affect subsequent requests to the same host. In local testing (HTTP), servers don't send HSTS. In production testing (HTTPS), forwarding HSTS is correct. The new code is strictly more correct than the old code (which dropped HSTS along with everything else).

---

## Problem 8: Digest header stripping may not cover all variants

**Description**: Only `digest` (RFC 3230) is stripped. RFC 9530 introduces `repr-digest` and `content-digest` as successors. These are not in the denylist.

**Origin**: Direct result of the new code.

**Can it be disregarded?** Yes. RFC 9530 headers are extremely rare in practice (the RFC was finalized in 2024 and adoption is negligible). The `digest` header covers the legacy case. If `repr-digest` or `content-digest` become common, adding them is a one-line change. The failure mode (a service worker validating the digest would reject the response) would be immediately visible and easy to diagnose.

---

## Problem 9: The denylist uses ASCII-only case matching

**Description**: `STRIPPED_RESPONSE_HEADERS` entries are lowercase and compared with `eq_ignore_ascii_case`. If a server sends non-ASCII header names, they would not match.

**Origin**: Direct result of the new code.

**Can it be disregarded?** Yes. HTTP header field names are defined as ASCII tokens (RFC 9110, Section 5.1). Non-ASCII header names are a protocol violation by the server. `eq_ignore_ascii_case` is the correct comparison function per the HTTP specification.

---

## Problem 10: build_response_headers always appends etag even for documents

**Description**: The synthetic `etag` (derived from `source_id`) is appended to every fulfilled response, including HTML documents. This could cause caching issues if a proxy uses the etag for conditional requests.

**Origin**: Partially from the original code (which also set a synthetic etag on every response) and partially from the new code (which preserves this behavior).

**Can it be disregarded?** Yes. The original code already set a synthetic etag on every fulfilled response. The etag is used for instrumentation source ID tracking. Caching proxies between CDP and the page are not a realistic scenario — CDP operates within the browser process, not over a network link.

---

## Problem 11: resource_type clone before builder

**Description**: `event.resource_type.clone()` is called before the builder to avoid a borrow conflict. The comment says "before the iterator borrows event" but `resource_type` is a small Copy-like enum.

**Origin**: Direct result of the new code.

**Can it be disregarded?** Yes. `network::ResourceType` is a small enum. Cloning it is at worst a trivial copy. The clone exists because the Rust borrow checker requires it when `event.response_headers` is also borrowed in the same expression. This is a Rust-specific ergonomic concern, not a correctness or security issue.

---

## Problem 12: strict-dynamic removal widens the effective CSP

**Description**: When hashes/nonces are stripped from a directive, `'strict-dynamic'` is also removed. In the presence of `'strict-dynamic'`, `'unsafe-inline'` and host-based sources are normally ignored by the browser. Removing `'strict-dynamic'` makes them active again, widening the effective policy.

**Origin**: Direct result of the new code.

**Can it be disregarded?** Yes, for the instrumentation use case. When Bombadil removes script hashes (because instrumentation invalidates them), `'strict-dynamic'` without a trust anchor (hash or nonce) would block all dynamically inserted scripts — making testing impossible. Removing it allows `'unsafe-inline'` or host sources to take effect, which is more permissive than the original policy. But the alternative is all scripts being blocked. The goal of CSP sanitization is to allow instrumented scripts to run while preserving as much of the original policy as practical.

---

## Problem 13: No handling of CSP sandbox directive

**Description**: The CSP `sandbox` directive can restrict script execution. `sanitize_csp` does not modify it.

**Origin**: Direct result of the new code.

**Can it be disregarded?** Yes. If a page uses `sandbox` without `allow-scripts`, scripts cannot run regardless of `script-src`. Modifying `sandbox` would fundamentally change the page's security boundary, which is beyond the scope of script instrumentation. Forwarding it unchanged is the correct, conservative behavior.

---

## Problem 14: Compression test only covers gzip

**Description**: The `test_compressed_script` integration test uses `CompressionLayer::new()` which defaults to gzip. Brotli and zstd are not tested.

**Origin**: Direct result of the new code.

**Can it be disregarded?** Yes. The logic being tested is `content-encoding` header stripping, which is independent of the specific compression algorithm. CDP decompresses the response body before handing it to the Fetch interception handler regardless of the algorithm used. The specific algorithm is irrelevant to correctness.

---

## Problem 15: test_csp_script uses a hardcoded hash that doesn't match the actual script

**Description**: The CSP header in `test_csp_script` uses a sha256 hash (`sRoPO3cq...`) that does not match the actual content of `shared/script.js`.

**Origin**: Direct result of the new code.

**Can it be disregarded?** Yes, this is intentional. Using a non-matching hash makes the test meaningful: without Bombadil's CSP stripping, the browser would block the script (hash mismatch) and the test would fail. The test verifies that Bombadil correctly strips the hash, allowing the script to load despite the CSP mismatch.

---

## Problem 16: make_csp_router requires 'static lifetime for CSP string

**Description**: `make_csp_router` takes `csp: &'static str`, which prevents runtime-generated CSP values.

**Origin**: Direct result of the new code.

**Can it be disregarded?** Yes. This is test-only code. All callers use string literals, which are inherently `'static`. The `'static` requirement is a natural consequence of the axum middleware closure capturing the reference across an async boundary.

---

## Problem 17: New dev-dependency feature increases build surface

**Description**: Adding `compression-gzip` to `tower-http` dev-dependencies pulls in `async-compression`, `compression-codecs`, `compression-core`, and `flate2`.

**Origin**: Direct result of the new code.

**Can it be disregarded?** Yes. These are dev-dependencies only — they do not affect the production binary size, attack surface, or runtime behavior. The crates are well-maintained parts of the tower-http ecosystem.

---

## Problem 18: The `_` wildcard in CSP resource-type match is currently unreachable

**Description**: In `build_response_headers`, the CSP match has `Script`, `Document`, and `_ => Some(h.clone())`. But the Fetch interception only registers for Script and Document resource types, so the `_` arm is effectively dead code.

**Origin**: Direct result of the new code.

**Can it be disregarded?** Yes. The `_` arm acts as a safe default: if someone later adds more resource types to the Fetch interception, CSP would pass through unchanged for those types. Since instrumentation only modifies Script and Document bodies, forwarding CSP unchanged for other types is the correct conservative behavior. However, the arm lacks a comment explaining why the default is safe, which is a deviation from the project's pattern matching conventions.

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
| 18 | Unreachable `_` arm | New code | Yes | Low |

**One non-disregardable problem identified (Problem 2).** It is adequately mitigated by existing unit and integration tests, but the implicit nature of the protection warrants awareness during future maintenance. A code-level comment explicitly noting that `content-type` must not be added to the denylist would strengthen the defense.
