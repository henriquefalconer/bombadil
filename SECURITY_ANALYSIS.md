# Security Analysis: Header Forwarding Fix

Complete analysis of fundamental problems and risky assumptions in the header forwarding changes introduced in `main` relative to `antithesishq/main`.

---

## Fundamental Problems and Risky Assumptions

### P1. The stripped-header list may be incomplete

The filter strips `etag`, `content-length`, `content-encoding`, `transfer-encoding`. HTTP defines additional hop-by-hop and transport-layer headers that become invalid after CDP decompresses and Bombadil re-instruments the body:

- `Content-Range` (meaningless if the original response was a 206 partial)
- `Connection` and headers it names (RFC 7230 hop-by-hop)
- `Keep-Alive`, `TE`, `Trailer`, `Upgrade` (hop-by-hop per spec)
- `Content-Security-Policy` / `Content-Security-Policy-Report-Only` (hash/nonce directives become invalid after instrumentation changes the body)
- `Strict-Transport-Security` (could pin HSTS on localhost during testing)

**Assessment: Direct result of the new code. Cannot be disregarded.**

The upstream forwarded zero headers (`// TODO: forward headers`), so there was nothing problematic to forward. The new code introduced the decision of which headers to forward and which to strip. The correctness and completeness of that filter is the new code's responsibility.

### P2. Test timeout equals LTL timeout, creating a race

Both new tests use `Duration::from_secs(10)` for the tokio test timeout and `.within(10, "seconds")` for the LTL property. The test harness treats `Timeout` as `Success`:

```rust
(Outcome::Timeout, Expect::Success) => {}  // passes silently
```

The tokio clock and the LTL step clock are independent. If the runner loop is slow (slow page load, slow CDP, slow CI), the LTL clock falls behind and the tokio timer fires first, causing the test to pass without the LTL property ever reaching a conclusion.

**Assessment: Direct result of the new code. Cannot be disregarded.**

No existing upstream test has this race. All upstream tests with `.within()` bounds use a test timeout strictly greater than the LTL bound (e.g., `test_back_from_non_html`: 30s test / 20s LTL).

### P3. When `response_headers` is `None`, only an `etag` is sent

If `event.response_headers` is `None`, the `.iter().flatten()` chain yields nothing. The fulfilled response would contain only the synthetic `etag` — no `Content-Type`, no `Cache-Control`, nothing else. Chrome could reject the response (especially for ES modules requiring a valid MIME type).

**Assessment: Can be disregarded.**

The upstream code always sent only `etag` for every response, regardless of whether headers were present. The new code is strictly better: it forwards what's available and falls back to the same etag-only behavior when there's nothing. Bombadil intercepts at `RequestStage::Response` (line 23), so `response_headers` is populated by CDP in practice. The existing content-type detection code (line 91-104) already treats `None` headers as a theoretical edge case with `.as_ref().and_then()`.

### P4. `Content-Security-Policy` script hashes become stale

If the original response carries a CSP header with `script-src` hash directives (`sha256-...`), those hashes were computed against the original script body. After instrumentation, the body is different, and Chrome will block the script.

**Assessment: This is a consequence of P1 (incomplete filter). The specific header was already considered there.**

The upstream was already replacing the entire body with instrumented code and sending it with no headers at all. CSP hash mismatches were already a certainty for inline scripts, but for external scripts the upstream accidentally avoided CSP enforcement by dropping all headers. The new code re-introduces CSP enforcement by forwarding the header, making it a regression for CSP-protected apps. See P1 for full consequence analysis.

### P5. `Cache-Control` / caching semantics silently change

The original `etag` is stripped and replaced with `source_id`. Related caching headers (`Cache-Control`, `Last-Modified`, `Expires`, `Age`) are forwarded as-is. A proxy or service worker cache might store the instrumented body keyed by the synthetic etag.

**Assessment: Can be disregarded.**

The upstream already replaced the etag with `source_id` and never forwarded `Cache-Control`. The new code preserves `Cache-Control` (better than before) and still replaces etag (same as before). In the context of a fuzzing tool that instruments every intercepted response, caching coherence is not a goal.

### P6. Non-200 responses skip instrumentation but are not header-audited

The early return for `status != 200` uses `ContinueRequestParams` (pass-through). Redirects (301/302), 304 Not Modified, and partial content (206) all bypass the fix entirely.

**Assessment: Can be disregarded.**

This is pre-existing behavior untouched by the new code. `ContinueRequestParams` is a pass-through that lets the browser handle the response normally, which is correct for non-200 responses.

### P7. Per-header array allocation in the filter

The `["etag", "content-length", "content-encoding", "transfer-encoding"]` array is constructed for each header being filtered.

**Assessment: Can be disregarded.**

Trivially cheap (4-element stack array, 5-20 headers per response). The upstream codebase uses similar inline patterns throughout (e.g., the `eq_ignore_ascii_case` check for content-type detection on line 98).

