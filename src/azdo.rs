//! Azure DevOps provider: API calls for list_repos, read_file, read_files, list_files.
//! Uses Azure DevOps REST API structure. Logic unchanged from original implementation.
//!
//! Author: Venkatesh Omkaram

use async_trait::async_trait;
use base64::{engine::general_purpose::STANDARD, Engine as _};
use reqwest::Client;

use crate::provider::GitProvider;

const AZDO_API_VERSION: &str = "7.1";
const DEFAULT_AZDO_BASE_URL: &str = "https://dev.azure.com";
const ENV_AZDO_BASE_URL: &str = "AZDO_BASE_URL";

pub struct AzureDevOpsProvider {
    pub org: String,
    pub project: String,
    pub pat: String,
    pub api_base: String,
    http: Client,
}

impl AzureDevOpsProvider {
    pub fn new(org: String, project: String, pat: String) -> Self {
        let api_base = std::env::var(ENV_AZDO_BASE_URL)
            .unwrap_or_else(|_| DEFAULT_AZDO_BASE_URL.to_string())
            .trim_end_matches('/')
            .to_string();
        Self { org, project, pat, api_base, http: Client::new() }
    }

    fn auth_header(&self) -> String {
        let encoded = STANDARD.encode(format!(":{}", self.pat));
        format!("Basic {}", encoded)
    }

    fn base_url(&self, repo: &str) -> String {
        format!(
            "{api_base}/{org}/{project}/_apis/git/repositories/{repo}",
            api_base = self.api_base,
            org = self.org,
            project = self.project,
            repo = repo,
        )
    }

    fn repos_url(&self) -> String {
        format!(
            "{api_base}/{org}/{project}/_apis/git/repositories",
            api_base = self.api_base,
            org = self.org,
            project = self.project,
        )
    }
}

#[async_trait]
impl GitProvider for AzureDevOpsProvider {
    fn base_description(&self) -> String {
        format!("{}/{}", self.org, self.project)
    }

    async fn list_repos(&self) -> std::result::Result<String, String> {
        let resp = self
            .http
            .get(self.repos_url())
            .header("Authorization", self.auth_header())
            .query(&[("api-version", AZDO_API_VERSION)])
            .send()
            .await
            .map_err(|e| e.to_string())?;

        if resp.status() == 401 {
            return Err("401 Unauthorized — check your AZDO_PAT token".to_string());
        }
        if !resp.status().is_success() {
            return Err(format!(
                "HTTP {}: {}",
                resp.status(),
                resp.text().await.unwrap_or_default()
            ));
        }

        let json: serde_json::Value = resp.json().await.map_err(|e| e.to_string())?;
        let repos = json["value"]
            .as_array()
            .ok_or("Unexpected response format")?;

        let mut lines: Vec<String> = repos
            .iter()
            .filter_map(|r| r["name"].as_str().map(String::from))
            .collect();
        lines.sort();
        Ok(lines.join("\n"))
    }

