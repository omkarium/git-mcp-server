//! GitHub provider: API calls for list_repos, read_file, read_files, list_files.
//! Uses GitHub REST API (Contents API, Git Trees API, Orgs/User Repos API).
//!
//! Author: Venkatesh Omkaram

use async_trait::async_trait;
use base64::{engine::general_purpose::STANDARD, Engine as _};
use reqwest::Client;

use crate::provider::GitProvider;

const DEFAULT_GITHUB_API_URL: &str = "https://api.github.com";
const ENV_GITHUB_API_URL: &str = "GITHUB_API_URL";
const GITHUB_ACCEPT_RAW: &str = "application/vnd.github.raw+json";
const GITHUB_ACCEPT_JSON: &str = "application/vnd.github+json";
const GITHUB_API_VERSION: &str = "2022-11-28";
/// GitHub requires a User-Agent header for all API requests.
const USER_AGENT: &str = "git-mcp-server/0.1.0";

/// When true, list_repos uses GET /user/repos (authenticated user's repos).
const ENV_USE_USER_REPOS: &str = "GITHUB_USE_USER_REPOS";

/// When "user", list_repos uses GET /users/{owner}/repos (for usernames).
/// When "org" (default), uses GET /orgs/{owner}/repos (for organizations).
const ENV_OWNER_TYPE: &str = "GITHUB_OWNER_TYPE";

pub struct GitHubProvider {
    pub owner: String,
    pub token: String,
    pub api_base_url: String,
    pub use_user_repos: bool,
    pub owner_is_user: bool,
    http: Client,
}

impl GitHubProvider {
    pub fn new(owner: String, token: String) -> Self {
        let use_user_repos = std::env::var(ENV_USE_USER_REPOS)
            .map(|v| v == "1" || v.eq_ignore_ascii_case("true") || v.eq_ignore_ascii_case("yes"))
            .unwrap_or(false);
        let owner_is_user = std::env::var(ENV_OWNER_TYPE)
            .map(|v| v.eq_ignore_ascii_case("user"))
            .unwrap_or(false);
        let api_base_url = std::env::var(ENV_GITHUB_API_URL)
            .unwrap_or_else(|_| DEFAULT_GITHUB_API_URL.to_string())
            .trim_end_matches('/')
            .to_string();
        let http = Client::builder()
            .user_agent(USER_AGENT)
            .build()
            .unwrap_or_else(|_| Client::new());
        Self { owner, token, api_base_url, use_user_repos, owner_is_user, http }
    }

    fn auth_header(&self) -> String {
        format!("Bearer {}", self.token)
    }

    fn trees_url(&self, repo: &str, tree_sha: &str) -> String {
        format!(
            "{}/repos/{}/{}/git/trees/{}",
            self.api_base_url, self.owner, repo, tree_sha
        )
    }
}

#[async_trait]
impl GitProvider for GitHubProvider {
    fn base_description(&self) -> String {
        if self.use_user_repos {
            "user (authenticated)".to_string()
        } else {
            self.owner.clone()
        }
    }

    async fn list_repos(&self) -> std::result::Result<String, String> {
        let url = if self.use_user_repos {
            format!("{}/user/repos", self.api_base_url)
        } else if self.owner_is_user {
            format!("{}/users/{}/repos", self.api_base_url, self.owner)
        } else {
            format!("{}/orgs/{}/repos", self.api_base_url, self.owner)
        };

        let mut lines: Vec<String> = Vec::new();
        let mut page = 1u32;

        loop {
            let resp = self
                .http
                .get(&url)
                .header("Authorization", self.auth_header())
                .header("Accept", GITHUB_ACCEPT_JSON)
                .header("X-GitHub-Api-Version", GITHUB_API_VERSION)
                .query(&[("per_page", "100"), ("page", page.to_string().as_str())])
                .send()
                .await
                .map_err(|e| e.to_string())?;

            if resp.status() == 401 {
                return Err("401 Unauthorized — check your GITHUB_TOKEN".to_string());
            }
            if !resp.status().is_success() {
                return Err(format!(
                    "HTTP {}: {}",
                    resp.status(),
                    resp.text().await.unwrap_or_default()
                ));
            }

            let json: Vec<serde_json::Value> = resp.json().await.map_err(|e| e.to_string())?;
            if json.is_empty() {
                break;
            }

            for r in &json {
                if let Some(name) = r["name"].as_str() {
                    lines.push(name.to_string());
                }
            }
            page += 1;
            if json.len() < 100 {
                break;
            }
        }

        lines.sort();
        Ok(lines.join("\n"))
    }

