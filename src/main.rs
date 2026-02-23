//! MCP server entry point. Server setup and configuration.
//! Handler logic lives in handler.rs. Provider logic in azdo.rs and github.rs.
//!
//! Author: Venkatesh Omkaram

mod azdo;
mod github;
mod handler;
mod provider;

use handler::{AzureDevOpsProvider, GitMcpHandler, GitHubProvider};
use rust_mcp_sdk::McpServer;
use rust_mcp_sdk::schema::{
    Implementation, InitializeResult, ProtocolVersion, ServerCapabilities, ServerCapabilitiesTools,
};
use rust_mcp_sdk::{
    error::SdkResult,
    mcp_server::{server_runtime, McpServerOptions, ToMcpServerHandler},
    StdioTransport, TransportOptions,
};
use std::sync::Arc;

const ENV_GIT_PROVIDER: &str = "GIT_PROVIDER";

#[tokio::main]
async fn main() -> SdkResult<()> {
    let provider = std::env::var(ENV_GIT_PROVIDER)
        .unwrap_or_else(|_| "azdo".to_string())
        .to_lowercase();

    let (server_details, handler) = match provider.as_str() {
        "azdo" | "azure" | "azuredevops" => {
            let pat = std::env::var("AZDO_PAT").unwrap_or_else(|_| {
                eprintln!("Warning: AZDO_PAT not set — tool calls will fail with 401");
                String::new()
            });
            let org = std::env::var("AZDO_ORG").unwrap_or_else(|_| "your-org".to_string());
            let project = std::env::var("AZDO_PROJECT").unwrap_or_else(|_| "your-project".to_string());

            let server_details = InitializeResult {
                server_info: Implementation {
                    name: "git-mcp-server".into(),
                    version: "0.1.0".into(),
                    title: Some("Git MCP Server (Azure DevOps)".into()),
                    description: Some(format!("MCP server for Azure DevOps {org}/{project}. Repo name is required per tool call.")),
                    icons: vec![],
                    website_url: None,
                },
                capabilities: ServerCapabilities {
                    tools: Some(ServerCapabilitiesTools { list_changed: None }),
                    ..Default::default()
                },
                protocol_version: ProtocolVersion::V2025_11_25.into(),
                instructions: Some(
                    "Set AZDO_PAT (Personal Access Token) before using tools. \
                     Optionally set AZDO_ORG, AZDO_PROJECT, AZDO_BASE_URL to override defaults. \
                     Use list_repos to list all repos in the project. \
                     Repo name must be provided in each read_file, read_files, and list_files call.".into(),
                ),
                meta: None,
            };

            let provider = Box::new(AzureDevOpsProvider::new(org, project, pat));
            let handler = GitMcpHandler::new(provider).to_mcp_server_handler();
            (server_details, handler)
        }
        "github" => {
            let token = std::env::var("GITHUB_TOKEN").unwrap_or_else(|_| {
                eprintln!("Warning: GITHUB_TOKEN not set — tool calls will fail with 401");
                String::new()
            });
            let owner = std::env::var("GITHUB_OWNER").unwrap_or_else(|_| "your-owner".to_string());

            let server_details = InitializeResult {
                server_info: Implementation {
                    name: "git-mcp-server".into(),
                    version: "0.1.0".into(),
                    title: Some("Git MCP Server (GitHub)".into()),
                    description: Some(format!("MCP server for GitHub ({owner}). Repo name must be provided per tool call.")),
                    icons: vec![],
                    website_url: None,
                },
                capabilities: ServerCapabilities {
                    tools: Some(ServerCapabilitiesTools { list_changed: None }),
                    ..Default::default()
                },
                protocol_version: ProtocolVersion::V2025_11_25.into(),
                instructions: Some(
                    "Set GITHUB_TOKEN before using tools. \
                     Set GITHUB_OWNER (org or username). \
                     Set GITHUB_OWNER_TYPE=user when owner is a username (e.g. github.com/omkarium). \
                     Set GITHUB_USE_USER_REPOS=1 to list authenticated user's repos. \
                     Set GITHUB_API_URL for GitHub Enterprise. \
                     Repo name must be provided in each read_file, read_files, and list_files call.".into(),
                ),
                meta: None,
            };

            let provider = Box::new(GitHubProvider::new(owner, token));
            let handler = GitMcpHandler::new(provider).to_mcp_server_handler();
            (server_details, handler)
        }
        other => {
            eprintln!("Unknown GIT_PROVIDER '{}'. Use 'azdo' or 'github'.", other);
            std::process::exit(1);
        }
    };

    let transport = StdioTransport::new(TransportOptions::default())?;

    let server: Arc<_> = server_runtime::create_server(McpServerOptions {
        server_details,
        transport,
        handler,
        task_store: None,
        client_task_store: None,
    });

    if let Err(e) = server.start().await {
        let fallback = e.to_string();
        let msg = e.rpc_error_message().unwrap_or(&fallback);
        eprintln!("{}", msg);
    }
    Ok(())
}
