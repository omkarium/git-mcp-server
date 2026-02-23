# git-mcp-server

MCP (Model Context Protocol) server for Git repositories. Supports **Azure DevOps** and **GitHub**. Provides tools to list repos, list files, and read file contents. Client-agnostic — use with any org, project, owner, and repo names.

**Author:** Venkatesh Omkaram

## Prerequisites

- **Option A:** Rust (for building from source)
- **Option B:** Pre-built binary (no Cargo/Rust required)
- Personal Access Token (PAT) for your provider (Azure DevOps or GitHub)
- Node.js (for MCP Inspector)

## Provider Selection

Set `GIT_PROVIDER` to choose the backend:

| Value | Provider |
|-------|----------|
| `azdo`, `azure`, `azuredevops` | Azure DevOps (default) |
| `github` | GitHub |

## Configuration

### Azure DevOps (`GIT_PROVIDER=azdo`)

| Variable      | Required | Description                          | Default              |
|---------------|----------|--------------------------------------|----------------------|
| `AZDO_PAT`    | Yes      | Azure DevOps Personal Access Token   | —                    |
| `AZDO_ORG`    | No       | Organization name                    | `your-org`           |
| `AZDO_PROJECT`| No       | Project name                         | `your-project`       |
| `AZDO_BASE_URL` | No     | Base URL for Azure DevOps (e.g. for Azure DevOps Server) | `https://dev.azure.com` |

### GitHub (`GIT_PROVIDER=github`)

| Variable              | Required | Description                                              | Default      |
|-----------------------|----------|----------------------------------------------------------|--------------|
| `GITHUB_TOKEN`        | Yes      | GitHub Personal Access Token                             | —            |
| `GITHUB_OWNER`        | No       | Organization or username (for `list_repos`)              | `your-owner` |
| `GITHUB_OWNER_TYPE`   | No       | `user` for usernames (e.g. github.com/omkarium); `org` for organizations | `org` |
| `GITHUB_USE_USER_REPOS` | No     | Set to `1` or `true` to list the authenticated token owner's repos (ignores `GITHUB_OWNER`) | —            |
| `GITHUB_API_URL`      | No       | GitHub API base URL (e.g. for GitHub Enterprise)        | `https://api.github.com` |

**GitHub owner modes:** Use `GITHUB_OWNER_TYPE=user` when `GITHUB_OWNER` is a username (e.g. `github.com/omkarium`). Use `GITHUB_OWNER_TYPE=org` (default) for organizations. Use `GITHUB_USE_USER_REPOS=1` to list repos of the account that owns the token.

## Building

```bash
# Debug build
cargo build

# Release build (recommended for running without Cargo)
cargo build --release
```

The binary is produced at `target/release/git-mcp-server`. Copy it anywhere and run directly — no Cargo required.

## Testing

### 1. MCP Inspector (recommended)

```bash
cd git-mcp-server

# Azure DevOps
npx @modelcontextprotocol/inspector -e AZDO_PAT=$AZDO_PAT -e GIT_PROVIDER=azdo cargo run

# GitHub (organization)
npx @modelcontextprotocol/inspector -e GITHUB_TOKEN=$GITHUB_TOKEN -e GITHUB_OWNER=my-org -e GIT_PROVIDER=github cargo run

# GitHub (username, e.g. github.com/omkarium)
npx @modelcontextprotocol/inspector -e GITHUB_TOKEN=$GITHUB_TOKEN -e GITHUB_OWNER=omkarium -e GITHUB_OWNER_TYPE=user -e GIT_PROVIDER=github cargo run
```

The inspector opens at **http://localhost:6274**.

**Without Cargo:** Use the binary path instead of `cargo run`:

```bash
npx @modelcontextprotocol/inspector -e AZDO_PAT=$AZDO_PAT -e GIT_PROVIDER=azdo /path/to/git-mcp-server
```

### 2. Shell script

