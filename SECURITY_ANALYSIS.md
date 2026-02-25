# Full Security Analysis: Header Forwarding in `Fetch.fulfillRequest`

## Background

`src/browser/instrumentation.rs` intercepts script and document responses via CDP's Fetch domain to inject AFL-style edge coverage instrumentation. The upstream (`antithesishq/main`) sent only a synthetic `etag` header with `Fetch.fulfillRequest`, dropping all original headers (with a `// TODO: forward headers` comment). The fork (`main`) changed this to forward all original response headers (filtering out only `etag`, which is replaced with the source ID).

This analysis enumerates every problematic assumption in the header forwarding implementation, classifies each as either introduced by the new code or pre-existing, and determines which can be disregarded given the codebase's established patterns.

---

## 1. `Content-Length` is forwarded but the body has changed

**Direct result of the new code.** antithesishq sent no headers, so `Content-Length` was never forwarded.

The original `Content-Length` reflects the pre-instrumentation body size. The instrumented body is significantly larger (coverage hooks injected at every branch point). Chrome receives a `Content-Length` that doesn't match the actual body. Depending on Chrome's undocumented internal handling of `fulfillRequest`, this could cause truncated script loads (Chrome stops reading at the declared length, cutting off instrumented code mid-statement), stalled requests (Chrome waits for bytes that never arrive), or be silently ignored. Behavior may vary across Chrome versions.

**Verdict: genuine problem. Must be fixed.**

---

## 2. `Content-Encoding` is forwarded but the body is decoded

**Direct result of the new code.** antithesishq never forwarded `Content-Encoding`.

CDP's `GetResponseBody` returns the **decoded** body — gzip/br/deflate decompression has already happened. The instrumentation modifies this decoded body and sends it back as plaintext via `fulfillRequest`. But the original `Content-Encoding: gzip` (or `br`, `deflate`) header is forwarded, telling Chrome the body is still compressed. Chrome would attempt to decompress plaintext JavaScript, producing garbage or errors. This affects any server that uses compression, which is essentially all production servers.

**Verdict: genuine problem. Must be fixed.**

---

## 3. `Transfer-Encoding: chunked` is forwarded but `fulfillRequest` sends a single block

**Direct result of the new code.** antithesishq never forwarded `Transfer-Encoding`.

`fulfillRequest` delivers the body as a single base64 blob. Forwarding `Transfer-Encoding: chunked` tells Chrome to expect chunked encoding framing that doesn't exist. This is a framing mismatch of the same category as #2.

**Verdict: genuine problem. Must be fixed.**

---

## 4. Caching headers are forwarded and could cause stale instrumented content

**Direct result of the new code.** antithesishq only sent a synthetic `etag`.

Headers like `Cache-Control`, `Last-Modified`, `Expires` are now forwarded. While `etag` is replaced with the source ID, other caching headers survive.

**Verdict: disregard.** Every request is intercepted at the response stage via `Fetch.RequestPaused`. The interception happens at the network layer, before Chrome's HTTP cache. Since Bombadil intercepts every single script/document response, the cached vs. uncached distinction is irrelevant — the response never reaches Chrome's cache unmodified. The `etag` replacement (which antithesishq already did) is the only caching mechanism the codebase concerns itself with.

---

## 5. `response_headers` can be `None`

**Pre-existing in antithesishq/main.** Not introduced by the new code.

`event.response_headers` is `Option<Vec<HeaderEntry>>`. The code does `.iter().flatten()` on the `Option`, so if it's `None`, no headers are forwarded — only the synthetic `etag`. This means no `Content-Type` header is sent, which is the original MIME type bug. The fix only works when `response_headers` is `Some`.

While CDP should always populate `response_headers` at the response stage, there's no guarantee. A `None` here silently reproduces the original bug with no warning or logging. However, this is the same behavior as antithesishq/main (which also sent no `Content-Type` in all cases), so the new code is no worse.

**Verdict: disregard.** Pre-existing behavior, not a regression.

---

## 6. Security-sensitive headers are forwarded blindly

**Direct result of the new code.** antithesishq dropped all headers.

Headers like `Set-Cookie`, `Content-Security-Policy`, `X-Frame-Options`, `Strict-Transport-Security`, and CORS headers (`Access-Control-*`) are now forwarded. The main concern is `Content-Security-Policy`: antithesishq effectively disabled CSP by stripping all headers, and now a strict CSP could theoretically interfere.

**Verdict: disregard.** The instrumentation injects code *inside* the script body itself (inline `__bombadil__` references at branch points). It doesn't add a new `<script>` tag or load a separate file. CSP's `script-src` controls which scripts are allowed to execute, not what code runs inside an already-allowed script. Since the instrumented script is served from the same origin via `fulfillRequest`, CSP permits it. The other security headers (`Set-Cookie`, CORS, `X-Frame-Options`) are actually more correct to forward than to drop — antithesishq was silently breaking cookie-dependent and CORS-dependent apps by stripping all headers.

---