    async fn read_file(
        &self,
        repo: &str,
        file_path: &str,
        branch: Option<&str>,
    ) -> std::result::Result<String, String> {
        let path = file_path.trim_matches('/');
        if path.is_empty() {
            return Err("file_path cannot be empty".to_string());
        }
        let url = format!("{}/repos/{}/{}/contents/{}", self.api_base_url, self.owner, repo, path);

        let mut req = self
            .http
            .get(&url)
            .header("Authorization", self.auth_header())
            .header("Accept", GITHUB_ACCEPT_RAW)
            .header("X-GitHub-Api-Version", GITHUB_API_VERSION);

        if let Some(b) = branch {
            req = req.query(&[("ref", b)]);
        }

        let resp = req.send().await.map_err(|e| e.to_string())?;
        let status = resp.status();
        if status == 401 {
            return Err("401 Unauthorized — check your GITHUB_TOKEN".to_string());
        }
        if status == 404 {
            return Err(format!("404 Not Found — '{}' does not exist", file_path));
        }
        if !status.is_success() {
            return Err(format!("HTTP {}: {}", status, resp.text().await.unwrap_or_default()));
        }

        let body = resp.text().await.map_err(|e| e.to_string())?;

        // With Accept: application/vnd.github.raw+json, GitHub returns raw content for files.
        // For directories it returns JSON — we'd get 404 from our flow since we request a file.
        // If we somehow get JSON (e.g. directory), check for "content" field and decode base64.
        if body.trim_start().starts_with('{') {
            if let Ok(json) = serde_json::from_str::<serde_json::Value>(&body) {
                if let Some(content_b64) = json["content"].as_str() {
                    let decoded = STANDARD
                        .decode(content_b64.replace('\n', ""))
                        .map_err(|e| format!("Base64 decode error: {}", e))?;
                    return String::from_utf8(decoded).map_err(|e| format!("UTF-8 error: {}", e));
                }
            }
        }

        Ok(body)
    }

    async fn read_files(
        &self,
        repo: &str,
        file_paths: &[String],
        branch: Option<&str>,
    ) -> std::result::Result<String, String> {
        if file_paths.is_empty() {
            return Err("file_paths cannot be empty".to_string());
        }

        let mut parts: Vec<String> = Vec::with_capacity(file_paths.len());
        for path in file_paths {
            match self.read_file(repo, path, branch).await {
                Ok(content) => {
                    parts.push(format!("=== {} ===\n{}", path, content));
                }
                Err(e) => {
                    parts.push(format!("=== {} ===\nError: {}", path, e));
                }
            }
        }
        Ok(parts.join("\n\n"))
    }

