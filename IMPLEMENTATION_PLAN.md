# Implementation Plan

## Previously Completed

- CSP selective stripping for documents (hash/nonce removal in `script-src`/`script-src-elem`, resource-type-aware header filtering)
- HSTS: stopped unconditional stripping (`strict-transport-security` removed from denylist)
- Added `digest` header to denylist (instrumentation invalidates body hash)
- HTML fixture cleanup for fork-added tests (matched upstream structure)
- `default-src` hash fallback stripping: when no `script-src`/`script-src-elem` present, hashes/nonces are now stripped from `default-src` too
- `strict-dynamic` removal without trust anchor: `'strict-dynamic'` is now removed from any directive whose hashes/nonces are stripped (meaningless without a trust anchor)
- `report-uri`/`report-to` directives stripped: prevents false-positive CSP violation reports triggered by instrumentation
- Resource type wildcard fixed: `_ =>` replaced with explicit `network::ResourceType::Document =>` + passthrough `_` arm

## Remaining Items

(none)
