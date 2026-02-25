# Security Assessment: develop vs antithesishq/main

## Summary of Changes

The `develop` branch fixes a bug where `instrument_js_coverage` dropped **all** response headers when fulfilling intercepted requests via CDP. The original code replaced the entire header set with a single synthetic `etag`. The fix forwards upstream headers while stripping only those invalidated by body instrumentation, and adds resource-type-aware CSP handling.

## What Changed

1. **Header forwarding with denylist** — upstream response headers are now preserved by default. A named constant `STRIPPED_RESPONSE_HEADERS` lists the five headers invalidated by instrumentation (`etag`, `content-length`, `content-encoding`, `transfer-encoding`, `digest`). All other headers pass through.

2. **CSP sanitization** — `content-security-policy` and `content-security-policy-report-only` headers receive resource-type-aware treatment:
   - **Script resources**: CSP is dropped entirely (instrumented body invalidates hash-based script-src).
   - **Document resources**: CSP is sanitized — hash/nonce values and `strict-dynamic` are removed from `script-src`/`script-src-elem` (falling back to `default-src` when neither is present). `report-uri` and `report-to` directives are stripped. All other directives (`img-src`, `frame-ancestors`, etc.) are preserved.
   - **Other resource types**: CSP is forwarded unchanged.

3. **Test coverage** — 19 unit tests for `sanitize_csp`, 9 unit tests for `build_response_headers`, and 4 new integration tests (`external-module-script`, `compressed-script`, `csp-script`, `csp-document`).

## Security Posture

The changes **improve** the security posture relative to `antithesishq/main`:

- **Before (main)**: All response headers were dropped. This silently removed CORS, HSTS, X-Frame-Options, X-Content-Type-Options, Referrer-Policy, Permissions-Policy, and every other security header from every intercepted response.
- **After (develop)**: All response headers are preserved except the five that instrumentation directly invalidates, plus CSP which is handled specifically.

## Remaining Risks

1. **content-type preservation is implicit** — `content-type` is critical for module script MIME checking (the original bug). Its preservation depends on its absence from `STRIPPED_RESPONSE_HEADERS`. There is no positive assertion in the production code path that `content-type` must survive; it is protected only by unit and integration tests that would catch a regression.

2. **CSP sanitization is best-effort** — The sanitizer handles the most common CSP patterns (hash, nonce, strict-dynamic, report directives) but does not cover every edge case in the CSP specification. Unusual constructs like `require-trusted-types-for` or `sandbox` directives with script-adjacent effects are forwarded unchanged, which is the conservative default.

3. **Headers that interact with instrumented bodies** — If a server sends a header whose semantics depend on body integrity and that header is not in the denylist (e.g., a proprietary `X-Body-Checksum`), it will be forwarded with an incorrect value. This is unlikely to cause security issues but could cause functional failures in specific deployments.

## Conclusion

No new security vulnerabilities are introduced by these changes. The header forwarding approach is strictly more correct than the previous behavior of dropping all headers. The denylist is conservative and well-documented. The CSP handling is thorough for the common cases and fails safe (drops rather than allows) when in doubt.