    async fn list_files(
        &self,
        repo: &str,
        folder_path: &str,
        recursive: bool,
        branch: Option<&str>,
    ) -> std::result::Result<String, String> {
        let path = folder_path.trim_matches('/');
        let ref_str = branch.unwrap_or("HEAD");

        if recursive {
            // Git Trees API: GET /repos/{owner}/{repo}/git/trees/{ref}?recursive=1
            let url = self.trees_url(repo, ref_str);
            let resp = self
                .http
                .get(&url)
                .header("Authorization", self.auth_header())
                .header("Accept", GITHUB_ACCEPT_JSON)
                .header("X-GitHub-Api-Version", GITHUB_API_VERSION)
                .query(&[("recursive", "1")])
                .send()
                .await
                .map_err(|e| e.to_string())?;

            if resp.status() == 401 {
                return Err("401 Unauthorized — check your GITHUB_TOKEN".to_string());
            }
            if resp.status() == 404 {
                return Err(format!("404 Not Found — '{}' or ref '{}' does not exist", folder_path, ref_str));
            }
            if !resp.status().is_success() {
                return Err(format!(
                    "HTTP {}: {}",
                    resp.status(),
                    resp.text().await.unwrap_or_default()
                ));
            }

            let json: serde_json::Value = resp.json().await.map_err(|e| e.to_string())?;
            let entries = json["tree"].as_array().ok_or("Trees response missing tree")?;

            let prefix = if path.is_empty() {
                String::new()
            } else {
                format!("{}/", path)
            };

            let mut lines: Vec<String> = entries
                .iter()
                .filter_map(|entry| {
                    let p = entry["path"].as_str()?;
                    if !prefix.is_empty() && !p.starts_with(&prefix) {
                        return None;
                    }
                    let is_dir = entry["type"].as_str() == Some("tree");
                    let marker = if is_dir { "/" } else { "" };
                    Some(format!("/{}{}", p, marker))
                })
                .collect();
            lines.sort();
            Ok(lines.join("\n"))
        } else {
            // Contents API: GET /repos/{owner}/{repo}/contents/{path}
            let url = if path.is_empty() {
                format!("{}/repos/{}/{}/contents", self.api_base_url, self.owner, repo)
            } else {
                format!("{}/repos/{}/{}/contents/{}", self.api_base_url, self.owner, repo, path)
            };

            let mut req = self
                .http
                .get(&url)
                .header("Authorization", self.auth_header())
                .header("Accept", GITHUB_ACCEPT_JSON)
                .header("X-GitHub-Api-Version", GITHUB_API_VERSION);

            if let Some(b) = branch {
                req = req.query(&[("ref", b)]);
            }

            let resp = req.send().await.map_err(|e| e.to_string())?;
            if resp.status() == 401 {
                return Err("401 Unauthorized — check your GITHUB_TOKEN".to_string());
            }
            if resp.status() == 404 {
                return Err(format!("404 Not Found — '{}' does not exist", folder_path));
            }
            if !resp.status().is_success() {
                return Err(format!(
                    "HTTP {}: {}",
                    resp.status(),
                    resp.text().await.unwrap_or_default()
                ));
            }

            let json: serde_json::Value = resp.json().await.map_err(|e| e.to_string())?;
            let entries = json.as_array().ok_or("Contents response for directory should be array")?;

            let base = if path.is_empty() {
                "/".to_string()
            } else {
                format!("/{}/", path)
            };

            let mut lines: Vec<String> = entries
                .iter()
                .filter_map(|entry| {
                    let name = entry["name"].as_str()?;
                    let is_dir = entry["type"].as_str() == Some("dir");
                    let marker = if is_dir { "/" } else { "" };
                    Some(format!("{}{}{}", base, name, marker))
                })
                .collect();
            lines.sort();
            Ok(lines.join("\n"))
        }
    }

    async fn stargazers(&self, repo: &str) -> std::result::Result<String, String> {
        let url = format!("{}/repos/{}/{}", self.api_base_url, self.owner, repo);
        let resp = self
            .http
            .get(&url)
            .header("Authorization", self.auth_header())
            .header("Accept", GITHUB_ACCEPT_JSON)
            .header("X-GitHub-Api-Version", GITHUB_API_VERSION)
            .send()
            .await
            .map_err(|e| e.to_string())?;

        if resp.status() == 401 {
            return Err("401 Unauthorized — check your GITHUB_TOKEN".to_string());
        }
        if resp.status() == 404 {
            return Err(format!("404 Not Found — repo '{}' does not exist", repo));
        }
        if !resp.status().is_success() {
            return Err(format!(
                "HTTP {}: {}",
                resp.status(),
                resp.text().await.unwrap_or_default()
            ));
        }

        let json: serde_json::Value = resp.json().await.map_err(|e| e.to_string())?;
        let count = json["stargazers_count"]
            .as_u64()
            .unwrap_or(0);
        Ok(format!("{}", count))
    }

