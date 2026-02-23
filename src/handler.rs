//! MCP handler: tool definitions and request dispatch.
//! Delegates to the selected Git provider (Azure DevOps or GitHub).
//!
//! Author: Venkatesh Omkaram

use async_trait::async_trait;
use crate::provider::GitProvider;
use rust_mcp_sdk::schema::{
    schema_utils::CallToolError,
    CallToolRequestParams, CallToolResult, ListToolsResult, PaginatedRequestParams, RpcError,
    Tool, ToolInputSchema,
};
use rust_mcp_sdk::mcp_server::ServerHandler;
use rust_mcp_sdk::McpServer;
use std::collections::HashMap;
use std::sync::Arc;

pub use crate::azdo::AzureDevOpsProvider;
pub use crate::github::GitHubProvider;

pub struct GitMcpHandler {
    provider: Box<dyn GitProvider>,
}

impl GitMcpHandler {
    pub fn new(provider: Box<dyn GitProvider>) -> Self {
        Self { provider }
    }
}

fn make_tool(name: &str, description: &str, required: Vec<&str>, props: HashMap<&str, serde_json::Value>) -> Tool {
    let schema_props: HashMap<String, serde_json::Map<String, serde_json::Value>> = props
        .into_iter()
        .map(|(k, v)| {
            let obj = v.as_object().cloned().unwrap_or_default();
            (k.to_string(), obj)
        })
        .collect();

    Tool {
        name: name.to_string(),
        description: Some(description.to_string()),
        input_schema: ToolInputSchema::new(
            required.into_iter().map(|s| s.to_string()).collect(),
            Some(schema_props),
            None,
        ),
        annotations: None,
        execution: None,
        icons: vec![],
        meta: None,
        output_schema: None,
        title: None,
    }
}

