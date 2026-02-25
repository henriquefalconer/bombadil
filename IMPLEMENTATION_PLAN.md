# Implementation Plan

## Completed

- Response header forwarding pipeline: `STRIPPED_RESPONSE_HEADERS` constant + iterator chain in `FulfillRequestParams` builder
- `sanitize_csp` function: strips hashes/nonces from `script-src`/`script-src-elem`, `default-src` fallback, `strict-dynamic` orphaning, `report-uri`/`report-to` removal
- Resource type explicit matching: `Script` and `Document` arms, `_ =>` passthrough
- 19 unit tests for `sanitize_csp` covering all SECURITY.md resolved issues
- 4 integration tests: `test_external_module_script`, `test_compressed_script`, `test_csp_script`, `test_csp_document_directives_preserved`
- HTML fixtures for all new integration tests
- `run_browser_test` split into wrapper + `run_browser_test_with_router`
- `Cargo.toml`: `compression-gzip` feature for `tower-http`

## Remaining Items

### 1. Extract header-construction logic into a named helper function

The `FulfillRequestParams` builder contains a ~52-line inline iterator chain (lines 194–245 of `src/browser/instrumentation.rs`) that filters headers, applies resource-type-aware CSP handling, and appends a synthetic etag. Both SECURITY.md and PATTERNS.md require extracting this into a named helper.

Proposed signature:
```rust
fn build_response_headers(
    response_headers: &Option<Vec<fetch::HeaderEntry>>,
    resource_type: &network::ResourceType,
    source_id: SourceId,
) -> Vec<fetch::HeaderEntry>
```

Place after `sanitize_csp`. The builder call site becomes `.response_headers(build_response_headers(...))`.

### 2. Add unit tests for the extracted helper function

Once extracted, add unit tests covering:
- Headers in `STRIPPED_RESPONSE_HEADERS` are removed (etag, content-length, content-encoding, transfer-encoding, digest)
- `content-type` is preserved (the root cause of the module script issue — verifies the fix)
- CSP header is dropped for `Script` resources
- CSP header is sanitized for `Document` resources
- Synthetic etag is appended with the correct `source_id` value
- `None` response headers produce only the synthetic etag
- Non-CSP, non-stripped headers pass through unchanged

### 3. Remove section-separator comments from test module

Lines 486, 517, 544 of `instrumentation.rs` contain `// Item N: ...` section separators inside the `#[cfg(test)]` module. PATTERNS.md forbids these — remove them.
