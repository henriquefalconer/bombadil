# Implementation Plan

## Completed

- Fix external module script MIME type issue (forwarding response headers in `Fetch.fulfillRequest`)
- Test for external module script (`test_external_module_script`)
- Strip stale transport headers after instrumentation (`content-length`, `content-encoding`, `transfer-encoding`) — prevents Chrome from treating plaintext CDP-decoded body as compressed data
- Test for compressed script (`test_compressed_script`) — uses `CompressionLayer` from `tower-http` to serve gzip-compressed JS, verifying the strip fix end-to-end
