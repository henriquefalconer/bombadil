# Fundamental Problems and Risky Assumptions

## Problem 1: CSP Header Fully Stripped from Document Responses

**Category:** Direct result of the fork's code.

**What happens:** The `STRIPPED_RESPONSE_HEADERS` denylist includes
`content-security-policy` and `content-security-policy-report-only`. The
filtering code applies identically to Script and Document resource types.
For scripts, only `script-src` hash directives are relevant, and stripping the
whole header is a valid (if coarse) fix. For documents, the CSP header defines
the **entire page security policy** — not just script hashes.

**Connected systems and variables:**

- CDP `Fetch.fulfillRequest` uses replacement semantics: providing
  `responseHeaders` replaces the entire original header set. Omitting a header
  is equivalent to removing it.
- `instrument_js_coverage` intercepts both `ResourceType::Script` and
  `ResourceType::Document`. The `FulfillRequestParams` builder call (lines
  183–212 of `instrumentation.rs`) sits after the `body_instrumented` branch,
  meaning all fulfilled responses — scripts, HTML documents, and even
  pass-through non-HTML documents — go through the same header filter.
- CSP directives affected by full stripping: `script-src`, `style-src`,
  `img-src`, `font-src`, `connect-src`, `frame-ancestors`, `frame-src`,
  `base-uri`, `form-action`, `navigate-to`, `media-src`, `object-src`,
  `worker-src`, `manifest-src`, `default-src`, `sandbox`, `report-uri`,
  `report-to`, `upgrade-insecure-requests`.
- The Bombadil runner cycles through actions (click, type, scroll, navigate) and
  checks LTL properties against `BrowserState` snapshots. If CSP is absent, the
  browser allows requests and loads that would be blocked in production. This
  means:
  - Properties like `noHttpErrorCodes` may see fewer errors (CSP-blocked loads
    never fire network errors).
  - Properties checking DOM content may see elements (iframes, images, styles)
    that CSP would prevent from loading.
  - Coverage data reflects code paths reachable only without CSP.

**Consequences of deploying:**

- Bombadil produces a **less faithful** simulation of the target application.
- Bugs that only manifest under CSP enforcement (e.g., a feature that relies on
  an inline style which CSP blocks) will be missed.
- Bugs that CSP prevents (e.g., XSS via inline script injection) will appear as
  reachable paths in fuzz coverage, wasting exploration budget.
- Any user relying on Bombadil to validate that their app works correctly
  **with** CSP in place will get false confidence.

**Disposition:** Cannot be disregarded. The fix is to either (a) parse CSP and
selectively remove only hash/nonce directives from `script-src`, or (b) only
strip CSP from Script responses, not Document responses.

---

## Problem 2: HSTS Stripping Is Unconditional

**Category:** Direct result of the fork's code.

**What happens:** `strict-transport-security` is in the denylist. The comment
says "Prevent HSTS pinning on ephemeral localhost test sessions" but the code
does not check whether the origin is localhost or whether the protocol is HTTPS.

**Connected systems and variables:**

- Bombadil can fuzz any origin via `bombadil test <origin>` or
  `bombadil test-external <origin>`. The origin can be `https://...`.
- Browsers learn HSTS from the first response with the header and enforce it for
  subsequent requests to the same domain within the session.
- Bombadil uses ephemeral browser profiles (`TempDir`), so HSTS does not persist
  across runs. But within a single run, navigations triggered by `BrowserAction`
  (Reload, Back, Forward, Click on links) can visit the same domain multiple
  times.
- If a link on the page points to `http://sub.example.com` and the main domain
  sent HSTS with `includeSubDomains`, the browser would normally auto-upgrade to
  HTTPS. Without HSTS, the browser follows the HTTP link, observing different
  behavior.

**Consequences of deploying:**

- When fuzzing HTTPS targets, the browser's intra-run security state is weaker
  than a real user's browser.
- Mixed-content and downgrade scenarios that HSTS would prevent become reachable,
  creating false positives in coverage and potentially in property violations.
- For localhost targets (the common case), this is harmless and even beneficial.

**Disposition:** Low severity. Can be partially disregarded because Bombadil's
primary use case is ephemeral test environments on localhost. However, for
correctness on HTTPS targets, the stripping should be conditional on the origin
scheme or host. A simple improvement: only strip HSTS when the origin is
`http://` or `localhost`.

---

## Problem 3: Denylist Potentially Incomplete

**Category:** Partially a direct result of the fork's code.