    async fn read_file(
        &self,
        repo: &str,
        file_path: &str,
        branch: Option<&str>,
    ) -> std::result::Result<String, String> {
        let mut req = self
            .http
            .get(format!("{}/items", self.base_url(repo)))
            .header("Authorization", self.auth_header())
            .query(&[
                ("path", file_path),
                ("api-version", AZDO_API_VERSION),
                ("$format", "text"),
            ]);

        if let Some(b) = branch {
            req = req
                .query(&[("versionDescriptor.version", b)])
                .query(&[("versionDescriptor.versionType", "branch")]);
        }

        let resp = req.send().await.map_err(|e| e.to_string())?;
        let status = resp.status();
        if status == 401 {
            return Err("401 Unauthorized — check your AZDO_PAT token".to_string());
        }
        if status == 404 {
            return Err(format!("404 Not Found — '{}' does not exist", file_path));
        }
        if !status.is_success() {
            return Err(format!("HTTP {}: {}", status, resp.text().await.unwrap_or_default()));
        }
        resp.text().await.map_err(|e| e.to_string())
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
        let path_param = if path.is_empty() { "/".to_string() } else { format!("/{}", path) };

        let mut items_req = self
            .http
            .get(format!("{}/items", self.base_url(repo)))
            .header("Authorization", self.auth_header())
            .query(&[("path", path_param.as_str()), ("api-version", AZDO_API_VERSION)]);

        if let Some(b) = branch {
            items_req = items_req
                .query(&[("versionDescriptor.version", b)])
                .query(&[("versionDescriptor.versionType", "branch")]);
        }

        let items_resp = items_req.send().await.map_err(|e| e.to_string())?;
        let status = items_resp.status();
        if status == 401 {
            return Err("401 Unauthorized — check your AZDO_PAT token".to_string());
        }
        if status == 404 {
            return Err(format!("404 Not Found — '{}' does not exist", folder_path));
        }
        if !status.is_success() {
            return Err(format!(
                "HTTP {}: {}",
                status,
                items_resp.text().await.unwrap_or_default()
            ));
        }

        let items_json: serde_json::Value = items_resp.json().await.map_err(|e| e.to_string())?;
        let object_id = items_json["objectId"]
            .as_str()
            .ok_or("Items response missing objectId")?;

        let tree_url = format!("{}/trees/{}", self.base_url(repo), object_id);
        let tree_req = self
            .http
            .get(&tree_url)
            .header("Authorization", self.auth_header())
            .query(&[
                ("recursive", if recursive { "true" } else { "false" }),
                ("api-version", AZDO_API_VERSION),
            ]);

        let tree_resp = tree_req.send().await.map_err(|e| e.to_string())?;
        if !tree_resp.status().is_success() {
            return Err(format!(
                "Trees API error {}: {}",
                tree_resp.status(),
                tree_resp.text().await.unwrap_or_default()
            ));
        }

        let tree_json: serde_json::Value = tree_resp.json().await.map_err(|e| e.to_string())?;
        let entries = tree_json["treeEntries"]
            .as_array()
            .ok_or("Trees response missing treeEntries")?;

        let base = if path.is_empty() {
            "/".to_string()
        } else {
            format!("/{}/", path)
        };

        let mut lines: Vec<String> = entries
            .iter()
            .filter_map(|entry| {
                let rel = entry["relativePath"].as_str()?;
                let is_folder = entry["gitObjectType"].as_str() == Some("tree");
                let full = format!("{}{}", base, rel);
                let marker = if is_folder { "/" } else { "" };
                Some(format!("{}{}", full, marker))
            })
            .collect();
        lines.sort();
        Ok(lines.join("\n"))
    }

    async fn stargazers(&self, _repo: &str) -> std::result::Result<String, String> {
        Ok("N/A - Azure DevOps does not have stargazers".to_string())
    }

    async fn total_pull_requests(&self, repo: &str) -> std::result::Result<String, String> {
        let repo_id = self.repo_name_to_id(repo).await?;
        let count = self.count_pull_requests(&repo_id, "all").await?;
        Ok(format!("{}", count))
    }

    async fn list_active_pull_requests(&self, repo: &str) -> std::result::Result<String, String> {
        let repo_id = self.repo_name_to_id(repo).await?;
        self.list_pull_requests(&repo_id, "active").await
    }

    async fn list_closed_pull_requests(&self, repo: &str) -> std::result::Result<String, String> {
        let repo_id = self.repo_name_to_id(repo).await?;
        self.list_pull_requests(&repo_id, "completed").await
    }

    async fn pull_request_details(
        &self,
        repo: &str,
        pr_id: &str,
    ) -> std::result::Result<String, String> {
        self.get_pr_details(repo, pr_id).await
    }
}

impl AzureDevOpsProvider {
    /// Resolve repository name to GUID by fetching repos list.
    async fn repo_name_to_id(&self, repo: &str) -> std::result::Result<String, String> {
        let resp = self
            .http
            .get(&self.repos_url())
            .header("Authorization", self.auth_header())
            .query(&[("api-version", AZDO_API_VERSION)])
            .send()
            .await
            .map_err(|e| e.to_string())?;

        if resp.status() == 401 {
            return Err("401 Unauthorized — check your AZDO_PAT token".to_string());
        }
        if !resp.status().is_success() {
            return Err(format!(
                "HTTP {}: {}",
                resp.status(),
                resp.text().await.unwrap_or_default()
            ));
        }

        let json: serde_json::Value = resp.json().await.map_err(|e| e.to_string())?;
        let repos = json["value"].as_array().ok_or("Unexpected response format")?;

        for r in repos {
            let name = r["name"].as_str().unwrap_or("");
            if name.eq_ignore_ascii_case(repo) {
                if let Some(id) = r["id"].as_str() {
                    return Ok(id.to_string());
                }
            }
        }
        Err(format!("Repository '{}' not found", repo))
    }

