# Integration Tests

- Test functions do not carry `///` doc comments. The test name and the spec string inside the body should be self-explanatory. Rationale for a code change belongs at the change site in production code or in the commit message.
- Keep inline `//` comments on test functions to a minimum. If a test's purpose is not clear from its name and spec string, improve the name or the spec before adding a comment block. A one-line comment for non-obvious setup is acceptable; multi-line explanations are not. Historical context (e.g., how main vs develop differ, why a regression test was added) belongs in commit messages, not in test bodies.
- When a test provides a custom spec and needs interaction to keep the runner loop cycling, export `clicks` as the baseline action. Only export a different action set when the test specifically exercises that action type.
- Use `TEST_TIMEOUT_SECONDS` (120s) as the test timeout unless there is a concrete reason for a shorter bound. When a shorter bound is used, it must be at least 2x any LTL `.within()` value in the spec, because the harness treats `Timeout` as `Success` for `Expect::Success` tests. Only use existing timeout tiers (3s, 5s, 30s, 120s); do not introduce new values.
- When refactoring a shared helper into a wrapper and a lower-level implementation, the wrapper (the function most tests call) retains the full doc comment. The lower-level function gets a brief one-liner referencing the wrapper.
- If you modify any part of a doc comment, verify the entire comment for factual correctness. Fixing a typo while leaving a wrong URL or outdated description signals carelessness.
- Test fixture directories use kebab-case names that match the test function name after dropping the `test_` prefix and converting underscores to hyphens (e.g., `test_external_module_script` -> `external-module-script/`). Each directory must contain an `index.html` file.
- Test HTML fixtures should follow the structure used by existing fixtures in the `tests/` directory: include `<html>`, `<head>`, and `<title>` elements. Omit `<!DOCTYPE html>`, `<meta>`, viewport tags, and styling unless the test specifically exercises them. The `<title>` should be a human-readable name for the test case.
- All HTML fixtures for the same logical test pattern (e.g., "script loads and sets text content") should use identical structure. Do not vary whitespace, indentation style, or casing of HTML tags between fixtures that serve the same purpose.
- When multiple test fixtures contain identical files (e.g., the same `script.js`), place the shared file in the `tests/shared/` directory and reference it via an absolute path in the HTML (e.g., `/shared/script.js`). Do not duplicate files across fixture directories.
- When multiple tests share the same setup logic (e.g., building a router with specific middleware), extract that logic into a named helper function rather than duplicating the closure or builder inline.
- When several integration tests verify the same class of property (e.g., "script loads and sets DOM text"), use a consistent spec structure: same `extract` pattern, same `eventually(...).within(...)` form, same baseline action export. Consistency across similar tests makes deviations visible and review easier.
- A test fixture should only exercise the feature the test is named for. Do not add attributes, headers, or structural elements that implicitly test unrelated features. Exception: when a test is a regression test for a bug whose fix involves preserving `content-type`, use `<script type="module" src="...">` — this makes the test fail with the old (broken) code (dropped `content-type` → module MIME check fails) and pass with the fixed code (`content-type` preserved, `content-encoding` stripped). Without `type="module"`, both old and new code produce the same outcome and the test does not distinguish before/after the fix.
- Each integration test should verify one behaviour. When two capabilities are tightly coupled (e.g., CSP stripping for scripts and CSP preservation for documents), a combined test is acceptable only if the spec property explicitly names both conditions and no individual test already covers each side.
- In spec raw strings, escape `#` in CSS selectors as `\#` (e.g., `state.document.body?.querySelector("\#result")`). This is required because `#` has special meaning in Rust raw string delimiters at certain nesting levels.

# Unit Tests

