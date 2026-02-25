# Security Considerations

## Header Forwarding in Request Interception

Bombadil intercepts HTTP responses via CDP's `Fetch.fulfillRequest` to inject coverage instrumentation into JavaScript. When fulfilling a request, CDP uses **replacement semantics** — the provided headers replace the original set entirely.

### Current Approach

The code forwards all original response headers except a denylist of 7 headers that become invalid after body instrumentation:

| Stripped Header | Reason |
|----------------|--------|
| `etag` | Replaced with a fresh source-ID-based etag |
| `content-length` | Body size changes after instrumentation |
| `content-encoding` | CDP's `GetResponseBody` returns decompressed content; the instrumented body is uncompressed |
| `transfer-encoding` | Same as above (chunked encoding no longer applies) |
| `content-security-policy` | Script hash digests in CSP no longer match the instrumented script body |
| `content-security-policy-report-only` | Same as above |
| `strict-transport-security` | Prevents HSTS pinning on the interception endpoint |

### Known Limitations

1. **Denylist is not exhaustive.** Headers like `digest`, `repr-digest` (RFC 9530), or custom CDN checksum headers are not stripped. If the target site or its infrastructure adds body-dependent headers not in the list, they will be forwarded with stale values. This could cause integrity verification failures in downstream proxies or service workers.

2. **CSP is stripped entirely, not modified.** This means CSP protections (XSS prevention, inline script blocking) are disabled during Bombadil testing. Properties that depend on CSP enforcement behavior cannot be tested. This is an inherent limitation of body-rewriting instrumentation.

3. **HSTS stripping runs in all contexts.** The `strict-transport-security` header is stripped for all target sites, not just localhost. In practice this is harmless (Bombadil uses ephemeral browser profiles), but the code comment's rationale is narrower than the actual scope.

### Recommendations for Future Work

- Consider switching to an allowlist approach (only forward `content-type`, `set-cookie`, `cache-control`, `access-control-*`, etc.) to fail closed against unknown body-dependent headers.
- Consider selectively modifying CSP (adding instrumentation script hashes to the allowlist) rather than stripping it entirely, to preserve CSP enforcement for the tested application.
