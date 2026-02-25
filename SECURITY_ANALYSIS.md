# Security Analysis: Fundamental Problems and Risky Assumptions

This document covers every identified problem and assumption in the code added to `develop` (relative to `antithesishq/main`), the reasoning for whether each should be acted on, and the projected consequences of the ones that matter.

## Context

`antithesishq/main` dropped all response headers when fulfilling intercepted requests, adding only a synthetic `etag`. This was a known gap marked `// TODO: forward headers`. The `develop` branch replaces this with a header-forwarding pipeline that strips specific headers and applies resource-type-aware CSP sanitization. The analysis below evaluates the new code, not the pre-existing gap.

---

## Problem 1: `default-src` hash fallback is not handled

### Description

`sanitize_csp()` processes only `script-src` and `script-src-elem` directives. When neither is present in a CSP, browsers fall back to `default-src` for script loading decisions. If `default-src` contains hash values (e.g., `default-src 'sha256-...' 'self'`), those hashes are invalidated by script instrumentation but are NOT stripped by the current code.

### Why it cannot be disregarded

This is a direct consequence of the added code's design. The CSP spec fallback chain is not optional — it is how every browser evaluates CSP. A site using `default-src` with hashes instead of explicit `script-src` is a valid, real-world configuration.

### Connected systems and variables

- **CSP evaluation order**: The browser checks `script-src` → `script-src-elem` → `default-src`. If `script-src` is absent, `default-src` governs scripts.
- **Instrumentation scope**: Bombadil modifies script bodies (both external and inline), which invalidates any hash computed against the original body.
- **Where this applies**: Both Script and Document resource types. For Scripts, the entire CSP is currently stripped so this does not bite. For Documents, the CSP is sanitized — and the sanitization misses `default-src`.
- **Interaction with `sanitize_csp` returning `Some`**: If a Document CSP is `default-src 'sha256-abc' 'self'`, `sanitize_csp` returns `Some("default-src 'sha256-abc' 'self'")` unchanged. The browser uses the hash from `default-src` to evaluate inline scripts, which no longer match.

### Consequences if deployed

- Sites using `default-src` with hash values and no explicit `script-src` will have inline scripts blocked after Bombadil instruments the HTML document.
- The page appears broken during testing. Developers may misattribute the breakage to their application rather than to the testing tool.
- For a testing tool whose purpose is to faithfully exercise the application, silently breaking CSP-governed script loading undermines the tool's core value proposition.

---

## Problem 2: `strict-dynamic` becomes meaningless after nonce stripping

### Description

`sanitize_csp()` strips `'nonce-...'` values from `script-src`. However, `'strict-dynamic'` is designed to work with nonces: it says "trust this nonce-bearing script, and transitively trust everything it dynamically loads." Removing the nonce while preserving `'strict-dynamic'` leaves `'strict-dynamic'` without a trust anchor.

### Why it cannot be disregarded

