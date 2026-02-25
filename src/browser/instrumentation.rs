use anyhow::{Context, Result, anyhow, bail};
use base64::Engine;
use base64::prelude::BASE64_STANDARD;
use chromiumoxide::Page;
use chromiumoxide::cdp::browser_protocol::fetch;
use chromiumoxide::cdp::browser_protocol::network;
use futures::StreamExt;
use log;
use oxc::span::SourceType;
use serde_json as json;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::spawn;

use crate::instrumentation;
use crate::instrumentation::source_id::SourceId;

/// Response headers that must be stripped after script instrumentation.
///
/// Each entry is lower-cased for case-insensitive matching.
///
/// Note: `content-security-policy` and `content-security-policy-report-only` are NOT
/// listed here. CSP stripping is resource-type-aware: for Script responses the whole
/// header is dropped (script body instrumentation invalidates hash-based `script-src`
/// values); for Document responses the header is sanitised via [`sanitize_csp`] instead
/// of being removed wholesale. See the `FulfillRequestParams` construction below.
const STRIPPED_RESPONSE_HEADERS: &[&str] = &[
    // Replaced with an instrumentation-stable source ID derived from the
    // original ETag or body hash, so the upstream value is always stale.
    "etag",
    // Body size changes when we rewrite the script, so the declared length
    // no longer matches the actual bytes sent.
    "content-length",
    // CDP already returns a decompressed body; re-advertising a compression
    // encoding would cause the browser to double-decompress.
    "content-encoding",
    // Same reason as content-encoding: the transfer framing is gone once CDP
    // hands us the raw bytes.
    "transfer-encoding",
    // The Digest header (RFC 3230 / RFC 9530) contains a hash of the response
    // body. After instrumentation that hash is wrong; a service worker
    // validating it would reject the instrumented script.
    "digest",
];

