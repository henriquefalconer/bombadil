# Security Summary

## What Changed

The `develop` branch fixes a known issue in Bombadil's network interception layer where all response headers were dropped during script instrumentation. The original code (`antithesishq/main`) replaced the entire header set with a single synthetic `etag`, marked with `// TODO: forward headers`. This caused `<script type="module">` tags to fail because the browser requires a valid `Content-Type` header for ES module MIME type enforcement.

## What the Fix Does

1. **Header forwarding**: Response headers are now preserved by default. A denylist (`STRIPPED_RESPONSE_HEADERS`) removes only headers invalidated by body modification: `etag`, `content-length`, `content-encoding`, `transfer-encoding`, and `digest`.

2. **CSP handling**: Content Security Policy headers receive resource-type-aware treatment:
   - **Script responses**: CSP dropped entirely (instrumentation invalidates hash-based trust).
   - **Document responses**: CSP sanitised — script hashes/nonces and `strict-dynamic` stripped, non-script directives (`img-src`, `frame-ancestors`, etc.) preserved.
   - **Other resources**: CSP forwarded unchanged.

3. **Report directive stripping**: `report-uri` and `report-to` removed from sanitised CSP to prevent false-positive violation reports.

## Security Assessment

No new vulnerabilities are introduced. The changes strictly improve correctness over the previous behaviour (dropping all headers).

- **CSP relaxation is inherent to the tool's purpose.** Bombadil modifies script bodies for coverage instrumentation, which invalidates hash-based CSP. The tool is a testing tool, not a production proxy; relaxing script CSP during testing is expected and necessary.

- **Header forwarding is the correct default.** The previous "drop everything" approach was a bug, not a security feature. Headers like `content-type`, `cache-control`, `set-cookie`, and CORS headers should be forwarded to maintain correct browser behaviour during testing.

- **The denylist is conservative.** It strips all headers whose validity depends on body content. The list covers all realistic scenarios (deprecated headers like `content-md5` are excluded as non-concerns).

- **Pre-existing concerns** (debug writes to `/tmp/`, source ID using request headers, SRI incompatibility) are inherited from `antithesishq/main` and not affected by these changes.

## Accepted Trade-offs

- Hash-only `script-src` directives are omitted after sanitisation, falling back to `default-src`. This is inherent to any instrumentation approach that modifies script bodies. The prior state was strictly worse (all CSP dropped).
- `content-type` preservation depends on its absence from the strip list, guarded by a unit test (`build_headers_preserves_content_type`). Future modifications to the strip list should verify `content-type` remains forwarded.
- `content-security-policy-report-only` is treated identically to the enforcing header. This prevents false-positive reports and is conservative.