- Place `#[cfg(test)] mod tests` blocks at the end of the file, after all production code.
- Define test helper functions (constructors, factory functions) inside the `mod tests` block, not outside it. Test helpers are private to the test module.
- Do not use section-separator comments (`// Item 1: ...`, `// --- ...`) inside unit test modules to organize tests by topic. Test ordering and `#[test]` names are sufficient grouping. If a module has so many tests that it needs internal headers, consider splitting into submodules.
- When a unit test module grows beyond a handful of tests, group related tests by naming convention (e.g., `sanitize_csp_*`, `build_headers_*`) rather than by structural dividers.

# Doc Comments

- Doc comments (`///`) go on public API surfaces, shared helpers, and constants whose purpose is not obvious from the name. Do not add doc comments to test functions, private one-off helpers, or self-explanatory code.
- When a doc comment references a URL path, code path, or concrete value, verify it matches the actual implementation. Documentation that contradicts the code is worse than no documentation.
- Private functions that are complex, security-sensitive, or called from both production and test code qualify for `///` doc comments. Simple private helpers that are only called from one site do not.
- Keep doc comments proportional to the function's complexity. A one-line summary is sufficient for most functions. Multi-paragraph doc comments are warranted only for security-sensitive logic, complex algorithms, or public API entry points. Do not repeat information that is obvious from the function signature or parameter names.

# Constants and Magic Values

- Values that control behavior (header lists, timeouts, size limits, namespace strings) should be defined as named `const` or `static` items at module or function level, not inline in closures or expressions. This makes them discoverable, referenceable, and testable.
- Follow the existing naming convention: `SCREAMING_SNAKE_CASE` for constants.

# Cross-Boundary Contracts