    async fn total_pull_requests(&self, repo: &str) -> std::result::Result<String, String> {
        let count = self.count_pulls(repo, "all").await?;
        Ok(format!("{}", count))
    }

    async fn list_active_pull_requests(&self, repo: &str) -> std::result::Result<String, String> {
        self.list_pulls(repo, "open").await
    }

    async fn list_closed_pull_requests(&self, repo: &str) -> std::result::Result<String, String> {
        self.list_pulls(repo, "closed").await
    }

    async fn pull_request_details(
        &self,
        repo: &str,
        pr_id: &str,
    ) -> std::result::Result<String, String> {
        self.get_pr_details(repo, pr_id).await
    }
}

impl GitHubProvider {
    async fn count_pulls(&self, repo: &str, state: &str) -> std::result::Result<u64, String> {
        let url = format!("{}/repos/{}/{}/pulls", self.api_base_url, self.owner, repo);
        let mut total: u64 = 0;
        let mut page = 1u32;

        loop {
            let resp = self
                .http
                .get(&url)
                .header("Authorization", self.auth_header())
                .header("Accept", GITHUB_ACCEPT_JSON)
                .header("X-GitHub-Api-Version", GITHUB_API_VERSION)
                .query(&[
                    ("state", state),
                    ("per_page", "100"),
                    ("page", &page.to_string()),
                ])
                .send()
                .await
                .map_err(|e| e.to_string())?;

            if resp.status() == 401 {
                return Err("401 Unauthorized — check your GITHUB_TOKEN".to_string());
            }
            if resp.status() == 404 {
                return Err(format!("404 Not Found — repo '{}' does not exist", repo));
            }
            if !resp.status().is_success() {
                return Err(format!(
                    "HTTP {}: {}",
                    resp.status(),
                    resp.text().await.unwrap_or_default()
                ));
            }

            let json: Vec<serde_json::Value> = resp.json().await.map_err(|e| e.to_string())?;
            let len = json.len() as u64;
            total += len;
            if len < 100 {
                break;
            }
            page += 1;
        }
        Ok(total)
    }

    async fn list_pulls(&self, repo: &str, state: &str) -> std::result::Result<String, String> {
        let url = format!("{}/repos/{}/{}/pulls", self.api_base_url, self.owner, repo);
        let mut all: Vec<serde_json::Value> = Vec::new();
        let mut page = 1u32;

        loop {
            let resp = self
                .http
                .get(&url)
                .header("Authorization", self.auth_header())
                .header("Accept", GITHUB_ACCEPT_JSON)
                .header("X-GitHub-Api-Version", GITHUB_API_VERSION)
                .query(&[
                    ("state", state),
                    ("per_page", "100"),
                    ("page", &page.to_string()),
                ])
                .send()
                .await
                .map_err(|e| e.to_string())?;

            if resp.status() == 401 {
                return Err("401 Unauthorized — check your GITHUB_TOKEN".to_string());
            }
            if resp.status() == 404 {
                return Err(format!("404 Not Found — repo '{}' does not exist", repo));
            }
            if !resp.status().is_success() {
                return Err(format!(
                    "HTTP {}: {}",
                    resp.status(),
                    resp.text().await.unwrap_or_default()
                ));
            }

            let json: Vec<serde_json::Value> = resp.json().await.map_err(|e| e.to_string())?;
            let len = json.len();
            if len == 0 {
                break;
            }
            all.extend(json);
            if len < 100 {
                break;
            }
            page += 1;
        }

        let lines: Vec<String> = all
            .iter()
            .map(|pr| {
                let num = pr["number"].as_u64().unwrap_or(0);
                let title = pr["title"].as_str().unwrap_or("(no title)");
                let state_val = pr["state"].as_str().unwrap_or("?");
                let html_url = pr["html_url"].as_str().unwrap_or("");
                format!("#{} [{}] {} - {}", num, state_val, title, html_url)
            })
            .collect();
        Ok(lines.join("\n"))
    }