    async fn count_pull_requests(
        &self,
        repo_id: &str,
        status_filter: &str,
    ) -> std::result::Result<u64, String> {
        let url = format!(
            "{}/{}/{}/_apis/git/repositories/{}/pullrequests",
            self.api_base, self.org, self.project, repo_id
        );

        let mut total: u64 = 0;
        let mut skip = 0i32;

        loop {
            let req = self
                .http
                .get(&url)
                .header("Authorization", self.auth_header())
                .query(&[
                    ("api-version", AZDO_API_VERSION),
                    ("searchCriteria.status", status_filter),
                    ("$top", "100"),
                    ("$skip", &skip.to_string()),
                ]);

            let resp = req.send().await.map_err(|e| e.to_string())?;

            if resp.status() == 401 {
                return Err("401 Unauthorized — check your AZDO_PAT token".to_string());
            }
            if !resp.status().is_success() {
                return Err(format!(
                    "HTTP {}: {}",
                    resp.status(),
                    resp.text().await.unwrap_or_default()
                ));
            }

            let json: serde_json::Value = resp.json().await.map_err(|e| e.to_string())?;
            let prs = json["value"].as_array().ok_or("Unexpected response format")?;
            let len = prs.len() as u64;
            total += len;

            if len < 100 {
                break;
            }
            skip += 100;
        }
        Ok(total)
    }

    async fn list_pull_requests(
        &self,
        repo_id: &str,
        status_filter: &str,
    ) -> std::result::Result<String, String> {
        let url = format!(
            "{}/{}/{}/_apis/git/repositories/{}/pullrequests",
            self.api_base, self.org, self.project, repo_id
        );

        let mut all: Vec<serde_json::Value> = Vec::new();
        let mut skip = 0i32;

        loop {
            let resp = self
                .http
                .get(&url)
                .header("Authorization", self.auth_header())
                .query(&[
                    ("api-version", AZDO_API_VERSION),
                    ("searchCriteria.status", status_filter),
                    ("$top", "100"),
                    ("$skip", &skip.to_string()),
                ])
                .send()
                .await
                .map_err(|e| e.to_string())?;

            if resp.status() == 401 {
                return Err("401 Unauthorized — check your AZDO_PAT token".to_string());
            }
            if !resp.status().is_success() {
                return Err(format!(
                    "HTTP {}: {}",
                    resp.status(),
                    resp.text().await.unwrap_or_default()
                ));
            }

            let json: serde_json::Value = resp.json().await.map_err(|e| e.to_string())?;
            let prs = json["value"].as_array().ok_or("Unexpected response format")?;
            if prs.is_empty() {
                break;
            }
            all.extend(prs.clone());
            if prs.len() < 100 {
                break;
            }
            skip += 100;
        }

        let lines: Vec<String> = all
            .iter()
            .map(|pr| {
                let id = pr["pullRequestId"].as_u64().unwrap_or(0);
                let title = pr["title"].as_str().unwrap_or("(no title)");
                let status = pr["status"].as_str().unwrap_or("?");
                let url_val = pr["url"].as_str().unwrap_or("");
                format!("#{} [{}] {} - {}", id, status, title, url_val)
            })
            .collect();
        Ok(lines.join("\n"))
    }