**What happens:** The denylist contains 7 headers. Other headers whose validity
depends on body content are not included:

| Header | Situation |
|--------|-----------|
| `digest` (RFC 3230/9530) | Body hash. After instrumentation, the hash is wrong. Rare in practice. |
| `age` | Cache age. Stale for the instrumented response. |
| `last-modified` | Stale timestamp for instrumented content. |
| `expires` | Stale expiry for instrumented content. |
| `accept-ranges` | Byte-range semantics change with body modification. |

**Connected systems and variables:**

- Bombadil uses ephemeral browser profiles, so cache headers are unlikely to
  cause cross-session problems.
- The `etag` is already replaced with a source-ID-based value, which signals
  cache invalidation to the browser.
- `digest` is extremely rare in practice but is the most semantically dangerous
  omission — if a service worker validates it, the script will be rejected.

**Consequences of deploying:**

- In the vast majority of real-world targets, no impact.
- Edge case: a target using `digest` headers or service-worker-based integrity
  validation could silently reject instrumented scripts, causing Bombadil to see
  an uninstrumented (or error) state.

**Disposition:** Low severity. Can be largely disregarded for current use cases.
Worth documenting as known limitations.

---

## Problem 4: Non-HTML Document Bodies Unchanged but Headers Still Filtered

**Category:** Not a direct result of the fork's code. Upstream already dropped
all headers for these responses.

**What happens:** When `event.resource_type == ResourceType::Document` and the
response is not HTML (XML, PDF, etc.), the body is passed through as
`body.clone()` but headers still go through the denylist filter. This strips
`content-length`, `content-encoding`, and `transfer-encoding` from an unmodified
body.

**Connected systems and variables:**

- CDP `GetResponseBody` returns the decompressed body, so `content-encoding` is
  already stale regardless of body modification.
- `content-length` for the re-encoded (base64) body is recalculated by CDP.
- This is strictly better than upstream, which provided **no** headers at all for
  these responses.

**Disposition:** Disregarded. The fork improves on upstream behavior, and the
stale-header concern is already handled by CDP's own processing.

---

## Problem 5: CDP `response_headers` Field May Be `None`

**Category:** Not a direct result of the fork's code.

**What happens:** When `event.response_headers` is `None`, the iterator
`.iter().flatten()` produces an empty sequence, and only the synthetic `etag` is
emitted. This matches upstream behavior exactly (upstream always sent only etag).

**Disposition:** Disregarded. Pre-existing behavior, no regression.

---

## Problem 6: No Content-Type Injection for Scripts

**Category:** Not a direct result of the fork's code.

**What happens:** If the upstream response lacks a `Content-Type` header (or CDP
strips it), the fulfilled response also lacks it. Module scripts require
`application/javascript` MIME type. The fork forwards `Content-Type` when it
exists (it is not in the denylist), which is an improvement over upstream.

**Disposition:** Disregarded. Pre-existing behavior, and the fork improves on it
by forwarding `Content-Type` when available.

---

## Problem 7: No Logging When Headers Are Stripped

**Category:** Partially a direct result of the fork's code.

**What happens:** Security-relevant headers are silently removed. There is no
`log::debug!` or `log::trace!` message when a header is filtered out. The
existing code logs at `debug` level for successful instrumentation and at `warn`
level for failures, but the header filtering is invisible.

**Connected systems and variables:**

- The `log` crate is used throughout the codebase with consistent levels:
  `debug` for operational detail, `warn` for recoverable issues, `error` for
  critical failures.
- In the upstream code, there were no headers to log about (all were dropped).

**Consequences of deploying:**

- Debugging header-related issues (e.g., "why did CSP stop working?") requires
  reading the source code rather than checking logs.
- Not a correctness issue, but a debuggability gap.

**Disposition:** Minor. Not a fundamental problem, but adding `log::debug!` for
stripped headers would follow the codebase's existing logging patterns and aid
debugging.

---

## Summary Table

| # | Problem | Direct Result of Fork? | Can Disregard? | Severity |
|---|---------|----------------------|----------------|----------|
| 1 | CSP fully stripped from documents | Yes | No | High |
| 2 | HSTS stripping unconditional | Yes | Partially | Low |
| 3 | Denylist potentially incomplete | Partially | Mostly | Low |
| 4 | Unmodified bodies get header filtering | No | Yes | None |
| 5 | CDP headers may be None | No | Yes | None |
| 6 | No content-type injection | No | Yes | None |
| 7 | No logging of stripped headers | Partially | Mostly | Minor |
