use async_trait::async_trait;

use serde_json::json;

use crate::tool_source::{ToolCallContent, ToolCallContext, ToolSourceError};
use crate::tools::Tool;

/// Tool name for the web fetcher operation.
pub const TOOL_WEB_FETCHER: &str = "web_fetcher";

/// Tool for fetching content from URLs via HTTP GET requests.
///
/// Wraps reqwest::Client and exposes it as a tool for the LLM.
/// Interacts with HTTP servers to retrieve web pages, API responses,
/// or other HTTP-accessible content.
///
/// # Examples
///
/// ```no_run
/// use langgraph::tools::WebFetcherTool;
/// use serde_json::json;
///
/// # #[tokio::main]
/// # async fn main() {
/// let tool = WebFetcherTool::new();
///
/// let args = json!({
///     "url": "https://example.com/api/data"
/// });
/// let result = tool.call(args, None).await.unwrap();
/// assert!(!result.text.is_empty());
/// # }
/// ```
///
/// # Interaction
///
/// - **reqwest::Client**: Performs HTTP GET requests
/// - **ToolRegistry**: Registers this tool by name "web_fetcher"
/// - **AggregateToolSource**: Uses this tool via ToolRegistry
/// - **ToolSourceError**: Maps HTTP errors to tool error types
pub struct WebFetcherTool {
    client: reqwest::Client,
}

impl Default for WebFetcherTool {
    fn default() -> Self {
        Self::new()
    }
}

impl WebFetcherTool {
    /// Creates a new WebFetcherTool with a default HTTP client.
    ///
    /// Uses reqwest::Client::new() to create a client with default settings.
    ///
    /// # Examples
    ///
    /// ```
    /// use langgraph::tools::web::WebFetcherTool;
    ///
    /// let tool = WebFetcherTool::new();
    /// ```
    pub fn new() -> Self {
        Self {
            client: reqwest::Client::new(),
        }
    }

    /// Creates a new WebFetcherTool with a custom HTTP client.
    ///
    /// # Parameters
    ///
    /// - `client`: Custom reqwest::Client for configuring timeouts, proxies, etc.
    ///
    /// # Examples
    ///
    /// ```
    /// use langgraph::tools::web::WebFetcherTool;
    /// use std::time::Duration;
    ///
    /// let client = reqwest::Client::builder()
    ///     .timeout(Duration::from_secs(30))
    ///     .build()
    ///     .unwrap();
    /// let tool = WebFetcherTool::with_client(client);
    /// ```
    pub fn with_client(client: reqwest::Client) -> Self {
        Self { client }
    }
}

#[async_trait]
impl Tool for WebFetcherTool {
    /// Returns the unique name of this tool.
    ///
    /// Returns "web_fetcher" as the tool identifier.
    fn name(&self) -> &str {
        TOOL_WEB_FETCHER
    }

    /// Returns the specification for this tool.
    ///
    /// Includes tool name, description (for the LLM), and JSON schema for arguments.
    /// The spec describes the required "url" parameter.
    ///
    /// # Interaction
    ///
    /// - Called by ToolRegistry::list() to build Vec<ToolSpec>
    /// - Spec fields are aligned with MCP tools/list result
    fn spec(&self) -> crate::tool_source::ToolSpec {
        crate::tool_source::ToolSpec {
            name: TOOL_WEB_FETCHER.to_string(),
            description: Some(
                "Fetch content from a URL. Use this tool to retrieve web pages, API responses, \
                 or other HTTP-accessible content. Supports GET requests and returns the response \
                 body as text.".to_string(),
            ),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "url": {
                        "type": "string",
                        "description": "The URL to fetch content from. Must be a valid HTTP/HTTPS URL."
                    }
                },
                "required": ["url"]
            }),
        }
    }

    /// Executes the tool by fetching content from the specified URL.
    ///
    /// # Parameters
    ///
    /// - `args`: JSON value containing the "url" parameter
    /// - `_ctx`: Optional per-call context (not used by this tool)
    ///
    /// # Returns
    ///
    /// The HTTP response body as text content.
    ///
    /// # Errors
    ///
    /// Returns ToolSourceError for:
    /// - Missing or invalid "url" parameter (InvalidInput)
    /// - HTTP request failures (Transport)
    /// - Non-success HTTP status codes (Transport)
    /// - Response read failures (Transport)
    ///
    /// # Interaction
    ///
    /// - Called by ToolRegistry::call() which validates tool name exists
    /// - Uses reqwest::Client for HTTP GET requests
    async fn call(
        &self,
        args: serde_json::Value,
        _ctx: Option<&ToolCallContext>,
    ) -> Result<ToolCallContent, ToolSourceError> {
        let url = args
            .get("url")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolSourceError::InvalidInput("missing url".to_string()))?;

        let response = self
            .client
            .get(url)
            .send()
            .await
            .map_err(|e| ToolSourceError::Transport(format!("request failed: {}", e)))?;

        if !response.status().is_success() {
            return Err(ToolSourceError::Transport(format!(
                "request failed with status: {}",
                response.status()
            )));
        }

        let content = response
            .text()
            .await
            .map_err(|e| ToolSourceError::Transport(format!("failed to read response: {}", e)))?;

        Ok(ToolCallContent { text: content })
    }
}