- When the same set of values must exist in both TypeScript and Rust (e.g., key codes in `keycodes()` and `key_info()`, action variant names in `Action` and `JsAction`), keep the two definitions adjacent in intent and verify they match after every change. A mismatch produces runtime errors that are not caught at compile time.
- When a protocol or API defines distinct concepts for the same domain (e.g., CDP's `code` vs `key` for keyboard events), the Rust data model should preserve the distinction even if current values happen to be identical. Use separate struct fields rather than reusing one field for both, so that future additions cannot silently conflate them.
- When simulating browser input via CDP, match the event sequence and field values used by reference implementations (e.g., Puppeteer's `USKeyboardLayout`). Deviations from reference behavior should be documented with the reason for divergence.

# Header Handling

- When constructing response headers for `Fetch.fulfillRequest`, document why each header is stripped. The strip list is a security-sensitive surface — every entry and every omission should have a stated reason.
- Headers that must NOT be stripped (e.g., `content-type`, which is required for ES module MIME type enforcement) should be called out in a comment near the strip list. The strip list documents removals; a complementary note should document critical preservations to prevent accidental regression.
- When using a denylist approach (forward by default, strip listed headers), add a unit test that verifies each critical header is preserved. A comment in the strip list documents intent; a unit test enforces it against future regressions.
- CDP's `Fetch.fulfillRequest` uses replacement semantics: providing `responseHeaders` replaces the entire original header set. Omitting a header is equivalent to actively removing it.
- Any header whose validity depends on body content (hashes, lengths, encodings, integrity digests) becomes stale after instrumentation and must be accounted for in the strip list.
- Prefer failing closed over failing open: when in doubt about whether a header is safe to forward after body modification, strip it.
- Header stripping decisions must account for resource type. A header that is safe to strip from a Script response may not be safe to strip from a Document response. When the same `FulfillRequestParams` code path serves multiple resource types, either use separate denylists or verify that each entry is valid for all resource types that pass through it.
- Keep the `FulfillRequestParams` builder call simple. Move header filtering, transformation, and conditional logic into a named helper function so that the builder reads as a sequence of named values. Inline iterator chains with closures, conditionals, and function calls inside a builder are hard to review and easy to get wrong.
- When sanitizing CSP headers, account for the browser's directive fallback chain. If `script-src` is absent, the browser falls back to `default-src` for script-loading decisions. Sanitization logic that only processes `script-src` and `script-src-elem` misses this fallback.
- CSP values with semantic dependencies must be handled together. Stripping `'nonce-…'` or `'sha…'` from a directive that contains `'strict-dynamic'` leaves `'strict-dynamic'` without a trust anchor; the orphaned value must also be removed or the directive must be dropped entirely.
- When modifying CSP headers, consider whether preserved directives can cause external side effects. `report-uri` and `report-to` direct the browser to POST violation reports to external endpoints; forwarding these after instrumentation-induced policy changes generates false reports.
- When stripping CSP hash/nonce values from a directive, also remove any keyword that becomes semantically meaningless without them. `'unsafe-hashes'` without accompanying hashes has no effect but is dead syntax that misleads readers. Strip it alongside `'strict-dynamic'` when its trust anchors are removed.

# Request Interception

- Non-200 upstream responses should be forwarded via `ContinueRequestParams` (pass-through), not `FulfillRequestParams` (replace). Only replace the response body when instrumenting a successful response.
- The response status code passed to `FulfillRequestParams` should reflect the actual upstream status that was checked. Do not hardcode a status code that could diverge from the condition guarding the code path.
- When interception or instrumentation fails mid-flight, fall back to `ContinueRequestParams` to forward the original response uninstrumented rather than dropping the request. Log the failure at `warn` level.

# Pattern Matching

- When a code path branches on resource type, match each registered interception type explicitly. Use `_ =>` only when it represents a genuinely safe, conservative default (e.g., forward unchanged) and include a brief comment explaining why the default is safe. Do not use `_ =>` as a stand-in for "the one other type currently registered."
- When a `match` arm is currently unreachable due to upstream filtering (e.g., only Script and Document are registered for Fetch interception), but serves as a safe default for future extensibility, add a comment noting which types currently reach the match and why the default is safe for any others.

# Builder Patterns

- Keep builder call sites short. If a builder argument requires multi-line logic (filtering, mapping, branching), extract the computation into a local `let` binding or a named function. The builder call should read as a list of named values, not contain inline algorithms.
- When an iterator chain inside a builder exceeds ~10 lines, it should be a named function that returns the iterator or collected result.

# Error Handling

- Use `anyhow::Result` with `.context()` for application-level code. Use domain-specific error enums (like `SpecificationError`) only when callers need to match on error variants.
- Reserve `.expect()` and `.unwrap()` for cases where failure indicates a programmer error, not a runtime condition.
- When an interception or instrumentation fails, continue the request uninstrumented rather than crashing. Log the failure at `warn` level.

# Imports

- Alias `serde_json` as `json` everywhere: `use serde_json as json;`.
- Use `::` prefix to disambiguate crate names from local modules (e.g., `use ::url::Url;`).
- Group imports by source: external crates first, then `crate::` local imports, separated by a blank line.
- When grouping multiple items from a single crate inside `{}`, order them alphabetically by the full path of each item (e.g., `extract::Request` before `http::HeaderValue` before `middleware` before `response::Response` before `Router`).

# Logging

- Use `log::info!` for high-level lifecycle events (test start, browser initiated, test complete). Use `log::debug!` for operational details (individual request interception, file writes, state transitions). Use `log::warn!` for recoverable errors (failed instrumentation, failed request continuation). Use `log::error!` only for unrecoverable failures.
- Do not add log statements that serve only as CI cache-busters or change-detection markers. Every log call should have diagnostic value.

# Dev-Dependencies

- Feature flags on dev-dependencies should be the minimum required for test functionality. Do not enable features speculatively; add them when a test concretely needs them and document which test drives the requirement (e.g., `compression-gzip` for `test_compressed_script`).

# Repository Hygiene

- Do not commit environment-specific build configuration (e.g., `.cargo/config.toml` with platform-specific linker settings) to the main codebase. These files break builds on other architectures. Use environment variables, CI-specific overlays, or document the setup in `AGENTS.md` instead.
- Development workflow artifacts (AI prompts, iteration scripts, implementation plans, synthesis documents) do not belong in the repository. Keep them in local directories excluded by `.gitignore`, or remove them before merging.
