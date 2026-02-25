# Security Considerations for Header Forwarding

## Resolved Issues

The following issues from the prior analysis have been addressed in the current code:

1. **`default-src` hash fallback**: `sanitize_csp` now strips hashes/nonces from `default-src` when no explicit `script-src` or `script-src-elem` is present.
2. **`strict-dynamic` orphaning**: `'strict-dynamic'` is removed alongside hashes and nonces, preventing a directive without a trust anchor.
3. **CSP violation report noise**: `report-uri` and `report-to` directives are stripped entirely from sanitized CSP headers.
4. **Resource type wildcard**: The CSP match block now uses explicit `network::ResourceType::Script` and `network::ResourceType::Document` arms, with `_ =>` as a conservative passthrough.

## Accepted Trade-offs

### Hash-only `script-src` removal widens the security model

When all values in a `script-src` directive are hashes/nonces/`strict-dynamic`, the directive is omitted entirely. The browser falls back to `default-src`, which may be more permissive. This is inherent to Bombadil's approach: instrumentation changes script bodies, making hash-based trust impossible without computing new hashes at CSP-emission time. The weakening only applies during the test session.

### `content-security-policy-report-only` identical treatment

Both enforcing and report-only CSP headers are treated the same way: dropped for Script resources, sanitized for Document resources. After sanitization removes `report-uri`/`report-to`, the report-only header is effectively inert. Passing it through unchanged would reintroduce the false-positive reporting problem.

## Pre-existing Limitations (not introduced by this change)

### SRI (`integrity` attribute) incompatibility

Script tags with `integrity` attributes will fail SRI checks after instrumentation modifies the body. This existed before header forwarding — `antithesishq/main` had the same limitation.

### ETag replacement for non-instrumented content

Non-HTML Document responses pass through with their body unchanged, but still receive a synthetic ETag. Inherited from `antithesishq/main`.

## Code Quality Note

The header-construction logic is currently an inline iterator chain inside the `FulfillRequestParams` builder. Extracting it into a named helper function would improve reviewability and allow independent unit testing of the header filtering logic.
