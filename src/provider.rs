//! Git provider trait — unified interface for Azure DevOps and GitHub.
//!
//! Author: Venkatesh Omkaram

/// Trait for Git hosting providers (Azure DevOps, GitHub).
/// Each provider implements the same operations with its own API structure.
#[async_trait::async_trait]
pub trait GitProvider: Send + Sync {
    /// Lists all repositories (in project for AZDO, in org/user for GitHub).
    async fn list_repos(&self) -> std::result::Result<String, String>;

    /// Reads the content of a single file.
    async fn read_file(
        &self,
        repo: &str,
        file_path: &str,
        branch: Option<&str>,
    ) -> std::result::Result<String, String>;

    /// Reads multiple files. Returns each file's content with a path header.
    async fn read_files(
        &self,
        repo: &str,
        file_paths: &[String],
        branch: Option<&str>,
    ) -> std::result::Result<String, String>;

    /// Lists files/folders under a directory. Supports recursive listing.
    async fn list_files(
        &self,
        repo: &str,
        folder_path: &str,
        recursive: bool,
        branch: Option<&str>,
    ) -> std::result::Result<String, String>;

    /// Returns stargazers count for a repo. GitHub only; AZDO returns N/A.
    async fn stargazers(&self, repo: &str) -> std::result::Result<String, String>;

    /// Returns total count of pull requests (open + closed).
    async fn total_pull_requests(&self, repo: &str) -> std::result::Result<String, String>;

    /// Returns list of active (open) pull requests.
    async fn list_active_pull_requests(&self, repo: &str) -> std::result::Result<String, String>;

    /// Returns list of closed pull requests.
    async fn list_closed_pull_requests(&self, repo: &str) -> std::result::Result<String, String>;

    /// Returns detailed PR info: description, commits, files changed, comments (active/closed), approvers, approval status.
    async fn pull_request_details(
        &self,
        repo: &str,
        pr_id: &str,
    ) -> std::result::Result<String, String>;

    /// Human-readable description for tool descriptions (e.g. "org/project" or "owner").
    fn base_description(&self) -> String;
}