pub async fn instrument_js_coverage(page: Arc<Page>) -> Result<()> {
    page.execute(
        fetch::EnableParams::builder()
            .pattern(
                fetch::RequestPattern::builder()
                    .request_stage(fetch::RequestStage::Response)
                    .resource_type(network::ResourceType::Script)
                    .build(),
            )
            .pattern(
                fetch::RequestPattern::builder()
                    .request_stage(fetch::RequestStage::Response)
                    .resource_type(network::ResourceType::Document)
                    .build(),
            )
            .build(),
    )
    .await
    .context("failed enabling request interception")?;

    let mut events = page.event_listener::<fetch::EventRequestPaused>().await?;

    let _handle = spawn(async move {
        let intercept =
            async |event: &fetch::EventRequestPaused| -> Result<()> {
                // Any non-200 upstream response is forwarded as-is.
                if let Some(status) = event.response_status_code
                    && status != 200
                {
                    return page
                        .execute(
                            fetch::ContinueRequestParams::builder()
                                .request_id(event.request_id.clone())
                                .build()
                                .map_err(|error| {
                                    anyhow!(
                                    "failed building ContinueRequestParams: {}",
                                    error
                                )
                                })?,
                        )
                        .await
                        .map(|_| ())
                        .context("failed continuing request");
                }

                let headers: HashMap<String, String> =
                    json::from_value(event.request.headers.inner().clone())?;

                let body_response = page
                    .execute(
                        fetch::GetResponseBodyParams::builder()
                            .request_id(event.request_id.clone())
                            .build()
                            .map_err(|error| {
                                anyhow!(
                                    "failed building GetResponseBodyParams: {}",
                                    error
                                )
                            })?,
                    )
                    .await
                    .context("failed getting response body")?;

                let body = if body_response.base64_encoded {
                    let bytes = body_response.body.as_bytes();
                    String::from_utf8(BASE64_STANDARD.decode(bytes)?)?
                } else {
                    body_response.body.clone()
                };

                let source_id = source_id(headers, &body);

                let is_html_document = event.resource_type
                    == network::ResourceType::Document
                    && event
                        .response_headers
                        .as_ref()
                        .and_then(|headers| {
                            headers.iter().find(|h| {
                                h.name.eq_ignore_ascii_case("content-type")
                            })
                        })
                        .map(|h| h.value.starts_with("text/html"))
                        .unwrap_or_else(|| {
                            !body.trim_start().starts_with("<?xml")
                        });

                let body_instrumented = if event.resource_type
                    == network::ResourceType::Script
                {
                    let instrumented =
                        instrumentation::js::instrument_source_code(
                            source_id,
                            &body,
                            // As we can't know if the script is an ES module or a regular script,
                            // we use this source type to let the parser decide.
                            SourceType::unambiguous(),
                        )?;

                    // Write to /tmp/ for debugging
                    if let Some(filename) =
                        event.request.url.split('/').next_back()
                    {
                        let safe_filename =
                            filename.replace(['?', '#', '&', '='], "_");
                        let path = format!("/tmp/{}", safe_filename);
                        if let Err(e) =
                            tokio::fs::write(&path, &instrumented).await
                        {
                            log::debug!(
                                "failed to write debug file to {}: {}",
                                path,
                                e
                            );
                        } else {
                            log::debug!(
                                "wrote instrumented script to {}",
                                path
                            );
                        }
                    }

                    instrumented
                } else if is_html_document {
                    instrumentation::html::instrument_inline_scripts(
                        source_id, &body,
                    )?
                } else if event.resource_type == network::ResourceType::Document
                {
                    // Non-HTML documents (XML, PDF, etc.) are passed
                    // through without instrumentation.
                    body.clone()
                } else {
                    bail!(
                        "should only intercept script and document resources, but got {:?}",
                        event.resource_type
                    );
                };

                // Capture resource type before the iterator borrows `event`.
                let resource_type = event.resource_type.clone();

                page.execute(
                    fetch::FulfillRequestParams::builder()
                        .request_id(event.request_id.clone())
                        .body(BASE64_STANDARD.encode(body_instrumented))
                        .response_code(200)
                        .response_headers(
                            event
                                .response_headers
                                .iter()
                                .flatten()
                                .filter(|h| {
                                    !STRIPPED_RESPONSE_HEADERS.iter().any(
                                        |name| {
                                            h.name.eq_ignore_ascii_case(name)
                                        },
                                    )
                                })
                                .flat_map(move |h| {
                                    // CSP headers require resource-type-aware
                                    // handling: strip entirely for scripts
                                    // (instrumentation invalidates all hash
                                    // values), sanitise for documents (preserve
                                    // non-hash directives like img-src,
                                    // frame-ancestors, connect-src, …).
                                    let is_csp = h.name.eq_ignore_ascii_case(
                                        "content-security-policy",
                                    ) || h.name.eq_ignore_ascii_case(
                                        "content-security-policy-report-only",
                                    );
                                    if is_csp {
                                        match resource_type {
                                            network::ResourceType::Script => {
                                                None
                                            }
                                            _ => sanitize_csp(&h.value).map(
                                                |v| fetch::HeaderEntry {
                                                    name: h.name.clone(),
                                                    value: v,
                                                },
                                            ),
                                        }
                                    } else {
                                        Some(h.clone())
                                    }
                                })
                                .chain(std::iter::once(fetch::HeaderEntry {
                                    name: "etag".to_string(),
                                    value: format!("{}", source_id.0),
                                })),
                        )
                        .build()
                        .map_err(|error| {
                            anyhow!(
                                "failed building FulfillRequestParams: {}",
                                error
                            )
                        })?,
                )
                .await
                .context("failed fulfilling request")?;
                log::debug!(
                    "intercepted and instrumented request: {}",
                    event.request.url
                );
                Ok(())
            };
        while let Some(event) = events.next().await {
            if let Err(error) = intercept(&event).await {
                let error_debug = format!("{error:?}");
                if error_debug.contains("Invalid InterceptionId") {
                    log::debug!(
                        "interception invalidated (likely due to navigation): {}",
                        event.request.url
                    );
                    continue;
                }

                log::warn!("failed to instrument requested script: {error}");
                if let Err(error) = async {
                    let params = fetch::ContinueRequestParams::builder()
                        .request_id(event.request_id.clone())
                        .build()
                        .map_err(|error| anyhow!("{error}"))?;
                    page.execute(params)
                        .await
                        .map(|_| ())
                        .map_err(|error| anyhow!("{error}"))
                }
                .await
                {
                    log::warn!(
                        "failed continuing request after instrumentation failed: {error}"
                    );
                }
            }
        }
    });

    Ok(())
}

/// Calculate source ID from etag or body.
fn source_id(headers: HashMap<String, String>, body: &str) -> SourceId {
    if let Some(etag) = headers.get("etag") {
        SourceId::hash(etag)
    } else {
        SourceId::hash(body)
    }
}

