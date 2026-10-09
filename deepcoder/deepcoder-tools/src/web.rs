//! Web tools.

use async_trait::async_trait;
use deepcoder_error::{DeepCoderError, DeepCoderResult};
use deepcoder_types::tool::{JsonToolOutput, ToolSpec};
use regex::Regex;
use serde_json::json;
use std::time::Duration;

use crate::{Tool, ToolContext};

const DEFAULT_WEB_TIMEOUT_MS: u64 = 30_000;
const DEFAULT_SEARCH_URL: &str = "https://duckduckgo.com/html/";

pub struct WebFetchTool;

#[async_trait]
impl Tool for WebFetchTool {
    fn name(&self) -> &'static str {
        "web_fetch"
    }

    fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: self.name().into(),
            description: "Fetch a URL and return response text plus status metadata.".into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "url": {"type": "string"},
                    "max_bytes": {"type": "integer", "minimum": 1},
                    "timeout_ms": {"type": "integer", "minimum": 1}
                },
                "required": ["url"],
                "additionalProperties": false
            }),
        }
    }

    fn is_concurrency_safe(&self) -> bool {
        true
    }

    async fn call(
        &self,
        params: serde_json::Value,
        _ctx: &ToolContext,
    ) -> DeepCoderResult<JsonToolOutput> {
        let url = required_str(&params, "url")?;
        let max_bytes = params
            .get("max_bytes")
            .and_then(|v| v.as_u64())
            .unwrap_or(256 * 1024) as usize;
        let timeout_ms = params
            .get("timeout_ms")
            .and_then(|v| v.as_u64())
            .unwrap_or(DEFAULT_WEB_TIMEOUT_MS);
        let client = build_web_client(url, timeout_ms)?;
        let response = client.get(url).send().await?;
        let status = response.status().as_u16();
        let final_url = response.url().to_string();
        let mut text = response.text().await?;
        if text.len() > max_bytes {
            text.truncate(max_bytes);
        }
        Ok(JsonToolOutput::success(json!({
            "url": url,
            "final_url": final_url,
            "status": status,
            "content": text
        })))
    }
}

pub struct WebSearchTool;

#[async_trait]
impl Tool for WebSearchTool {
    fn name(&self) -> &'static str {
        "web_search"
    }

    fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: self.name().into(),
            description: "Search the web using DuckDuckGo HTML results and return titles and URLs."
                .into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "query": {"type": "string"},
                    "max_results": {"type": "integer", "minimum": 1},
                    "timeout_ms": {"type": "integer", "minimum": 1}
                },
                "required": ["query"],
                "additionalProperties": false
            }),
        }
    }

    fn is_concurrency_safe(&self) -> bool {
        true
    }

    async fn call(
        &self,
        params: serde_json::Value,
        _ctx: &ToolContext,
    ) -> DeepCoderResult<JsonToolOutput> {
        let query = required_str(&params, "query")?;
        let max_results = params
            .get("max_results")
            .and_then(|v| v.as_u64())
            .unwrap_or(5) as usize;
        let timeout_ms = params
            .get("timeout_ms")
            .and_then(|v| v.as_u64())
            .unwrap_or(DEFAULT_WEB_TIMEOUT_MS);
        let base_url = params
            .get("base_url")
            .and_then(|v| v.as_str())
            .unwrap_or(DEFAULT_SEARCH_URL);
        let url = search_url(base_url, query)?;
        let client = build_web_client(base_url, timeout_ms)?;
        let html = client.get(&url).send().await?.text().await?;
        let results = extract_duckduckgo_results(&html, max_results)?;
        Ok(JsonToolOutput::success(json!({
            "query": query,
            "results": results
        })))
    }
}

fn required_str<'a>(params: &'a serde_json::Value, name: &str) -> DeepCoderResult<&'a str> {
    params
        .get(name)
        .and_then(|v| v.as_str())
        .filter(|s| !s.trim().is_empty())
        .ok_or_else(|| DeepCoderError::ToolExecution(format!("missing required string: {name}")))
}

fn search_url(base_url: &str, query: &str) -> DeepCoderResult<String> {
    let mut url = reqwest::Url::parse(base_url)
        .map_err(|e| DeepCoderError::ToolExecution(format!("invalid search base_url: {e}")))?;
    url.query_pairs_mut().append_pair("q", query);
    Ok(url.to_string())
}

fn build_web_client(base_url: &str, timeout_ms: u64) -> DeepCoderResult<reqwest::Client> {
    let mut builder = reqwest::Client::builder().timeout(Duration::from_millis(timeout_ms));
    if is_loopback_url(base_url) {
        builder = builder.no_proxy();
    }
    Ok(builder.build()?)
}

fn is_loopback_url(url: &str) -> bool {
    reqwest::Url::parse(url)
        .ok()
        .and_then(|url| url.host_str().map(is_loopback_host))
        .unwrap_or(false)
}

fn is_loopback_host(host: &str) -> bool {
    let host = host.trim_matches(&['[', ']'][..]);
    host.eq_ignore_ascii_case("localhost")
        || host == "::1"
        || host
            .parse::<std::net::IpAddr>()
            .is_ok_and(|addr| addr.is_loopback())
}

fn extract_duckduckgo_results(
    html: &str,
    max_results: usize,
) -> DeepCoderResult<Vec<serde_json::Value>> {
    let link_re = Regex::new(r#"<a[^>]*class="result__a"[^>]*href="([^"]+)"[^>]*>(.*?)</a>"#)
        .map_err(|e| DeepCoderError::ToolExecution(e.to_string()))?;
    let tag_re =
        Regex::new(r"<[^>]+>").map_err(|e| DeepCoderError::ToolExecution(e.to_string()))?;
    let mut results = Vec::new();
    for cap in link_re.captures_iter(html).take(max_results) {
        let result_url = html_unescape(&cap[1]);
        let title = html_unescape(&tag_re.replace_all(&cap[2], ""));
        results.push(json!({"title": title, "url": result_url}));
    }
    Ok(results)
}

fn html_unescape(input: &str) -> String {
    input
        .replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#x27;", "'")
}
