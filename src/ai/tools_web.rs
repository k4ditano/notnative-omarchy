use crate::ai::tools::ToolError;
use anyhow::Result;
use rig::tool::Tool;
use scraper::{Html, Selector};
use serde::Deserialize;

#[derive(Deserialize)]
pub struct WebSearchArgs {
    pub query: String,
}

pub struct WebSearch;

impl Tool for WebSearch {
    const NAME: &'static str = "web_search";

    type Args = WebSearchArgs;
    type Output = String;
    type Error = ToolError;

    async fn definition(&self, _prompt: String) -> rig::completion::ToolDefinition {
        rig::completion::ToolDefinition {
            name: "web_search".to_string(),
            description: "Search the internet for information using DuckDuckGo (No API key required). Use this to find up-to-date information, news, or data not in your training set.".to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "query": {
                        "type": "string",
                        "description": "The search query"
                    }
                },
                "required": ["query"]
            }),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        println!("🔧 [WebSearch] Searching for: '{}'", args.query);

        let result = tokio::task::spawn_blocking(move || -> anyhow::Result<String> {
            let client = reqwest::blocking::Client::builder()
                .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/91.0.4472.124 Safari/537.36")
                .build()
                .map_err(|e| anyhow::anyhow!("Failed to build HTTP client: {}", e))?;

            let response = client.get("https://html.duckduckgo.com/html/")
                .query(&[("q", &args.query)])
                .send()
                .map_err(|e| anyhow::anyhow!("Failed to execute search request: {}", e))?;

            if !response.status().is_success() {
                return Err(anyhow::anyhow!("Search request failed with status: {}", response.status()));
            }

            let html_content = response.text()
                .map_err(|e| anyhow::anyhow!("Failed to read response text: {}", e))?;

            let document = Html::parse_document(&html_content);

            // Selectors for DuckDuckGo HTML version
            let result_selector = Selector::parse(".result").unwrap();
            let title_selector = Selector::parse(".result__a").unwrap();
            let snippet_selector = Selector::parse(".result__snippet").unwrap();

            let mut results = Vec::new();

            for element in document.select(&result_selector).take(5) {
                let title = element.select(&title_selector).next()
                    .map(|e| e.text().collect::<Vec<_>>().join(" "))
                    .unwrap_or_else(|| "No title".to_string());

                let link = element.select(&title_selector).next()
                    .and_then(|e| e.value().attr("href"))
                    .unwrap_or("No link");

                let snippet = element.select(&snippet_selector).next()
                    .map(|e| e.text().collect::<Vec<_>>().join(" "))
                    .unwrap_or_else(|| "No snippet".to_string());

                if !title.is_empty() && link != "No link" {
                    results.push(format!("### [{}]({})\n{}", title, link, snippet));
                }
            }

            if results.is_empty() {
                Ok("No results found.".to_string())
            } else {
                Ok(results.join("\n\n"))
            }
        })
        .await
        .map_err(|e| ToolError(e.to_string()))??;

        Ok(result)
    }
}

impl WebSearch {
    pub fn new() -> Self {
        Self
    }
}

#[derive(Deserialize)]
pub struct FetchUrlArgs {
    pub url: String,
}

pub struct FetchUrl;

impl Tool for FetchUrl {
    const NAME: &'static str = "fetch_url";

    type Args = FetchUrlArgs;
    type Output = String;
    type Error = ToolError;

    async fn definition(&self, _prompt: String) -> rig::completion::ToolDefinition {
        rig::completion::ToolDefinition {
            name: "fetch_url".to_string(),
            description: "Fetch and extract text content from a specific URL. Use this when the user provides a link or when you need to read the full content of a search result.".to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "url": {
                        "type": "string",
                        "description": "The URL to fetch"
                    }
                },
                "required": ["url"]
            }),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        println!("🔧 [FetchUrl] Fetching: '{}'", args.url);

        let result = tokio::task::spawn_blocking(move || -> anyhow::Result<String> {
            let client = reqwest::blocking::Client::builder()
                .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/91.0.4472.124 Safari/537.36")
                .build()
                .map_err(|e| anyhow::anyhow!("Failed to build HTTP client: {}", e))?;

            let response = client.get(&args.url)
                .send()
                .map_err(|e| anyhow::anyhow!("Failed to fetch URL: {}", e))?;

            if !response.status().is_success() {
                return Err(anyhow::anyhow!("Request failed with status: {}", response.status()));
            }

            let html_content = response.text()
                .map_err(|e| anyhow::anyhow!("Failed to read response text: {}", e))?;

            let document = Html::parse_document(&html_content);

            // Remove unwanted elements
            let unwanted_tags = ["script", "style", "noscript", "header", "footer", "nav", "svg", "iframe"];

            // We can't easily remove elements from the DOM in scraper, so we'll just traverse and ignore them
            // Or we can select the body and traverse.

            let body_selector = Selector::parse("body").unwrap();
            let body = document.select(&body_selector).next();

            if let Some(body_node) = body {
                let mut text_content = String::new();

                for node in body_node.text() {
                    let text = node.trim();
                    if !text.is_empty() {
                        text_content.push_str(text);
                        text_content.push(' ');
                    }
                }

                // Simple cleanup: collapse multiple spaces
                let cleaned_text = text_content.split_whitespace().collect::<Vec<_>>().join(" ");

                // Limit length to avoid token limits (e.g., 10000 chars)
                if cleaned_text.len() > 10000 {
                    Ok(format!("{}... (truncated)", &cleaned_text[..10000]))
                } else {
                    Ok(cleaned_text)
                }
            } else {
                Ok("Could not find body content.".to_string())
            }
        })
        .await
        .map_err(|e| ToolError(e.to_string()))??;

        Ok(result)
    }
}

impl FetchUrl {
    pub fn new() -> Self {
        Self
    }
}
