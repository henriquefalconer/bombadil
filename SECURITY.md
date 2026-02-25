# Security Assessment: develop vs antithesishq/main

## Summary

The `develop` branch fixes a bug where `instrument_js_coverage` dropped **all** response headers when fulfilling intercepted requests via CDP's `Fetch.fulfillRequest`. The original code replaced the entire header set with a single synthetic `etag`, silently removing every security and functional header (CORS, HSTS, CSP, content-type, etc.) from every intercepted Script and Document response. The fix forwards upstream headers while stripping only those invalidated by body instrumentation, and adds resource-type-aware CSP handling.

## What Changed

1. **Header forwarding with denylist** — upstream response headers are now preserved by default. A named constant `STRIPPED_RESPONSE_HEADERS` lists the five headers invalidated by instrumentation: `etag`, `content-length`, `content-encoding`, `transfer-encoding`, `digest`. All other headers pass through unchanged.

2. **CSP sanitization** — `content-security-policy` and `content-security-policy-report-only` headers receive resource-type-aware treatment:
   - **Script resources**: CSP is dropped entirely (instrumented body invalidates hash-based `script-src`).
   - **Document resources**: CSP is sanitized — hash/nonce values and orphaned `strict-dynamic` are stripped from `script-src`/`script-src-elem` (falling back to `default-src` when neither is present). `report-uri` and `report-to` are stripped to prevent false-positive reports. All other directives (`img-src`, `frame-ancestors`, `connect-src`, etc.) are preserved.
   - **Other resource types**: CSP is forwarded unchanged.

3. **Test coverage** — 20 unit tests for `sanitize_csp`, 9 unit tests for `build_response_headers`, and 4 new integration tests.

## Security Posture

The changes **improve** the security posture relative to `antithesishq/main`:

- **Before (main)**: All response headers were dropped. Every security header was silently removed from every intercepted response.
- **After (develop)**: All response headers are preserved except the five that instrumentation directly invalidates, plus CSP which receives targeted sanitization rather than wholesale removal.

## Remaining Risks

1. **content-type preservation is implicit** — `content-type` is critical for ES module MIME type enforcement. Its preservation depends solely on its absence from the denylist. Existing unit tests (`build_headers_preserves_content_type`) and integration tests (`test_external_module_script`) catch regression, but the protection is indirect. A future maintainer adding `content-type` to the denylist would silently break all module scripts.

2. **CSP sanitization is best-effort** — Handles the most common CSP patterns but does not cover every construct. Unusual keywords like `'unsafe-hashes'` are left orphaned when their accompanying hashes are stripped (harmless but semantically dead). `require-trusted-types-for` and `sandbox` are forwarded unchanged, which is the correct conservative default.

3. **strict-dynamic removal widens policy** — When `strict-dynamic` is removed (because its trust anchors are stripped), `unsafe-inline` and host-based sources become active. This is intentionally more permissive to allow instrumented scripts to execute during testing. The effective CSP during testing is weaker than production for script-loading decisions.

4. **Denylist assumes completeness** — If a server sends a body-integrity header not in the denylist (e.g., RFC 9530's `repr-digest`), it will be forwarded with an incorrect value. Unlikely to cause security issues; could cause functional failures in specific deployments. The denylist approach is strictly more correct than dropping everything.

## Conclusion

No new security vulnerabilities are introduced. The header forwarding approach is strictly more correct than the previous behavior. The denylist is conservative, well-documented, and each entry has a stated reason. CSP handling is thorough for common cases and fails safe when in doubt.
