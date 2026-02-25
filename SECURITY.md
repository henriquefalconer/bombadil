# Security Summary

## What Changed

The `develop` branch fixes a known bug in Bombadil's network interception layer where all response headers were dropped during script instrumentation. The original code (`antithesishq/main`) replaced the entire header set with a single synthetic `etag`, marked with `// TODO: forward headers`. This caused `<script type="module">` tags to fail because browsers enforce strict MIME type checking for ES modules — without the `content-type` header, the module is rejected.

## What the Fix Does

1. **Header forwarding by default.** Response headers are now preserved. A denylist (`STRIPPED_RESPONSE_HEADERS`) removes only headers invalidated by body modification: `etag`, `content-length`, `content-encoding`, `transfer-encoding`, and `digest`.

2. **Resource-type-aware CSP handling.** Content Security Policy headers are processed based on resource type:
   - **Script responses:** CSP dropped entirely (instrumentation invalidates hash-based script trust).
   - **Document responses:** CSP sanitised — script hashes, nonces, `strict-dynamic`, `report-uri`, and `report-to` stripped; non-script directives (`img-src`, `frame-ancestors`, etc.) preserved.
   - **Other resources:** CSP forwarded unchanged.

3. **Report directive stripping.** `report-uri` and `report-to` removed from sanitised CSP to prevent false-positive violation reports against the application's reporting endpoint.

## Security Assessment

No new vulnerabilities are introduced. The changes strictly improve correctness over the previous behaviour (dropping all headers).

- **CSP relaxation is inherent to the tool's purpose.** Bombadil modifies script bodies for coverage instrumentation, which invalidates hash-based CSP. The tool is a testing tool, not a production proxy; relaxing script CSP during testing is expected and necessary.

- **Header forwarding is the correct default.** The previous "drop everything" approach was a bug, not a security feature. Headers like `content-type`, `cache-control`, `set-cookie`, and CORS headers should be forwarded to maintain correct browser behaviour during testing.

- **The denylist is conservative.** It strips all headers whose validity depends on body content (hashes, lengths, encodings, integrity digests). The list is documented with rationale for each entry.

- **Pre-existing concerns** (debug writes to `/tmp/`, `source_id` using request headers instead of response headers, SRI incompatibility) are inherited from `antithesishq/main` and not affected by these changes.

## Non-Disregardable Risk

**`content-type` preservation is implicit.** The header that caused the original bug (`content-type` being dropped) is protected only by its absence from the denylist. No compile-time or runtime assertion prevents it from being accidentally added. The unit test `build_headers_preserves_content_type` and the integration test `test_external_module_script` guard against regression, but the protection is test-dependent. Future maintainers editing the strip list should be aware of this dependency.

## Accepted Trade-offs

- Hash-only `script-src` directives are omitted after sanitisation, falling back to `default-src`. This is inherent to any instrumentation approach that modifies script bodies. The prior state was strictly worse (all CSP dropped).
- `content-security-policy-report-only` is treated identically to the enforcing header — conservative, prevents false-positive reports.
- `Vary` headers referencing stripped headers (e.g., `Vary: Accept-Encoding` after `content-encoding` removal) are forwarded unchanged. No impact in Bombadil's testing context (fresh browser profiles, no intermediate caches).