## 7. The non-200 handler uses `ContinueRequest` instead of `ContinueResponse`

**Pre-existing in antithesishq/main.** The new code did not touch this path.

Lines 44–62: when the upstream response status is non-200, the code calls `ContinueRequestParams`. But the interception is at the response stage (`RequestStage::Response`). At the response stage, `Fetch.continueRequest` re-issues the request from scratch, meaning the server gets hit twice for non-200 responses (redirects, 404s, etc.). The correct call at the response stage would be `Fetch.continueResponse` or `fulfillRequest` with the original body. This could cause double requests, race conditions, or redirect loops.

**Verdict: disregard for this analysis.** Pre-existing issue unrelated to the header forwarding change.

---

## 8. Multiple headers with the same name are collapsed by `HashMap`

**Pre-existing in antithesishq/main.** The new code did not touch this path.

HTTP allows multiple headers with the same name (e.g., multiple `Set-Cookie` headers). The `source_id` function at line 229 reads request headers into a `HashMap<String, String>`, which silently drops duplicate header names (last one wins). If a server sends multiple `etag`-like headers or multiple values for the same request header, this could produce incorrect source IDs.

Note: the new header forwarding code (`.filter().cloned().chain()`) does correctly preserve multiple headers with the same name in the response — this problem is isolated to the `source_id` function.

**Verdict: disregard for this analysis.** Pre-existing issue unrelated to the header forwarding change.

---

## 9. The test only covers the happy path

**Direct result of the new code** (the test is new).

The test uses a trivial local server (`ServeDir`) that never sends `Content-Encoding` (no compression), always sends correct `Content-Type`, doesn't send a `Content-Length` that would conflict, and doesn't use chunked transfer encoding. The test proves the fix works for the simplest case but doesn't cover any of the transport-level header mismatches (problems 1–3).

**Verdict: disregard.** Every integration test in antithesishq/main uses the identical pattern: `ServeDir` on localhost, no compression, no complex headers. The new test is consistent with the existing test conventions and adequately proves the specific fix (MIME type forwarding for module scripts). Testing against gzip/chunked scenarios would be a separate concern, not a regression in test quality.

---

## 10. Hardcoded `response_code(200)` masks upstream status

**Pre-existing in antithesishq/main.** The new code did not touch this.

Line 162: the fulfilled request always returns status 200, regardless of the original response status. Combined with the non-200 bypass (lines 44–62), only 200 responses are instrumented, which is correct. But if the status-code check were ever removed or a 200-equivalent status (like 203 or 206) were returned by the server, it would be silently rewritten to 200.

**Verdict: disregard for this analysis.** Pre-existing issue unrelated to the header forwarding change.

---

## 11. `etag` case sensitivity asymmetry between request and response headers

**Partially new.** The case-insensitive response filter is new code. The case-sensitive `HashMap::get("etag")` request lookup is pre-existing.

The response header filter uses `.eq_ignore_ascii_case("etag")` (correct), but the `source_id` function uses `headers.get("etag")` (case-sensitive) on request headers. A server sending `ETag` (capitalized) in the response would have it stripped, but the corresponding request header `ETag` would not be found by `HashMap::get("etag")`, causing a fallback to body hashing even when an etag exists.

**Verdict: disregard for this analysis.** Pre-existing issue in the `source_id` function. The new code's case-insensitive filter is the correct approach — it's the old code that's inconsistent.

---

## Summary

| # | Problem | Introduced by new code? | Verdict |
|---|---|---|---|
| 1 | `Content-Length` mismatch | Yes | **Must fix** |
| 2 | `Content-Encoding` mismatch | Yes | **Must fix** |
| 3 | `Transfer-Encoding` mismatch | Yes | **Must fix** |
| 4 | Caching headers forwarded | Yes | Disregard — interception bypasses cache |
| 5 | `response_headers` can be `None` | No (pre-existing) | Disregard — same behavior as before |
| 6 | Security headers forwarded | Yes | Disregard — CSP doesn't block inline instrumentation; other headers are more correct forwarded |
| 7 | `ContinueRequest` vs `ContinueResponse` | No (pre-existing) | Disregard — unrelated to this change |
| 8 | `HashMap` drops duplicate headers | No (pre-existing) | Disregard — unrelated to this change |
| 9 | Test only covers happy path | Yes | Disregard — follows established test conventions |
| 10 | Hardcoded `response_code(200)` | No (pre-existing) | Disregard — unrelated to this change |
| 11 | `etag` case sensitivity asymmetry | Partially | Disregard — pre-existing in `source_id`; new code is correct |

### Root Cause

Problems 1, 2, and 3 share the same root cause: the header forwarding blindly copies **transport-level** headers that describe properties of the original wire-format body. After CDP decodes, Bombadil transforms, and `fulfillRequest` re-encodes the body, these headers become semantically wrong.

### Fix

The `response_headers` filter chain should strip these headers alongside `etag` (all case-insensitive):

- `content-length`
- `content-encoding`
- `transfer-encoding`
