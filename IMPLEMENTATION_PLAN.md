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
- `build_response_headers` helper extracted from inline iterator chain; signature `(response_headers: &Option<Vec<fetch::HeaderEntry>>, resource_type: &network::ResourceType, source_id: SourceId) -> Vec<fetch::HeaderEntry>`; placed after `sanitize_csp`
- 7 unit tests for `build_response_headers`: stripped headers removed, content-type preserved, CSP dropped for Script, CSP sanitised for Document, synthetic etag appended, None headers produce only synthetic etag, non-CSP/non-stripped headers pass through
- Section-separator comments (`// Item N: …`) removed from test module
