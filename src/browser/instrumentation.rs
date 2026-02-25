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
                        .response_headers(build_response_headers(
                            &event.response_headers,
                            &resource_type,
                            source_id,
                        ))
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
/// values are invalidated by script body instrumentation. When neither `script-src` nor
/// `script-src-elem` is present, browsers fall back to `default-src` for script-loading
/// decisions, so `default-src` hashes/nonces are stripped in that case too.
///
/// `'strict-dynamic'` is also removed from any directive whose hashes/nonces are
/// stripped: without a trust anchor it has no effect and would block all scripts.
///
/// `report-uri` and `report-to` directives are stripped entirely to prevent
/// instrumentation-triggered mutations from sending false-positive CSP violation
/// reports to the application's reporting endpoint.
///
/// If a processed directive contained only hash/nonce values (plus optionally
/// `'strict-dynamic'`), it is omitted entirely rather than left empty.
///
/// Returns `None` when every directive was stripped (the caller should omit the header).
fn sanitize_csp(csp_value: &str) -> Option<String> {
    // Collect non-empty directives and detect whether any explicit script-src /
    // script-src-elem directive is present (needed for default-src fallback logic).
    let directives: Vec<&str> = csp_value
        .split(';')
        .map(str::trim)
        .filter(|d| !d.is_empty())
        .collect();

    let has_script_src = directives.iter().any(|d| {
        let lower = d.to_lowercase();
        lower.starts_with("script-src ")
            || lower == "script-src"
            || lower.starts_with("script-src-elem ")
            || lower == "script-src-elem"
    });

    let mut result: Vec<String> = Vec::new();

    for directive in directives {
        let lower = directive.to_lowercase();

        // Strip report-uri / report-to entirely — instrumentation activity must not
        // trigger false-positive violation reports to the application's endpoint.
        let directive_name_end =
            lower.find(char::is_whitespace).unwrap_or(lower.len());
        let directive_name = &lower[..directive_name_end];
        if directive_name == "report-uri" || directive_name == "report-to" {
            continue;
        }

        let is_script_src = lower.starts_with("script-src ")
            || lower == "script-src"
            || lower.starts_with("script-src-elem ")
            || lower == "script-src-elem";

        // Apply hash/nonce stripping to default-src only when no explicit script-src /
        // script-src-elem is present (browser would fall back to default-src for scripts).
        let is_default_src_fallback = !has_script_src
            && (lower.starts_with("default-src ") || lower == "default-src");

        if is_script_src || is_default_src_fallback {
            let mut parts = directive.splitn(2, char::is_whitespace);
            let name = parts.next().unwrap_or("");
            let values_str = parts.next().unwrap_or("").trim();

            // Remove hashes, nonces, and 'strict-dynamic' (which is meaningless
            // without a trust anchor and blocks all scripts when left alone).
            let filtered: Vec<&str> = values_str
                .split_whitespace()
                .filter(|v| {
                    let lv = v.to_lowercase();
                    !lv.starts_with("'sha256-")
                        && !lv.starts_with("'sha384-")
                        && !lv.starts_with("'sha512-")
                        && !lv.starts_with("'nonce-")
                        && lv != "'strict-dynamic'"
                })
                .collect();

            if !filtered.is_empty() {
                result.push(format!("{} {}", name, filtered.join(" ")));
            }
            // If all values were hashes/nonces/'strict-dynamic', omit the directive.
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

/// Build the response header list for a fulfilled CDP request.
///
/// Strips headers invalidated by instrumentation (see [`STRIPPED_RESPONSE_HEADERS`]),
/// applies resource-type-aware CSP handling, and appends a synthetic `etag` derived
/// from `source_id`.
///
/// CSP stripping is resource-type-aware:
/// - `Script`: the whole CSP header is dropped (script body instrumentation
///   invalidates all hash-based `script-src` values).
/// - `Document`: the header is sanitised via [`sanitize_csp`] (non-hash directives
///   like `img-src`, `frame-ancestors`, `connect-src` are preserved).
/// - Other resource types: CSP headers are forwarded unchanged.
fn build_response_headers(
    response_headers: &Option<Vec<fetch::HeaderEntry>>,
    resource_type: &network::ResourceType,
    source_id: SourceId,
) -> Vec<fetch::HeaderEntry> {
    response_headers
        .iter()
        .flatten()
        .filter(|h| {
            !STRIPPED_RESPONSE_HEADERS
                .iter()
                .any(|name| h.name.eq_ignore_ascii_case(name))
        })
        .flat_map(|h| {
            let is_csp = h.name.eq_ignore_ascii_case("content-security-policy")
                || h.name.eq_ignore_ascii_case(
                    "content-security-policy-report-only",
                );
            if is_csp {
                match resource_type {
                    network::ResourceType::Script => None,
                    network::ResourceType::Document => sanitize_csp(&h.value)
                        .map(|v| fetch::HeaderEntry {
                            name: h.name.clone(),
                            value: v,
                        }),
                    _ => Some(h.clone()),
                }
            } else {
                Some(h.clone())
            }
        })
        .chain(std::iter::once(fetch::HeaderEntry {
            name: "etag".to_string(),
            value: format!("{}", source_id.0),
        }))
        .collect()
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

    #[test]
    fn sanitize_csp_default_src_hash_stripped_when_no_script_src() {
        // No script-src/script-src-elem present → default-src hashes must be stripped.
        assert_eq!(
            sanitize_csp("default-src 'sha256-abc' 'self'"),
            Some("default-src 'self'".to_string())
        );
    }

    #[test]
    fn sanitize_csp_default_src_not_touched_when_script_src_present() {
        // Explicit script-src is present → default-src is NOT touched.
        assert_eq!(
            sanitize_csp(
                "default-src 'sha256-abc' 'self'; script-src 'unsafe-inline'"
            ),
            Some(
                "default-src 'sha256-abc' 'self'; script-src 'unsafe-inline'"
                    .to_string()
            )
        );
    }

    #[test]
    fn sanitize_csp_default_src_only_hashes_omitted_when_no_script_src() {
        // All values are hashes → directive is omitted entirely.
        assert_eq!(sanitize_csp("default-src 'sha256-abc'"), None);
    }

    #[test]
    fn sanitize_csp_strict_dynamic_removed_with_nonce() {
        // Nonce stripped → 'strict-dynamic' loses its trust anchor and is removed too.
        assert_eq!(
            sanitize_csp("script-src 'nonce-abc' 'strict-dynamic'"),
            None
        );
    }

    #[test]
    fn sanitize_csp_strict_dynamic_removed_keeps_other_values() {
        assert_eq!(
            sanitize_csp("script-src 'nonce-abc' 'strict-dynamic' 'self'"),
            Some("script-src 'self'".to_string())
        );
    }

    #[test]
    fn sanitize_csp_strict_dynamic_removed_with_hash() {
        assert_eq!(
            sanitize_csp("script-src 'sha256-abc' 'strict-dynamic'"),
            None
        );
    }

    #[test]
    fn sanitize_csp_strips_report_uri() {
        assert_eq!(
            sanitize_csp(
                "script-src 'sha256-abc' 'self'; report-uri /csp-report"
            ),
            Some("script-src 'self'".to_string())
        );
    }

    #[test]
    fn sanitize_csp_strips_report_to() {
        assert_eq!(
            sanitize_csp("script-src 'self'; report-to csp-group"),
            Some("script-src 'self'".to_string())
        );
    }

    #[test]
    fn sanitize_csp_strips_both_report_directives() {
        assert_eq!(
            sanitize_csp("default-src 'self'; report-uri /r; report-to g"),
            Some("default-src 'self'".to_string())
        );
    }

    fn hdr(name: &str, value: &str) -> fetch::HeaderEntry {
        fetch::HeaderEntry {
            name: name.to_string(),
            value: value.to_string(),
        }
    }

    fn sid(n: u32) -> SourceId {
        SourceId::hash(&n.to_string())
    }

    #[test]
    fn build_headers_strips_stripped_headers() {
        // All STRIPPED_RESPONSE_HEADERS must be absent from the output.
        let headers = Some(vec![
            hdr("etag", "\"upstream\""),
            hdr("content-length", "1234"),
            hdr("content-encoding", "gzip"),
            hdr("transfer-encoding", "chunked"),
            hdr("digest", "sha-256=abc"),
            hdr("content-type", "text/javascript"),
        ]);
        let result = build_response_headers(
            &headers,
            &network::ResourceType::Script,
            sid(1),
        );
        let names: Vec<&str> = result.iter().map(|h| h.name.as_str()).collect();
        for stripped in STRIPPED_RESPONSE_HEADERS {
            // The synthetic etag is allowed; it is the only etag in the output.
            if *stripped == "etag" {
                continue;
            }
            assert!(
                !names.iter().any(|n| n.eq_ignore_ascii_case(stripped)),
                "header {stripped} should have been stripped"
            );
        }
    }

    #[test]
    fn build_headers_preserves_content_type() {
        // content-type is not in STRIPPED_RESPONSE_HEADERS and must pass through.
        // This verifies the fix for the module-script issue (the original root cause
        // was content-type being inadvertently dropped).
        let headers =
            Some(vec![hdr("content-type", "text/javascript; charset=utf-8")]);
        let result = build_response_headers(
            &headers,
            &network::ResourceType::Script,
            sid(2),
        );
        assert!(
            result.iter().any(|h| h.name == "content-type"
                && h.value == "text/javascript; charset=utf-8"),
            "content-type must be preserved"
        );
    }

    #[test]
    fn build_headers_drops_csp_for_script_resources() {
        let headers = Some(vec![
            hdr("content-security-policy", "script-src 'self'"),
            hdr("content-type", "text/javascript"),
        ]);
        let result = build_response_headers(
            &headers,
            &network::ResourceType::Script,
            sid(3),
        );
        assert!(
            !result.iter().any(|h| h
                .name
                .eq_ignore_ascii_case("content-security-policy")),
            "CSP must be dropped for Script resources"
        );
    }

    #[test]
    fn build_headers_sanitizes_csp_for_document_resources() {
        let headers = Some(vec![hdr(
            "content-security-policy",
            "script-src 'sha256-abc' 'self'; img-src 'self'",
        )]);
        let result = build_response_headers(
            &headers,
            &network::ResourceType::Document,
            sid(4),
        );
        let csp = result
            .iter()
            .find(|h| h.name.eq_ignore_ascii_case("content-security-policy"))
            .expect("sanitized CSP must be present for Document resources");
        assert_eq!(csp.value, "script-src 'self'; img-src 'self'");
    }

    #[test]
    fn build_headers_drops_report_only_csp_for_script_resources() {
        let headers = Some(vec![
            hdr("content-security-policy-report-only", "script-src 'self'"),
            hdr("content-type", "text/javascript"),
        ]);
        let result = build_response_headers(
            &headers,
            &network::ResourceType::Script,
            sid(5),
        );
        assert!(
            !result.iter().any(|h| h
                .name
                .eq_ignore_ascii_case("content-security-policy-report-only")),
            "report-only CSP must be dropped for Script resources"
        );
    }

    #[test]
    fn build_headers_sanitizes_report_only_csp_for_document_resources() {
        let headers = Some(vec![hdr(
            "content-security-policy-report-only",
            "script-src 'sha256-abc' 'self'; img-src 'self'",
        )]);
        let result = build_response_headers(
            &headers,
            &network::ResourceType::Document,
            sid(6),
        );
        let csp = result
            .iter()
            .find(|h| {
                h.name
                    .eq_ignore_ascii_case("content-security-policy-report-only")
            })
            .expect(
                "sanitized report-only CSP must be present for Document \
                 resources",
            );
        assert_eq!(csp.value, "script-src 'self'; img-src 'self'");
    }

    #[test]
    fn build_headers_appends_synthetic_etag() {
        let source = sid(42);
        let result = build_response_headers(
            &None,
            &network::ResourceType::Script,
            source,
        );
        let etag = result
            .iter()
            .find(|h| h.name == "etag")
            .expect("synthetic etag must always be present");
        assert_eq!(etag.value, format!("{}", source.0));
    }

    #[test]
    fn build_headers_none_headers_yields_only_synthetic_etag() {
        let result = build_response_headers(
            &None,
            &network::ResourceType::Script,
            sid(7),
        );
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].name, "etag");
    }

    #[test]
    fn build_headers_non_csp_non_stripped_pass_through() {
        let headers = Some(vec![
            hdr("x-custom-header", "keep-me"),
            hdr("cache-control", "no-cache"),
        ]);
        let result = build_response_headers(
            &headers,
            &network::ResourceType::Script,
            sid(8),
        );
        assert!(
            result
                .iter()
                .any(|h| h.name == "x-custom-header" && h.value == "keep-me")
        );
        assert!(
            result
                .iter()
                .any(|h| h.name == "cache-control" && h.value == "no-cache")
        );
    }
}
