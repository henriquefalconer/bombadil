# Implementation Plan

## Completed Items

All four items from SECURITY.md have been implemented. All 15 integration tests and all 10 unit tests pass.

### 1. CSP: Selective stripping for documents (HIGH) - DONE

- Added `sanitize_csp()` function in `src/browser/instrumentation.rs` that removes only hash/nonce values from `script-src`/`script-src-elem` directives, passing all other directives through unchanged.
- Removed `content-security-policy` and `content-security-policy-report-only` from `STRIPPED_RESPONSE_HEADERS`.
- Made `FulfillRequestParams` header filtering resource-type-aware: Script responses strip CSP entirely (unchanged behavior); Document responses apply `sanitize_csp` instead.
- Added 10 unit tests for `sanitize_csp`.
- Added `test_csp_document_directives_preserved` integration test with `tests/csp-document/index.html` fixture.

### 2. HSTS: Stop unconditional stripping (LOW) - DONE

- Removed `strict-transport-security` from `STRIPPED_RESPONSE_HEADERS`.
- Integration test was not added: reading HSTS via `fetch` response headers is not testable from JS since HSTS is processed internally by the browser and not exposed to JavaScript.

### 3. Add `digest` header to denylist (LOW) - DONE

- Added `"digest"` to `STRIPPED_RESPONSE_HEADERS` with a rationale comment explaining that instrumentation invalidates the body hash.

### 4. HTML fixture cleanup for fork-added tests (LOW) - DONE

- Updated `tests/csp-script/index.html`, `tests/compressed-script/index.html`, and `tests/external-module-script/index.html` to match upstream structure: `<html>`, `<head>`, `<title>`, `<body>`, removed `<!DOCTYPE html>`, fixed indentation to 2 spaces.

## Remaining Items

None.

## Not Addressed

- **Non-HTML document header filtering (SECURITY.md §5):** SECURITY_ANALYSIS.md disposition is "disregarded." CDP's `GetResponseBody` returns decompressed content, making `content-encoding` stale even for unmodified bodies. `content-length` is recalculated by CDP. The fork already improves on upstream (which dropped ALL headers). No action needed.