`'strict-dynamic'` is the CSP Level 3 recommended pattern for complex applications (Google's CSP evaluator recommends it). The interaction between nonce removal and `'strict-dynamic'` creates a security model that differs from the original in unpredictable ways.

### Connected systems and variables

- **`'strict-dynamic'` semantics**: When present, the browser ignores source expressions (`'self'`, `https:`, host-based allowlists) and only allows scripts loaded by already-trusted scripts. The trust chain starts from nonce-bearing or hash-bearing scripts.
- **After nonce removal**: `script-src 'strict-dynamic'` with no nonce and no hash means no script has a trust anchor. Inline scripts without nonces are blocked. Parser-inserted `<script>` tags are blocked. `document.createElement('script')` may or may not be allowed depending on browser implementation.
- **`sanitize_csp` behavior**: Given `script-src 'nonce-abc' 'strict-dynamic'`, it strips the nonce and emits `script-src 'strict-dynamic'`. This is technically "preserving a non-hash value" per the filter logic, but semantically it creates a broken directive.

### Consequences if deployed

- Sites using `'strict-dynamic'` with nonces will have an unpredictable script loading experience during testing: some scripts load, others don't, depending on how they were inserted into the DOM.
- Test results do not reflect production behavior, defeating the purpose of the tool.
- Because `'strict-dynamic'` makes the browser ignore source lists, preserving `'self'` alongside `'strict-dynamic'` does not help — the browser ignores it when `'strict-dynamic'` is present.

---

## Problem 3: CSP violation reports sent to application endpoints

### Description

After CSP sanitization, `report-uri` and `report-to` directives are preserved unchanged. If instrumentation causes CSP violations (due to edge cases in sanitization, or nonce removal in `content-security-policy-report-only`), the browser sends violation reports to the application's configured reporting endpoint.

### Why it was considered but given lower priority

This produces noise rather than breakage. The application continues to function. However, for a testing tool, generating false CSP violation reports is a meaningful side effect.

### Connected systems and variables

- **`report-to` / `report-uri`**: CSP directives that instruct the browser to POST violation reports to a URL.
- **`content-security-policy-report-only`**: A header that does NOT enforce the policy, only reports violations. The code sanitizes both `content-security-policy` and `content-security-policy-report-only` identically. For the report-only header, violations from instrumentation changes generate real network requests to the reporting endpoint.
- **Report-only nonce stripping**: If the report-only header had `'nonce-...'` values, stripping them causes every nonce-checked script to be "reported" as a violation — even though the enforcing CSP may be fine.

### Consequences if deployed

- CSP monitoring dashboards show instrumentation-caused violations mixed with real ones.
- Automated alerting on CSP violations fires false positives.
- For `content-security-policy-report-only`, stripping nonces generates a report for every script load, potentially flooding the reporting endpoint.
- Mitigating factor: this only affects applications that have CSP reporting configured, and the reports stop when Bombadil stops testing.

---

## Problem 4: Resource type wildcard in CSP match

### Description

The `match resource_type` block uses `_ =>` (wildcard) for the non-Script branch of CSP handling:

```rust
match resource_type {
    network::ResourceType::Script => None,
    _ => sanitize_csp(&h.value).map(...)
}
```

Only `Script` and `Document` resource types are currently registered for interception. The wildcard effectively means `Document` today. But if a future change adds another resource type (e.g., `Stylesheet`, `Worker`), the wildcard silently applies Document-style CSP sanitization to it.

### Why it was considered

`antithesishq/main` uses explicit `bail!` for unexpected resource types in the body-instrumentation branch, demonstrating a preference for explicit matching over wildcards. The wildcard here is inconsistent with that pattern.

### Connected systems and variables

- **`fetch::EnableParams`**: Defines which resource types are intercepted. Currently `Script` and `Document`.
- **The `bail!` pattern**: The body-instrumentation `if/else` chain ends with an explicit `bail!` for unexpected resource types, showing that the codebase prefers to fail loudly on unexpected inputs.
- **Future resource types**: If `Stylesheet` or `Worker` were added, CSP sanitization for those types would need different logic (e.g., `style-src` for stylesheets, `worker-src` for workers).

### Consequences if deployed

- No immediate bug. The wildcard matches `Document` today and nothing else reaches this code path.
- If interception patterns are expanded without updating this match, CSP sanitization designed for Documents would be silently applied to other resource types, potentially causing incorrect behavior.
- Downgraded to a code-quality concern rather than a deployment risk, since changing the EnableParams and not updating this match would be a separate bug regardless.

---

## Problem 5: `Vary` header forwarded after `content-encoding` removal

### Description

The `Vary` header is not in `STRIPPED_RESPONSE_HEADERS`. If the original response included `Vary: Accept-Encoding` (very common) and Bombadil strips `content-encoding`, the `Vary` header now advertises a dimension that is absent from the response.

### Why it was considered but given lower priority

In the context of a testing tool running against localhost with a headless browser, intermediate HTTP caches are unlikely to be involved. The `Vary` header primarily affects caching proxies.

### Connected systems and variables

- **Browser cache**: The browser may use `Vary` to determine cache key partitioning. A `Vary: Accept-Encoding` with no `Content-Encoding` in the response could cause the browser to cache the response under a different key than expected.
- **Service workers**: If the application under test uses service workers that inspect `Vary` headers, the mismatch could cause unexpected cache behavior.

### Consequences if deployed

- Browser caching during a Bombadil test session may behave slightly differently than production, but since tests are short-lived and Bombadil creates fresh browser profiles, the practical impact is negligible.
- Service workers that depend on `Vary` correctness could see cache misses or unexpected matches, but this is an edge case within an edge case.

---

## Problem 6: ETag replacement for non-instrumented content

### Description

When a Document response is non-HTML (XML, PDF, etc.), the body passes through unchanged (`body.clone()`), but the header pipeline still runs: `content-encoding` is stripped, and the ETag is replaced with `source_id.0`.

### Why it was disregarded

This behavior is inherited from `antithesishq/main`, which also replaced the ETag for every intercepted response regardless of whether the body was modified. The new code does not change this behavior — it existed before header forwarding was added. The `source_id` is deterministic (derived from the body hash), so the synthetic ETag is at least stable.

---

## Problem 7: Missing `content-md5` in strip list

### Description

The `Content-MD5` header (RFC 1864) is a body hash, similar to `Digest`. It is not in `STRIPPED_RESPONSE_HEADERS`.

### Why it was disregarded

`Content-MD5` was explicitly deprecated by RFC 7231 (June 2014) and has been removed from the HTTP specification. No modern server or CDN generates it. The `Digest` header (its successor) IS in the strip list. Including `Content-MD5` would be completionist but not practically necessary.

---

## Problem 8: Missing `accept-ranges` handling

### Description

If the original response advertised `Accept-Ranges: bytes`, and the body was modified by instrumentation, range requests against the instrumented content would return wrong byte ranges.

### Why it was disregarded

Range requests against script and HTML document resources are exceedingly rare. Browsers do not make range requests for scripts. The `Accept-Ranges` header is informational — it does not cause breakage by its presence alone, only if a subsequent range request is made.

---

## Summary table

| # | Problem | Severity | Action needed |
|---|---------|----------|---------------|
| 1 | `default-src` hash fallback not handled in `sanitize_csp` | High | Yes — sanitize hashes in `default-src` when no `script-src` is present |
| 2 | `'strict-dynamic'` meaningless after nonce stripping | High | Yes — strip or account for `'strict-dynamic'` when nonces are removed |
| 3 | CSP violation reports to application endpoints | Medium | Consider stripping `report-uri`/`report-to` from sanitized CSP |
| 4 | Resource type wildcard in CSP match | Low | Replace `_` with explicit `network::ResourceType::Document` |
| 5 | `Vary` header forwarded after encoding removal | Low | Negligible for testing tool use case |
| 6 | ETag replacement for non-instrumented content | None | Inherited from antithesishq/main |
| 7 | Missing `Content-MD5` | None | Deprecated header, not practically relevant |
| 8 | Missing `Accept-Ranges` | None | Range requests on scripts are not a real scenario |