```bash
cd git-mcp-server

# Azure DevOps (default)
AZDO_PAT="your-token" ./test-mcp.sh

# GitHub (organization)
GIT_PROVIDER=github GITHUB_TOKEN="your-token" GITHUB_OWNER=my-org ./test-mcp.sh

# GitHub (username, e.g. github.com/omkarium)
GIT_PROVIDER=github GITHUB_TOKEN="your-token" GITHUB_OWNER=omkarium GITHUB_OWNER_TYPE=user ./test-mcp.sh

# Without Cargo: use pre-built binary
GIT_MCP_SERVER=./target/release/git-mcp-server AZDO_PAT="your-token" ./test-mcp.sh
```

### 3. Python script

```bash
cd git-mcp-server

# Azure DevOps (default)
AZDO_PAT="your-token" python3 test-mcp.py

# GitHub (organization)
GIT_PROVIDER=github GITHUB_TOKEN="your-token" GITHUB_OWNER=my-org python3 test-mcp.py

# GitHub (username)
GIT_PROVIDER=github GITHUB_TOKEN="your-token" GITHUB_OWNER=omkarium GITHUB_OWNER_TYPE=user python3 test-mcp.py

# Without Cargo: use pre-built binary
GIT_MCP_SERVER=./target/release/git-mcp-server AZDO_PAT="your-token" python3 test-mcp.py
```

## Available Tools

| Tool        | Description                                    |
|-------------|------------------------------------------------|
| `list_repos`| List all Git repositories (project/org/user)    |
| `list_files`| List files/folders in a repo (supports recursive) |
| `read_file` | Read content of a single file                  |
| `read_files`| Read multiple files in one call                |
| `stargazers`| Returns stargazers count for a repo (GitHub only; Azure DevOps returns N/A) |
| `total_pull_requests`| Returns total count of pull requests (open + closed) |
| `list_active_pull_requests`| Lists active (open) pull requests |
| `list_closed_pull_requests`| Lists closed pull requests |
| `pull_request_details`| Returns detailed PR info: description, commits, files changed, comments, approvers, approval status |

## Cursor Integration

Use the **binary path** when Cargo is not installed. Use **cargo run** when building from source.

### Azure DevOps (binary — no Cargo required)

```json
{
  "mcpServers": {
    "git": {
      "command": "/path/to/git-mcp-server",
      "args": [],
      "env": {
        "GIT_PROVIDER": "azdo",
        "AZDO_PAT": "your-token",
        "AZDO_ORG": "your-org",
        "AZDO_PROJECT": "your-project"
      }
    }
  }
}
```

### Azure DevOps (from source with Cargo)

```json
{
  "mcpServers": {
    "git": {
      "command": "cargo",
      "args": ["run", "--manifest-path", "/path/to/git-mcp-server/Cargo.toml"],
      "env": {
        "GIT_PROVIDER": "azdo",
        "AZDO_PAT": "your-token",
        "AZDO_ORG": "your-org",
        "AZDO_PROJECT": "your-project"
      }
    }
  }
}
```

### GitHub (binary — no Cargo required)

```json
{
  "mcpServers": {
    "git": {
      "command": "/path/to/git-mcp-server",
      "args": [],
      "env": {
        "GIT_PROVIDER": "github",
        "GITHUB_TOKEN": "your-token",
        "GITHUB_OWNER": "omkarium",
        "GITHUB_OWNER_TYPE": "user"
      }
    }
  }
}
```

### Both providers (binary)

```json
{
  "mcpServers": {
    "git-azdo": {
      "command": "/path/to/git-mcp-server",
      "args": [],
      "env": {
        "GIT_PROVIDER": "azdo",
        "AZDO_PAT": "your-azdo-token",
        "AZDO_ORG": "your-org",
        "AZDO_PROJECT": "your-project"
      }
    },
    "git-github": {
      "command": "/path/to/git-mcp-server",
      "args": [],
      "env": {
        "GIT_PROVIDER": "github",
        "GITHUB_TOKEN": "your-github-token",
        "GITHUB_OWNER": "omkarium",
        "GITHUB_OWNER_TYPE": "user"
      }
    }
  }
}
```

Replace `/path/to/git-mcp-server` with the actual binary path (e.g. `./target/release/git-mcp-server` or `$HOME/bin/git-mcp-server`). To switch providers, choose `git-azdo` or `git-github` when calling tools.
