# Implementation Plan

## Remaining Items

Items are ordered by priority. All address issues from SECURITY.md.

### 1. CSP: Selective stripping for documents (HIGH)

**Problem:** `content-security-policy` and `content-security-policy-report-only` are stripped from ALL responses, including HTML documents. For documents, this removes the entire page security policy (frame-ancestors, connect-src, img-src, etc.), creating a fidelity gap.

**Fix:**
- Add a `sanitize_csp(csp_value: &str) -> Option<String>` function that parses semicolon-delimited CSP directives and removes only `'sha256-...'`, `'sha384-...'`, `'sha512-...'`, and `'nonce-...'` values from `script-src` and `script-src-elem` directives. All other directives pass through unchanged. Returns `None` if the result is empty.
- Remove `content-security-policy` and `content-security-policy-report-only` from `STRIPPED_RESPONSE_HEADERS`.
- Make the `FulfillRequestParams` header filtering resource-type-aware: pass `resource_type` into the filter logic. For `ResourceType::Script`: strip CSP entirely (unchanged behavior). For `ResourceType::Document`: apply `sanitize_csp` to CSP/CSP-RO headers instead of stripping them.
- **Unit tests** for `sanitize_csp`: hash removal, nonce removal, mixed directives, no script-src, empty result, multiple script-src values, directive with only hash values removed entirely.
- **Integration test** `test_csp_document_directives_preserved`: Server middleware adds CSP `script-src 'unsafe-inline' 'self' 'sha256-ORIGINAL'; img-src 'self'` to document responses. Page has inline script that listens for `securitypolicyviolation` events and sets `#result` to `"CSP_ACTIVE"` when one fires, then tries loading a cross-origin image (`data:` URI or `https://external.invalid/x.png`). Spec uses `eventually(() => resultText.current === "CSP_ACTIVE").within(10, "seconds")`. Before fix: CSP stripped → no violation → timeout → FAIL. After fix: CSP preserved with hash removed → `img-src 'self'` enforced → violation event → PASS.
- Existing `test_csp_script` continues to verify that script-level CSP hash stripping works.

### 2. HSTS: Stop unconditional stripping (LOW)

**Problem:** `strict-transport-security` is stripped from all responses. HSTS provides intra-run HTTPS enforcement when fuzzing real HTTPS targets. Ephemeral `TempDir` profiles already prevent cross-run persistence, so stripping is unnecessary.

**Fix:**
- Remove `strict-transport-security` from `STRIPPED_RESPONSE_HEADERS`.
- **Integration test** `test_hsts_preserved`: Server middleware adds `strict-transport-security: max-age=31536000` header. Page inline script checks if the header round-trips by making a `fetch(window.location.href)` and reading `response.headers.get('strict-transport-security')`, setting `#result` to `"HSTS_PRESENT"` or `"HSTS_ABSENT"`. Spec: `eventually(() => resultText.current === "HSTS_PRESENT").within(10, "seconds")`. Before fix: header stripped → `HSTS_ABSENT` → timeout → FAIL. After fix: header preserved → `HSTS_PRESENT` → PASS.

### 3. Add `digest` header to denylist (LOW)

**Problem:** The `digest` header (RFC 3230/9530) contains a hash of the response body. After instrumentation the hash is wrong. While rare, a service worker validating it would reject the instrumented script.

**Fix:**
- Add `"digest"` to `STRIPPED_RESPONSE_HEADERS` with a comment explaining the rationale.
- No integration test needed (extremely rare in practice; the fix is a one-line addition to a well-tested denylist).

### 4. HTML fixture cleanup for fork-added tests

**Problem:** The 3 fork-added test fixtures (`csp-script`, `compressed-script`, `external-module-script`) use `<!DOCTYPE html>`/`<!doctype html>`, lack `<head>` and `<title>` elements, and have inconsistent indentation — violating PATTERNS.md conventions.

**Fix:**
- Update each fixture to match upstream structure: `<html>`, `<head>`, `<title>`, `<body>`. Remove `<!DOCTYPE html>`. Use consistent 2-space indentation.

### Not Addressed

- **Non-HTML document header filtering (SECURITY.md §5):** SECURITY_ANALYSIS.md disposition is "disregarded." CDP's `GetResponseBody` returns decompressed content, making `content-encoding` stale even for unmodified bodies. `content-length` is recalculated by CDP. The fork already improves on upstream (which dropped ALL headers). No action needed.