    async fn get_pr_details(&self, repo: &str, pr_id: &str) -> std::result::Result<String, String> {
        let pr_num: u64 = pr_id.parse().map_err(|_| format!("Invalid PR number: {}", pr_id))?;
        let base = format!("{}/repos/{}/{}/pulls/{}", self.api_base_url, self.owner, repo, pr_num);

        // 1. PR details (title, body, state)
        let pr_url = format!("{}", base);
        let pr_resp = self
            .http
            .get(&pr_url)
            .header("Authorization", self.auth_header())
            .header("Accept", GITHUB_ACCEPT_JSON)
            .header("X-GitHub-Api-Version", GITHUB_API_VERSION)
            .send()
            .await
            .map_err(|e| e.to_string())?;

        if pr_resp.status() == 404 {
            return Err(format!("PR #{} not found in repo '{}'", pr_id, repo));
        }
        if !pr_resp.status().is_success() {
            return Err(format!(
                "HTTP {}: {}",
                pr_resp.status(),
                pr_resp.text().await.unwrap_or_default()
            ));
        }

        let pr: serde_json::Value = pr_resp.json().await.map_err(|e| e.to_string())?;
        let title = pr["title"].as_str().unwrap_or("(no title)");
        let body = pr["body"].as_str().unwrap_or("(no description)");
        let state = pr["state"].as_str().unwrap_or("?");
        let html_url = pr["html_url"].as_str().unwrap_or("");

        // 2. Commits
        let commits_url = format!("{}/commits", base);
        let commits_resp = self
            .http
            .get(&commits_url)
            .header("Authorization", self.auth_header())
            .header("Accept", GITHUB_ACCEPT_JSON)
            .header("X-GitHub-Api-Version", GITHUB_API_VERSION)
            .query(&[("per_page", "100")])
            .send()
            .await
            .map_err(|e| e.to_string())?;

        let commits: Vec<serde_json::Value> = if commits_resp.status().is_success() {
            commits_resp.json().await.unwrap_or_default()
        } else {
            Vec::new()
        };

        let commit_lines: Vec<String> = commits
            .iter()
            .map(|c| {
                let sha = c["sha"].as_str().unwrap_or("").chars().take(7).collect::<String>();
                let msg = c["commit"]["message"].as_str().unwrap_or("").lines().next().unwrap_or("");
                let author = c["commit"]["author"]["name"].as_str().unwrap_or("?");
                format!("  {} {} ({})", sha, msg, author)
            })
            .collect();

        // 3. Files changed
        let files_url = format!("{}/files", base);
        let files_resp = self
            .http
            .get(&files_url)
            .header("Authorization", self.auth_header())
            .header("Accept", GITHUB_ACCEPT_JSON)
            .header("X-GitHub-Api-Version", GITHUB_API_VERSION)
            .send()
            .await
            .map_err(|e| e.to_string())?;

        let files: Vec<serde_json::Value> = if files_resp.status().is_success() {
            files_resp.json().await.unwrap_or_default()
        } else {
            Vec::new()
        };

        let file_lines: Vec<String> = files
            .iter()
            .map(|f| {
                let path = f["filename"].as_str().unwrap_or("?");
                let status = f["status"].as_str().unwrap_or("?");
                let additions = f["additions"].as_u64().unwrap_or(0);
                let deletions = f["deletions"].as_u64().unwrap_or(0);
                format!("  {} [{}] +{} -{}", path, status, additions, deletions)
            })
            .collect();

        // 4. Issue comments (general PR comments)
        let issue_num = pr["number"].as_u64().unwrap_or(pr_num);
        let comments_url = format!("{}/repos/{}/{}/issues/{}/comments", self.api_base_url, self.owner, repo, issue_num);
        let comments_resp = self
            .http
            .get(&comments_url)
            .header("Authorization", self.auth_header())
            .header("Accept", GITHUB_ACCEPT_JSON)
            .header("X-GitHub-Api-Version", GITHUB_API_VERSION)
            .query(&[("per_page", "100")])
            .send()
            .await
            .map_err(|e| e.to_string())?;

        let comments: Vec<serde_json::Value> = if comments_resp.status().is_success() {
            comments_resp.json().await.unwrap_or_default()
        } else {
            Vec::new()
        };

        // 5. Review comments (inline)
        let review_comments_url = format!("{}/repos/{}/{}/pulls/{}/comments", self.api_base_url, self.owner, repo, pr_num);
        let review_resp = self
            .http
            .get(&review_comments_url)
            .header("Authorization", self.auth_header())
            .header("Accept", GITHUB_ACCEPT_JSON)
            .header("X-GitHub-Api-Version", GITHUB_API_VERSION)
            .query(&[("per_page", "100")])
            .send()
            .await
            .map_err(|e| e.to_string())?;

        let review_comments: Vec<serde_json::Value> = if review_resp.status().is_success() {
            review_resp.json().await.unwrap_or_default()
        } else {
            Vec::new()
        };

        let all_comments: Vec<String> = comments
            .iter()
            .map(|c| {
                let user = c["user"]["login"].as_str().unwrap_or("?");
                let body = c["body"].as_str().unwrap_or("").lines().take(3).collect::<Vec<_>>().join(" ");
                let created = c["created_at"].as_str().unwrap_or("");
                format!("  [{}] {}: {} ({})", "general", user, body, created)
            })
            .chain(review_comments.iter().map(|c| {
                let user = c["user"]["login"].as_str().unwrap_or("?");
                let body = c["body"].as_str().unwrap_or("").lines().take(3).collect::<Vec<_>>().join(" ");
                let path = c["path"].as_str().unwrap_or("?");
                format!("  [review @ {}] {}: {}", path, user, body)
            }))
            .collect();

        // 6. Reviews (approvals)
        let reviews_url = format!("{}/repos/{}/{}/pulls/{}/reviews", self.api_base_url, self.owner, repo, pr_num);
        let reviews_resp = self
            .http
            .get(&reviews_url)
            .header("Authorization", self.auth_header())
            .header("Accept", GITHUB_ACCEPT_JSON)
            .header("X-GitHub-Api-Version", GITHUB_API_VERSION)
            .query(&[("per_page", "100")])
            .send()
            .await
            .map_err(|e| e.to_string())?;

        let reviews: Vec<serde_json::Value> = if reviews_resp.status().is_success() {
            reviews_resp.json().await.unwrap_or_default()
        } else {
            Vec::new()
        };

        // Last review per user counts (APPROVED, CHANGES_REQUESTED, COMMENTED)
        let mut approvers: Vec<String> = Vec::new();
        let mut last_by_user: std::collections::HashMap<String, &str> = std::collections::HashMap::new();
        for r in reviews.iter().rev() {
            let user = r["user"]["login"].as_str().unwrap_or("?");
            if !last_by_user.contains_key(user) {
                let state = r["state"].as_str().unwrap_or("?");
                last_by_user.insert(user.to_string(), state);
            }
        }
        for (user, state) in &last_by_user {
            approvers.push(format!("  {}: {}", user, state));
        }

        let approved = last_by_user.values().any(|s| *s == "APPROVED");

        let mut out = Vec::new();
        out.push(format!("=== PR #{}: {} ===", pr_num, title));
        out.push(format!("State: {} | URL: {}", state, html_url));
        out.push(String::new());
        out.push("--- Description ---".to_string());
        out.push(body.to_string());
        out.push(String::new());
        out.push(format!("--- Commits ({}) ---", commits.len()));
        out.extend(commit_lines);
        out.push(String::new());
        out.push(format!("--- Files Changed ({}) ---", files.len()));
        out.extend(file_lines);
        out.push(String::new());
        out.push(format!("--- Comments ({}) ---", all_comments.len()));
        out.extend(all_comments);
        out.push(String::new());
        out.push("--- Approvers / Reviews ---".to_string());
        out.push(format!("  Approved: {}", if approved { "Yes" } else { "No" }));
        out.extend(approvers);

        Ok(out.join("\n"))
    }
}
