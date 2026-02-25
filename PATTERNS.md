# Code Patterns and Conventions

Rules derived from studying the patterns established in the upstream codebase (`antithesishq/main`). Following these ensures new code stays consistent with the project and avoids the deviations documented here.

---

## Integration Tests

### Document the fix, not the test

Test functions do not carry `///` doc comments. The test name and the spec string inside the body should be self-explanatory. If the rationale behind a code change needs to be preserved in source, place it at the change site (e.g., a `//` comment next to the relevant logic in production code), not on the test function. The commit message is the right place for the full narrative.

**Upstream evidence:** All 11 test functions in the upstream have zero doc comments. The only doc comments in the test file are on the shared helper (`run_browser_test`) and the semaphore constant.

### Use `clicks` as the minimal action export in custom specs

When a test provides a custom spec and needs any form of interaction to keep the runner loop cycling, export `clicks` as the baseline. Only export a different action set when the test specifically exercises that action type (e.g., `back` for back-navigation tests, `inputs` for text input tests).

**Upstream evidence:** Every upstream custom spec exports `clicks` as its baseline (`test_random_text_input`, `test_counter_state_machine`, `test_back_from_non_html`). No upstream test exports `scroll` as a standalone action.

### Default to `TEST_TIMEOUT_SECONDS` unless there is a specific reason for a shorter bound

The constant `TEST_TIMEOUT_SECONDS` (120s) is the standard test timeout. Use it unless the test has a concrete reason for a shorter bound (e.g., the test is expected to finish in under a second and a long timeout would waste CI time on a hang).

If a shorter bound is used, ensure it is at least 2x any LTL `.within()` value in the spec. The test harness treats `Timeout` as `Success` for `Expect::Success` tests, so if the test timeout is equal to or shorter than the LTL bound, the test can pass vacuously when the LTL engine hasn't had time to produce a violation.

Prefer reusing existing timeout tiers (3s, 5s, 30s, 120s) over introducing new ones.

**Upstream evidence:** `test_back_from_non_html` uses 30s test / 20s LTL (1.5x). `test_random_text_input` uses 120s test / 10s LTL (12x). No upstream test has a test timeout equal to its LTL `.within()` bound.

### The helper that test authors call carries the documentation

When refactoring a shared helper into a higher-level wrapper and a lower-level implementation, the wrapper (the function most tests call) retains the full doc comment. The lower-level function gets a brief one-liner referencing the wrapper.

**Upstream evidence:** The single `run_browser_test` function carried the doc comment explaining the two-server setup and URL conventions. All tests called this one function directly.

### If you modify any part of a doc comment, verify the entire comment

Touching a comment signals that you have read it. Leaving a known inaccuracy after modifying adjacent text undermines trust in the documentation. If you change one line of a doc comment, review the rest for factual correctness.

---

## Header Handling (`src/browser/instrumentation.rs`)

### Prefer an explicit rationale for each forwarded or stripped header

When constructing response headers for `Fetch.fulfillRequest`, the choice of which headers to forward and which to strip has security and correctness implications. Each header in the strip list should have a documented reason (e.g., `content-encoding` is stripped because CDP's `GetResponseBody` returns decompressed content). When adding a new header to the list or removing one, state why.

### Be aware that `Fetch.fulfillRequest` uses replacement semantics

CDP does not merge `responseHeaders` with the original response headers. Providing `responseHeaders` replaces the entire header set. This means omitting a header is equivalent to actively removing it from the response the browser sees.

### Consider that instrumentation changes the body

Any header whose validity depends on the body content (hashes, lengths, encodings, CSP script digests) becomes stale after instrumentation. The strip list should account for all such headers, not just transport-layer ones.