### P8. No new `Content-Length` is set for the instrumented body

`Content-Length` is stripped but never re-added. CDP's `Fetch.fulfillRequest` must infer the content length from the `body` parameter.

**Assessment: Can be disregarded.**

The upstream also never set `Content-Length`. CDP's `Fetch.fulfillRequest` delivers the body directly to the browser engine — it does not go over HTTP. The `body` parameter is base64-encoded and CDP handles length internally. `Content-Length` is a transport concern that CDP abstracts away.

---

## Summary

| Problem | Direct result of new code? | Can be disregarded? |
|---------|---------------------------|-------------------|
| P1. Incomplete header filter | Yes | **No** |
| P2. Test timeout = LTL timeout | Yes | **No** |
| P3. `None` headers → etag only | No (pre-existing) | Yes |
| P4. CSP hashes stale | Subsumed by P1 | **No** (see P1) |
| P5. Caching semantics | No (pre-existing) | Yes |
| P6. Non-200 bypass | No (pre-existing) | Yes |
| P7. Per-header allocation | No (style only) | Yes |
| P8. No Content-Length | No (pre-existing) | Yes |

---

## Consequence Analysis for Non-Disregardable Problems

### P1. Incomplete Header Filter — Consequences in Depth

**What changed semantically:** The upstream sent `responseHeaders: [{"name":"etag","value":"..."}]` — telling Chrome "this response has exactly one header." The new code sends all original headers minus four, telling Chrome "this response has all the original headers (minus transport metadata)." This is a fundamental semantic shift from "discard everything" to "forward everything except a known blocklist."

**Silent instrumentation failure on CSP-protected apps:**

When a server sends CSP with hash-based or nonce-based `script-src` restrictions:
1. The hash/nonce was computed for the original script body.
2. Bombadil replaces the body with an instrumented version.
3. The original CSP header is forwarded unchanged.
4. Chrome enforces CSP, finds the mismatch, blocks the instrumented script.

This failure is **silent**: Bombadil navigates, takes actions, records traces, but no JS coverage is collected. A user could complete an entire fuzz campaign against a CSP-protected app with zero meaningful results and no error. CSP violations appear in the browser console but are not JS exceptions or `console.error` calls — they would not trigger `noUncaughtExceptions` or `noConsoleErrors` default properties.

The upstream behavior accidentally avoided this by dropping all headers (no CSP header = no CSP enforcement). The new code is a regression for CSP-protected apps.

**HSTS pinning on localhost:**

If the origin server sends `Strict-Transport-Security`, forwarding it could cause Chrome to pin HSTS for localhost, affecting subsequent test runs or local development.

**The blocklist is inherently fragile:**

A blocklist means every new problematic header must be discovered and added. An allowlist (forwarding only known-safe headers) would be safer but might miss headers that specific apps need. The current code makes this architectural choice implicitly.

**Interaction with non-200 early return:**

Non-200 responses use `ContinueRequestParams` (pass-through) and never hit the header filter. This is correct for 301/302/304 responses (no body to instrument), but means the two code paths have different header semantics.

**Real-world deployment:**
- Apps **without CSP hash/nonce**: Fix works correctly. Modules load, compressed scripts work, other headers are benignly preserved. Net improvement.
- Apps **with CSP hash/nonce**: Fix actively breaks instrumentation. Worse than upstream behavior. Silent failure mode.

### P2. Test Timeout Race — Consequences in Depth

**The two independent clocks:**
- LTL clock: ticks only when the verifier steps (requires a full runner cycle: state capture, extraction, verification, action).
- Tokio clock: ticks continuously.

If the runner loop is slow, the LTL clock falls behind. With both timeouts at 10 seconds:
- The tokio timer fires at wall-clock 10s.
- The LTL clock may be at 9s or less.
- `Outcome::Timeout` maps to success.
- The test passes without the LTL `eventually` ever resolving.

**When this matters most:**
- Slow CI environments (loaded machines, resource contention) make the runner cycle slower, widening the gap between the two clocks.
- A regression that breaks script loading would not be caught: the `#result` element stays "WAITING", the LTL property needs its full 10s to produce a violation, but the tokio timeout fires first.

**Comparison with upstream:**
- `test_back_from_non_html`: 30s test / 20s LTL (10s buffer)
- `test_counter_state_machine`: 3s test / `always(...)` with no `.within()` (violations on first step, no race possible)
- `test_random_text_input`: 120s test / 10s LTL (110s buffer)

The new tests are the only ones where both timeouts are equal.

**Practical note:** In the happy path, the script loads in under 1 second. The 10-second values are generous bounds. The race would only trigger when the fix is broken (exactly when detection matters) or under extreme CI load. This makes it a reliability concern for regression detection, not for day-to-day passing tests.
