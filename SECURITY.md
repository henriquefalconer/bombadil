# Security Implications of Header-Forwarding Changes

## CSP Stripping Removes Full Page Security Policy on Documents

The `content-security-policy` header is stripped from **all** fulfilled responses,
including HTML documents. For script resources, this is correct — instrumentation
changes the body, so script-hash digests become invalid. But for document
responses, the CSP header governs the **entire page's security policy**:
`frame-ancestors`, `connect-src`, `img-src`, `style-src`, `font-src`, `base-uri`,
`form-action`, `navigate-to`, and more.

Stripping it means Bombadil-tested pages run with **no CSP enforcement at all**.
Consequences:

- XHR/fetch to origins that CSP would block will succeed under Bombadil but fail
  in production, producing both false positives and missed bugs.
- Iframes from blocked origins will load.
- Inline styles that CSP blocks will render.
- The fuzz session observes a different application than real users see.

The correct approach is to parse the CSP value and only remove hash-based or
nonce-based directives within `script-src`, preserving all other directives.
The same reasoning applies to `content-security-policy-report-only`.

## HSTS Stripping Is Unconditional

The `strict-transport-security` header is stripped for every response, not only
for localhost or ephemeral test sessions. When Bombadil fuzzes a real HTTPS
origin:

- The browser will not enforce HTTPS for subsequent navigations within the run.
- Mixed-content loads that HSTS would block will succeed.
- HTTP-downgrade links that would be auto-upgraded in production will navigate
  to insecure origins.

Since Bombadil uses ephemeral browser profiles (`TempDir`), HSTS state does not
persist across runs. But within a single run, the browser would normally learn
and enforce HSTS after the first response; stripping it removes that intra-run
protection and creates a fidelity gap for HTTPS targets.

## Denylist May Be Incomplete

Headers whose validity depends on body content that are **not** currently
stripped:

| Header | Risk |
|--------|------|
| `digest` (RFC 3230 / RFC 9530) | Contains a hash of the response body. After instrumentation, validation against this hash fails. Rare in practice. |
| `age`, `last-modified`, `expires` | Stale cache metadata for the instrumented body. Low risk because Bombadil replaces the `etag` and uses ephemeral profiles. |

These are unlikely to cause problems in typical test targets but represent
unhandled edge cases.

## Non-HTML Document Bodies Are Unchanged but Headers Are Still Filtered

When a non-HTML document (XML, PDF) is intercepted, the body is passed through
unchanged (`body.clone()`), but `content-length`, `content-encoding`, and
`transfer-encoding` are still stripped. This is not a regression from upstream
(which dropped **all** headers), but it is unnecessary work that could
theoretically cause issues if a downstream consumer relies on those headers for
unmodified bodies.
