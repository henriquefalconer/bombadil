# Security Considerations for Header Forwarding

## Resolved Issues

The following issues from prior analyses have been addressed in the current code:

1. **`default-src` hash fallback**: `sanitize_csp` strips hashes/nonces from `default-src` when no explicit `script-src` or `script-src-elem` is present.
2. **`strict-dynamic` orphaning**: `'strict-dynamic'` is removed alongside hashes and nonces, preventing a directive without a trust anchor.
3. **CSP violation report noise**: `report-uri` and `report-to` directives are stripped entirely from sanitized CSP headers.
4. **Resource type wildcard**: The CSP match block uses explicit `Script` and `Document` arms, with `_ =>` as a conservative passthrough that preserves CSP unchanged.
5. **Inline iterator chain complexity**: Header construction logic extracted into `build_response_headers` helper with 7 dedicated unit tests.

## Accepted Trade-offs

### Hash-only `script-src` removal widens the security model

When all values in a `script-src` directive are hashes/nonces/`strict-dynamic`, the directive is omitted entirely. The browser falls back to `default-src`, which may be more permissive. This is inherent to Bombadil's approach: instrumentation changes script bodies, making hash-based trust impossible without computing new hashes at CSP-emission time. The weakening only applies during the test session.

### `content-security-policy-report-only` identical treatment

Both enforcing and report-only CSP headers are treated the same way: dropped for Script resources, sanitized for Document resources. After sanitization removes `report-uri`/`report-to`, the report-only header is effectively inert. Passing it through unchanged would reintroduce the false-positive reporting problem.

### `content-type` preservation is implicit

The `content-type` header — critical for ES module MIME type checking — is preserved by not being in `STRIPPED_RESPONSE_HEADERS`. There is no explicit safeguard preventing its accidental addition to the strip list. Future modifications to the strip list should verify that `content-type` remains forwarded.

## Pre-existing Limitations (not introduced by this change)

### SRI (`integrity` attribute) incompatibility

Script tags with `integrity` attributes will fail SRI checks after instrumentation modifies the body. This existed before header forwarding — `antithesishq/main` had the same limitation.

### ETag replacement for non-instrumented content

Non-HTML Document responses pass through with their body unchanged, but still receive a synthetic ETag. Inherited from `antithesishq/main`.