#[async_trait]
impl ServerHandler for GitMcpHandler {
    async fn handle_list_tools_request(
        &self,
        _params: Option<PaginatedRequestParams>,
        _runtime: Arc<dyn McpServer>,
    ) -> std::result::Result<ListToolsResult, RpcError> {
        let base_desc = self.provider.base_description();

        let read_file_tool = make_tool(
            "read_file",
            &format!("Reads the content of a file from a Git repo in {}. Repo name must be provided.", base_desc),
            vec!["repo", "file_path"],
            HashMap::from([
                ("repo",      serde_json::json!({"type": "string", "description": "Repository name (e.g. \"example-repo\")"})),
                ("file_path", serde_json::json!({"type": "string", "description": "Full path to the file (e.g. \"/README.md\", \"/src/main.rs\")"})),
                ("branch",    serde_json::json!({"type": "string", "description": "Branch name (optional, defaults to the repo's default branch)"})),
            ]),
        );

        let list_files_tool = make_tool(
            "list_files",
            &format!("Lists files and folders in a directory of a Git repo in {}. Repo name must be provided.", base_desc),
            vec!["repo", "folder_path"],
            HashMap::from([
                ("repo",        serde_json::json!({"type": "string", "description": "Repository name (e.g. \"example-repo\")"})),
                ("folder_path", serde_json::json!({"type": "string", "description": "Folder path to list (e.g. \"/\" for root, \"/src\")"})),
                ("recursive",   serde_json::json!({"type": "boolean", "description": "If true, lists all files recursively (default: false)"})),
                ("branch",      serde_json::json!({"type": "string", "description": "Branch name (optional)"})),
            ]),
        );

        let read_files_tool = make_tool(
            "read_files",
            &format!("Reads multiple files from a Git repo in {}. Repo name must be provided. Returns each file's content with a path header.", base_desc),
            vec!["repo", "file_paths"],
            HashMap::from([
                ("repo",       serde_json::json!({"type": "string", "description": "Repository name (e.g. \"example-repo\")"})),
                ("file_paths", serde_json::json!({"type": "array", "items": {"type": "string"}, "description": "Array of full paths to files (e.g. [\"/README.md\", \"/Cargo.toml\", \"/.gitignore\"])"})),
                ("branch",     serde_json::json!({"type": "string", "description": "Branch name (optional)"})),
            ]),
        );

        let list_repos_tool = make_tool(
            "list_repos",
            &format!("Lists all Git repositories in {}. Uses provider config for org/project or owner.", base_desc),
            vec![],
            HashMap::new(),
        );

        let stargazers_tool = make_tool(
            "stargazers",
            &format!("Returns stargazers count for a repo in {}. GitHub only; Azure DevOps returns N/A.", base_desc),
            vec!["repo"],
            HashMap::from([
                ("repo", serde_json::json!({"type": "string", "description": "Repository name (e.g. \"example-repo\")"})),
            ]),
        );

        let total_pull_requests_tool = make_tool(
            "total_pull_requests",
            &format!("Returns total count of pull requests (open + closed) for a repo in {}.", base_desc),
            vec!["repo"],
            HashMap::from([
                ("repo", serde_json::json!({"type": "string", "description": "Repository name (e.g. \"example-repo\")"})),
            ]),
        );

        let list_active_pull_requests_tool = make_tool(
            "list_active_pull_requests",
            &format!("Lists active (open) pull requests for a repo in {}.", base_desc),
            vec!["repo"],
            HashMap::from([
                ("repo", serde_json::json!({"type": "string", "description": "Repository name (e.g. \"example-repo\")"})),
            ]),
        );

        let list_closed_pull_requests_tool = make_tool(
            "list_closed_pull_requests",
            &format!("Lists closed pull requests for a repo in {}.", base_desc),
            vec!["repo"],
            HashMap::from([
                ("repo", serde_json::json!({"type": "string", "description": "Repository name (e.g. \"example-repo\")"})),
            ]),
        );

        let pull_request_details_tool = make_tool(
            "pull_request_details",
            "Returns detailed PR info: description, commits, files changed, comments (active/closed), approvers, approval status. Use repo and pr_id (PR number).",
            vec!["repo", "pr_id"],
            HashMap::from([
                ("repo", serde_json::json!({"type": "string", "description": "Repository name (e.g. \"example-repo\")"})),
                ("pr_id", serde_json::json!({"type": "string", "description": "Pull request number/ID (e.g. \"303\" or \"3\")"})),
            ]),
        );

        Ok(ListToolsResult {
            tools: vec![
                read_file_tool,
                list_files_tool,
                read_files_tool,
                list_repos_tool,
                stargazers_tool,
                total_pull_requests_tool,
                list_active_pull_requests_tool,
                list_closed_pull_requests_tool,
                pull_request_details_tool,
            ],
            meta: None,
            next_cursor: None,
        })
    }

