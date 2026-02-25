# Integration Tests

- Test functions do not carry `///` doc comments. The test name and the spec string inside the body should be self-explanatory. Rationale for a code change belongs at the change site in production code or in the commit message.
- When a test provides a custom spec and needs interaction to keep the runner loop cycling, export `clicks` as the baseline action. Only export a different action set when the test specifically exercises that action type.
- Use `TEST_TIMEOUT_SECONDS` (120s) as the test timeout unless there is a concrete reason for a shorter bound. When a shorter bound is used, it must be at least 2x any LTL `.within()` value in the spec, because the harness treats `Timeout` as `Success` for `Expect::Success` tests. Prefer reusing existing timeout tiers (3s, 5s, 30s, 120s) over introducing new ones.
- When refactoring a shared helper into a wrapper and a lower-level implementation, the wrapper (the function most tests call) retains the full doc comment. The lower-level function gets a brief one-liner referencing the wrapper.
- If you modify any part of a doc comment, verify the entire comment for factual correctness. Fixing a typo while leaving a wrong URL or outdated description signals carelessness.
- Test HTML fixtures should use the minimal structure needed for the test. Do not add `<head>`, `<title>`, or other elements unless the test specifically exercises them.

# Doc Comments

- Doc comments (`///`) go on public API surfaces, shared helpers, and constants whose purpose isn't obvious from the name. Do not add doc comments to test functions, private one-off helpers, or self-explanatory code.
- When a doc comment references a URL path, code path, or concrete value, verify it matches the actual implementation. Documentation that contradicts the code is worse than no documentation.

# Constants and Magic Values

- Values that control behavior (header lists, timeouts, size limits, namespace strings) should be defined as named `const` or `static` items at module or function level, not inline in closures or expressions. This makes them discoverable, referenceable, and testable.
- Follow the existing naming convention: `SCREAMING_SNAKE_CASE` for constants.

# Header Handling

- When constructing response headers for `Fetch.fulfillRequest`, document why each header is stripped. The strip list is a security-sensitive surface — every entry and every omission should have a stated reason.
- CDP's `Fetch.fulfillRequest` uses replacement semantics: providing `responseHeaders` replaces the entire original header set. Omitting a header is equivalent to actively removing it.
- Any header whose validity depends on body content (hashes, lengths, encodings, integrity digests) becomes stale after instrumentation and must be accounted for in the strip list.
- Prefer failing closed over failing open: when in doubt about whether a header is safe to forward after body modification, strip it.

# Error Handling

- Use `anyhow::Result` with `.context()` for application-level code. Use domain-specific error enums (like `SpecificationError`) only when callers need to match on error variants.
- Reserve `.expect()` and `.unwrap()` for cases where failure indicates a programmer error, not a runtime condition.
- When an interception or instrumentation fails, continue the request uninstrumented rather than crashing. Log the failure at `warn` level.

# Imports

- Alias `serde_json` as `json` everywhere: `use serde_json as json;`.
- Use `::` prefix to disambiguate crate names from local modules (e.g., `use ::url::Url;`).
- Group imports without blank lines between groups, ordering: std, external crates, internal modules.