    async fn get_pr_details(&self, repo: &str, pr_id: &str) -> std::result::Result<String, String> {
        let repo_id = self.repo_name_to_id(repo).await?;
        let pr_num: u64 = pr_id.parse().map_err(|_| format!("Invalid PR number: {}", pr_id))?;

        let base = format!(
            "{}/{}/{}/_apis/git/repositories/{}/pullrequests/{}",
            self.api_base, self.org, self.project, repo_id, pr_num
        );

        // 1. PR details (title, description, status, reviewers)
        let pr_resp = self
            .http
            .get(&base)
            .header("Authorization", self.auth_header())
            .query(&[("api-version", AZDO_API_VERSION)])
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
        let description = pr["description"].as_str().unwrap_or("(no description)");
        let status = pr["status"].as_str().unwrap_or("?");
        let web_url = pr["url"].as_str().unwrap_or("");

        // 2. Commits
        let commits_url = format!("{}/commits", base);
        let commits_resp = self
            .http
            .get(&commits_url)
            .header("Authorization", self.auth_header())
            .query(&[("api-version", AZDO_API_VERSION)])
            .send()
            .await
            .map_err(|e| e.to_string())?;

        let commits: Vec<serde_json::Value> = if commits_resp.status().is_success() {
            let json: serde_json::Value = commits_resp.json().await.map_err(|e| e.to_string())?;
            json["value"].as_array().cloned().unwrap_or_default()
        } else {
            Vec::new()
        };

        let commit_lines: Vec<String> = commits
            .iter()
            .map(|c| {
                let sha = c["commitId"].as_str().unwrap_or("").chars().take(7).collect::<String>();
                let msg = c["comment"].as_str().unwrap_or("").lines().next().unwrap_or("");
                let author = c["author"]["name"].as_str().unwrap_or("?");
                format!("  {} {} ({})", sha, msg, author)
            })
            .collect();

        // 3. Iterations -> get latest iteration changes (files)
        let iter_url = format!("{}/iterations", base);
        let iter_resp = self
            .http
            .get(&iter_url)
            .header("Authorization", self.auth_header())
            .query(&[("api-version", AZDO_API_VERSION)])
            .send()
            .await
            .map_err(|e| e.to_string())?;

        let mut file_lines: Vec<String> = Vec::new();
        if iter_resp.status().is_success() {
            let iter_json: serde_json::Value = iter_resp.json().await.map_err(|e| e.to_string())?;
            let empty: Vec<serde_json::Value> = Vec::new();
            let iters = iter_json["value"].as_array().unwrap_or(&empty);
            if let Some(last) = iters.last() {
                let iter_id = last["id"].as_u64().unwrap_or(0);
                let changes_url = format!("{}/iterations/{}/changes", base, iter_id);
                let changes_resp = self
                    .http
                    .get(&changes_url)
                    .header("Authorization", self.auth_header())
                    .query(&[("api-version", AZDO_API_VERSION)])
                    .send()
                    .await
                    .map_err(|e| e.to_string())?;

                if changes_resp.status().is_success() {
                    let ch_json: serde_json::Value = changes_resp.json().await.map_err(|e| e.to_string())?;
                    let empty_ch: Vec<serde_json::Value> = Vec::new();
                    let changes = ch_json["changeEntries"].as_array().unwrap_or(&empty_ch);
                    for ch in changes {
                        let item = &ch["item"];
                        let path = item["path"].as_str().unwrap_or("?");
                        let change_type = ch["changeType"].as_str().unwrap_or("?");
                        file_lines.push(format!("  {} [{}]", path, change_type));
                    }
                }
            }
        }

        // 4. Threads (comments)
        let threads_url = format!("{}/threads", base);
        let threads_resp = self
            .http
            .get(&threads_url)
            .header("Authorization", self.auth_header())
            .query(&[("api-version", AZDO_API_VERSION)])
            .send()
            .await
            .map_err(|e| e.to_string())?;

        let mut all_comments: Vec<String> = Vec::new();
        if threads_resp.status().is_success() {
            let threads_json: serde_json::Value = threads_resp.json().await.map_err(|e| e.to_string())?;
            let empty_arr: Vec<serde_json::Value> = Vec::new();
            let threads = threads_json["value"].as_array().unwrap_or(&empty_arr);
            for t in threads {
                let status = t["status"].as_str().unwrap_or("active");
                let comments = t["comments"].as_array().unwrap_or(&empty_arr);
                for c in comments {
                    let author = c["author"]["displayName"].as_str().unwrap_or("?");
                    let content = c["content"].as_str().unwrap_or("").lines().take(3).collect::<Vec<_>>().join(" ");
                    let created = c["publishedDate"].as_str().unwrap_or("");
                    all_comments.push(format!("  [{}] {}: {} ({})", status, author, content, created));
                }
            }
        }

        // 5. Reviewers (approvers)
        let empty_rev: Vec<serde_json::Value> = Vec::new();
        let reviewers = pr["reviewers"].as_array().unwrap_or(&empty_rev);
        let mut approver_lines: Vec<String> = Vec::new();
        let mut approved = false;
        for r in reviewers {
            let name = r["displayName"].as_str().unwrap_or(r["uniqueName"].as_str().unwrap_or("?"));
            let vote = r["vote"].as_i64().unwrap_or(0);
            let vote_str: String = match vote {
                10 => {
                    approved = true;
                    "Approved".to_string()
                }
                -10 => "Rejected".to_string(),
                5 => "Approved with suggestions".to_string(),
                0 => "No vote".to_string(),
                _ => format!("Vote {}", vote),
            };
            approver_lines.push(format!("  {}: {}", name, vote_str));
        }

        let mut out = Vec::new();
        out.push(format!("=== PR #{}: {} ===", pr_num, title));
        out.push(format!("Status: {} | URL: {}", status, web_url));
        out.push(String::new());
        out.push("--- Description ---".to_string());
        out.push(description.to_string());
        out.push(String::new());
        out.push(format!("--- Commits ({}) ---", commits.len()));
        out.extend(commit_lines);
        out.push(String::new());
        out.push(format!("--- Files Changed ({}) ---", file_lines.len()));
        out.extend(file_lines);
        out.push(String::new());
        out.push(format!("--- Comments ({}) ---", all_comments.len()));
        out.extend(all_comments);
        out.push(String::new());
        out.push("--- Approvers / Reviews ---".to_string());
        out.push(format!("  Approved: {}", if approved { "Yes" } else { "No" }));
        out.extend(approver_lines);

        Ok(out.join("\n"))
    }
}