    async fn handle_call_tool_request(
        &self,
        params: CallToolRequestParams,
        _runtime: Arc<dyn McpServer>,
    ) -> std::result::Result<CallToolResult, CallToolError> {
        let args = params.arguments.as_ref();

        match params.name.as_str() {
            "read_file" => {
                let repo = args
                    .and_then(|a| a.get("repo")).and_then(|v| v.as_str())
                    .ok_or_else(|| CallToolError::from_message("missing required argument: repo"))?;
                let file_path = args
                    .and_then(|a| a.get("file_path")).and_then(|v| v.as_str())
                    .ok_or_else(|| CallToolError::from_message("missing required argument: file_path"))?;
                let branch = args.and_then(|a| a.get("branch")).and_then(|v| v.as_str());

                let content = self.provider.read_file(repo, file_path, branch).await
                    .map_err(|e| CallToolError::from_message(e))?;
                Ok(CallToolResult::text_content(vec![content.into()]))
            }

            "list_files" => {
                let repo = args
                    .and_then(|a| a.get("repo")).and_then(|v| v.as_str())
                    .ok_or_else(|| CallToolError::from_message("missing required argument: repo"))?;
                let folder_path = args
                    .and_then(|a| a.get("folder_path")).and_then(|v| v.as_str())
                    .ok_or_else(|| CallToolError::from_message("missing required argument: folder_path"))?;
                let recursive = args
                    .and_then(|a| a.get("recursive")).and_then(|v| v.as_bool())
                    .unwrap_or(false);
                let branch = args.and_then(|a| a.get("branch")).and_then(|v| v.as_str());

                let listing = self.provider.list_files(repo, folder_path, recursive, branch).await
                    .map_err(|e| CallToolError::from_message(e))?;
                Ok(CallToolResult::text_content(vec![listing.into()]))
            }

            "read_files" => {
                let repo = args
                    .and_then(|a| a.get("repo")).and_then(|v| v.as_str())
                    .ok_or_else(|| CallToolError::from_message("missing required argument: repo"))?;
                let file_paths = args
                    .and_then(|a| a.get("file_paths"))
                    .and_then(|v| v.as_array())
                    .ok_or_else(|| CallToolError::from_message("missing required argument: file_paths (must be an array)"))?;
                let paths: Vec<String> = file_paths
                    .iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect();
                if paths.is_empty() {
                    return Err(CallToolError::from_message("file_paths must contain at least one path"));
                }
                let branch = args.and_then(|a| a.get("branch")).and_then(|v| v.as_str());

                let content = self.provider.read_files(repo, &paths, branch).await
                    .map_err(|e| CallToolError::from_message(e))?;
                Ok(CallToolResult::text_content(vec![content.into()]))
            }

            "list_repos" => {
                let listing = self.provider.list_repos().await
                    .map_err(|e| CallToolError::from_message(e))?;
                Ok(CallToolResult::text_content(vec![listing.into()]))
            }

            "stargazers" => {
                let repo = args
                    .and_then(|a| a.get("repo")).and_then(|v| v.as_str())
                    .ok_or_else(|| CallToolError::from_message("missing required argument: repo"))?;
                let result = self.provider.stargazers(repo).await
                    .map_err(|e| CallToolError::from_message(e))?;
                Ok(CallToolResult::text_content(vec![result.into()]))
            }

            "total_pull_requests" => {
                let repo = args
                    .and_then(|a| a.get("repo")).and_then(|v| v.as_str())
                    .ok_or_else(|| CallToolError::from_message("missing required argument: repo"))?;
                let result = self.provider.total_pull_requests(repo).await
                    .map_err(|e| CallToolError::from_message(e))?;
                Ok(CallToolResult::text_content(vec![result.into()]))
            }

            "list_active_pull_requests" => {
                let repo = args
                    .and_then(|a| a.get("repo")).and_then(|v| v.as_str())
                    .ok_or_else(|| CallToolError::from_message("missing required argument: repo"))?;
                let result = self.provider.list_active_pull_requests(repo).await
                    .map_err(|e| CallToolError::from_message(e))?;
                Ok(CallToolResult::text_content(vec![result.into()]))
            }

            "list_closed_pull_requests" => {
                let repo = args
                    .and_then(|a| a.get("repo")).and_then(|v| v.as_str())
                    .ok_or_else(|| CallToolError::from_message("missing required argument: repo"))?;
                let result = self.provider.list_closed_pull_requests(repo).await
                    .map_err(|e| CallToolError::from_message(e))?;
                Ok(CallToolResult::text_content(vec![result.into()]))
            }

            "pull_request_details" => {
                let repo = args
                    .and_then(|a| a.get("repo")).and_then(|v| v.as_str())
                    .ok_or_else(|| CallToolError::from_message("missing required argument: repo"))?;
                let pr_id = args
                    .and_then(|a| a.get("pr_id")).and_then(|v| v.as_str())
                    .ok_or_else(|| CallToolError::from_message("missing required argument: pr_id"))?;
                let result = self.provider.pull_request_details(repo, pr_id).await
                    .map_err(|e| CallToolError::from_message(e))?;
                Ok(CallToolResult::text_content(vec![result.into()]))
            }

            other => Err(CallToolError::unknown_tool(other.to_string())),
        }
    }
}
