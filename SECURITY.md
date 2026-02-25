# Security Analysis: Header Forwarding in `Fetch.fulfillRequest`

## Context

`src/browser/instrumentation.rs` intercepts script and document responses via CDP's Fetch domain, instruments the body for coverage, and fulfills the request with the modified body. The current implementation forwards all original response headers (minus `etag`) alongside the transformed body.

The problem: several response headers describe properties of the **original** body that become false after instrumentation.

## Transport-Level Header Mismatch

When `Fetch.fulfillRequest` is called, CDP's `GetResponseBody` has already decoded the response (decompressed, de-chunked). The instrumentation then modifies the decoded body, making it larger. But the original transport headers are forwarded verbatim:

### `Content-Length`

The original `Content-Length` reflects the pre-instrumentation body size. The instrumented body is significantly larger (coverage hooks are injected at every branch point). Chrome receives a `Content-Length` that doesn't match the actual body. Depending on Chrome's internal handling of `fulfillRequest` (which is undocumented), this could cause truncated script loads or stalled requests. Behavior may vary across Chrome versions.

### `Content-Encoding`

`GetResponseBody` returns the **decoded** body — gzip/br/deflate decompression has already happened. But `Content-Encoding: gzip` (or `br`, `deflate`) is forwarded, telling Chrome the body is still compressed. Chrome would attempt to decompress plaintext JavaScript, producing garbage. This affects any server that uses compression, which is essentially all production servers.

### `Transfer-Encoding`

`fulfillRequest` delivers the body as a single base64 blob. Forwarding `Transfer-Encoding: chunked` tells Chrome to expect chunk framing that doesn't exist.

## Fix

The `response_headers` filter chain should strip these headers alongside `etag`:

- `content-length`
- `content-encoding`
- `transfer-encoding`

All comparisons should be case-insensitive (matching the existing `etag` filter pattern).