/// Strip only instrumentation-sensitive values from a CSP header, preserving all other
/// directives.
///
/// Removes `'sha256-…'`, `'sha384-…'`, `'sha512-…'`, and `'nonce-…'` values from
/// `script-src` and `script-src-elem` directives — the only directives whose hash
/// values are invalidated by script body instrumentation. All other directives are
/// forwarded unchanged.
///
/// If a `script-src` or `script-src-elem` directive contained only hash or nonce values,
/// the directive is omitted entirely rather than left empty (an empty `script-src` would
/// block all scripts, which is worse than having no directive at all, since the browser
/// would fall back to `default-src`).
///
/// Returns `None` when every directive was stripped (the caller should omit the header).
fn sanitize_csp(csp_value: &str) -> Option<String> {
    let mut result: Vec<String> = Vec::new();
    for directive in csp_value.split(';') {
        let directive = directive.trim();
        if directive.is_empty() {
            continue;
        }
        let lower = directive.to_lowercase();
        let is_script_src = lower.starts_with("script-src ")
            || lower == "script-src"
            || lower.starts_with("script-src-elem ")
            || lower == "script-src-elem";
        if is_script_src {
            let mut parts = directive.splitn(2, char::is_whitespace);
            let name = parts.next().unwrap_or("");
            let values_str = parts.next().unwrap_or("").trim();
            let filtered: Vec<&str> = values_str
                .split_whitespace()
                .filter(|v| {
                    let lv = v.to_lowercase();
                    !lv.starts_with("'sha256-")
                        && !lv.starts_with("'sha384-")
                        && !lv.starts_with("'sha512-")
                        && !lv.starts_with("'nonce-")
                })
                .collect();
            if !filtered.is_empty() {
                result.push(format!("{} {}", name, filtered.join(" ")));
            }
            // If all values were hashes/nonces, omit the directive entirely.
        } else {
            result.push(directive.to_string());
        }
    }
    if result.is_empty() {
        None
    } else {
        Some(result.join("; "))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanitize_csp_removes_sha256() {
        assert_eq!(
            sanitize_csp("script-src 'sha256-abc123=' 'unsafe-inline'"),
            Some("script-src 'unsafe-inline'".to_string())
        );
    }

    #[test]
    fn sanitize_csp_removes_sha384() {
        assert_eq!(
            sanitize_csp("script-src 'sha384-abc123=' 'self'"),
            Some("script-src 'self'".to_string())
        );
    }

    #[test]
    fn sanitize_csp_removes_sha512() {
        assert_eq!(
            sanitize_csp("script-src 'sha512-abc123=' 'self'"),
            Some("script-src 'self'".to_string())
        );
    }

    #[test]
    fn sanitize_csp_removes_nonce() {
        assert_eq!(
            sanitize_csp("script-src 'nonce-xyz123' 'self'"),
            Some("script-src 'self'".to_string())
        );
    }

    #[test]
    fn sanitize_csp_mixed_directives() {
        assert_eq!(
            sanitize_csp("script-src 'sha256-abc' 'self'; img-src 'self'"),
            Some("script-src 'self'; img-src 'self'".to_string())
        );
    }

    #[test]
    fn sanitize_csp_no_script_src() {
        assert_eq!(
            sanitize_csp("img-src 'self'; frame-ancestors 'none'"),
            Some("img-src 'self'; frame-ancestors 'none'".to_string())
        );
    }

    #[test]
    fn sanitize_csp_empty_result() {
        assert_eq!(sanitize_csp("script-src 'sha256-abc'"), None);
    }

    #[test]
    fn sanitize_csp_multiple_hashes_with_safe_value() {
        assert_eq!(
            sanitize_csp(
                "script-src 'sha256-a' 'sha384-b' 'sha512-c' 'nonce-xyz' 'self'"
            ),
            Some("script-src 'self'".to_string())
        );
    }

    #[test]
    fn sanitize_csp_only_hash_directive_removed_others_kept() {
        assert_eq!(
            sanitize_csp("script-src 'sha256-a'; default-src 'self'"),
            Some("default-src 'self'".to_string())
        );
    }

    #[test]
    fn sanitize_csp_script_src_elem() {
        assert_eq!(
            sanitize_csp("script-src-elem 'sha256-abc' 'unsafe-inline'"),
            Some("script-src-elem 'unsafe-inline'".to_string())
        );
    }
}
