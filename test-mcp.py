#!/usr/bin/env python3
"""
Test git-mcp-server via JSON-RPC (stdio).

Azure DevOps: Set AZDO_PAT before running.
  AZDO_PAT="your-token" python3 test-mcp.py

GitHub (e.g. github.com/omkarium): Set GIT_PROVIDER, GITHUB_TOKEN, GITHUB_OWNER.
  GIT_PROVIDER=github GITHUB_TOKEN="your-token" GITHUB_OWNER=omkarium GITHUB_OWNER_TYPE=user python3 test-mcp.py

Override repo/branch/files: TEST_REPO, TEST_BRANCH, TEST_FILES (or AZDO_TEST_*, GITHUB_TEST_*).

Without Cargo: Set GIT_MCP_SERVER to the binary path, or run from a dir with target/release/git-mcp-server.
  GIT_MCP_SERVER=/path/to/git-mcp-server python3 test-mcp.py

Author: Venkatesh Omkaram
"""

import json
import os
import subprocess
import sys
from pathlib import Path
from typing import Dict, List

# ANSI styles
BOLD = "\033[1m"
DIM = "\033[2m"
GREEN = "\033[32m"
BLUE = "\033[34m"
YELLOW = "\033[33m"
RED = "\033[31m"
CYAN = "\033[36m"
RESET = "\033[0m"

# Box drawing
H = "─"
V = "│"
TL = "┌"
TR = "┐"
BL = "└"
BR = "┘"


def header(title: str, step: str) -> str:
    """Render a styled section header."""
    width = 60
    return f"\n{CYAN}{TL}{H * (width - 2)}{TR}{RESET}\n{CYAN}{V}{RESET} {BOLD}{title}{RESET} {DIM}({step}){RESET}\n{CYAN}{V}{H * (width - 2)}{V}{RESET}"


def footer() -> str:
    return f"{CYAN}{BL}{H * 58}{BR}{RESET}\n"


def format_tool_result(response: dict) -> str:
    """Extract and format tool result from JSON-RPC response."""
    if "result" not in response:
        return json.dumps(response, indent=2)

    result = response["result"]
    if "content" in result:
        parts = []
        for item in result.get("content", []):
            if item.get("type") == "text":
                text = item.get("text", "")
                if result.get("isError"):
                    parts.append(f"{RED}{text}{RESET}")
                else:
                    parts.append(text)
        return "\n".join(parts) if parts else json.dumps(result, indent=2)

    return json.dumps(result, indent=2)


def truncate(text: str, max_lines: int = 20) -> str:
    """Truncate long output with a note."""
    lines = text.splitlines()
    if len(lines) <= max_lines:
        return text
    return "\n".join(lines[:max_lines]) + f"\n{DIM}... ({len(lines) - max_lines} more lines){RESET}"


# Configurable via env vars. Defaults depend on GIT_PROVIDER.
_provider = os.environ.get("GIT_PROVIDER", "azdo").lower()
if _provider == "github":
    _default_repo = os.environ.get("GITHUB_TEST_REPO", "rufendec")  # e.g. github.com/omkarium/rufendec
    _default_branch = os.environ.get("GITHUB_TEST_BRANCH", "main")
    _default_files = os.environ.get("GITHUB_TEST_FILES", ".gitignore,README.md,Cargo.toml")
else:
    _default_repo = os.environ.get("AZDO_TEST_REPO", "example-repo")
    _default_branch = os.environ.get("AZDO_TEST_BRANCH", "main")
    _default_files = os.environ.get("AZDO_TEST_FILES", ".gitignore,README.md,Cargo.toml")

TEST_REPO = os.environ.get("TEST_REPO", _default_repo)
TEST_BRANCH = os.environ.get("TEST_BRANCH", _default_branch)
TEST_FILES = os.environ.get("TEST_FILES", _default_files)
TEST_FILE_LIST = [f.strip() for f in TEST_FILES.split(",") if f.strip()] or [".gitignore", "README.md", "Cargo.toml"]
# For pull_request_details: repo and pr_id. AZDO default uses a repo with known PRs.
_test_pr_repo = os.environ.get("TEST_PR_REPO", "mule-s-services-australia-hi" if _provider == "azdo" else _default_repo)
_test_pr_id = os.environ.get("TEST_PR_ID", "33403" if _provider == "azdo" else "3")
TEST_PR_REPO = os.environ.get("TEST_PR_REPO", _test_pr_repo)
TEST_PR_ID = os.environ.get("TEST_PR_ID", _test_pr_id)

# Tool definitions: (id, label, request)
TOOLS = [
    (1, "initialize", {
        "jsonrpc": "2.0", "id": 1, "method": "initialize",
        "params": {"protocolVersion": "2025-11-25", "clientInfo": {"name": "test", "version": "1.0"}, "capabilities": {}}
    }),
    (2, "list_files", {
        "jsonrpc": "2.0", "id": 2, "method": "tools/call",
        "params": {"name": "list_files", "arguments": {"repo": TEST_REPO, "folder_path": "/", "recursive": True, "branch": TEST_BRANCH}}
    }),
    (3, "read_file", {
        "jsonrpc": "2.0", "id": 3, "method": "tools/call",
        "params": {"name": "read_file", "arguments": {"repo": TEST_REPO, "file_path": "/.gitignore", "branch": TEST_BRANCH}}
    }),
    (4, "read_files", {
        "jsonrpc": "2.0", "id": 4, "method": "tools/call",
        "params": {"name": "read_files", "arguments": {"repo": TEST_REPO, "file_paths": TEST_FILE_LIST, "branch": TEST_BRANCH}}
    }),
    (5, "list_repos", {
        "jsonrpc": "2.0", "id": 5, "method": "tools/call",
        "params": {"name": "list_repos", "arguments": {}}
    }),
    (6, "stargazers", {
        "jsonrpc": "2.0", "id": 6, "method": "tools/call",
        "params": {"name": "stargazers", "arguments": {"repo": TEST_REPO}}
    }),
    (7, "total_pull_requests", {
        "jsonrpc": "2.0", "id": 7, "method": "tools/call",
        "params": {"name": "total_pull_requests", "arguments": {"repo": TEST_REPO}}
    }),
    (8, "list_active_pull_requests", {
        "jsonrpc": "2.0", "id": 8, "method": "tools/call",
        "params": {"name": "list_active_pull_requests", "arguments": {"repo": TEST_REPO}}
    }),
    (9, "list_closed_pull_requests", {
        "jsonrpc": "2.0", "id": 9, "method": "tools/call",
        "params": {"name": "list_closed_pull_requests", "arguments": {"repo": TEST_REPO}}
    }),
    (10, "pull_request_details", {
        "jsonrpc": "2.0", "id": 10, "method": "tools/call",
        "params": {"name": "pull_request_details", "arguments": {"repo": TEST_PR_REPO, "pr_id": TEST_PR_ID}}
    }),
]


def _server_cmd(script_dir: Path) -> List[str]:
    """Resolve server command: GIT_MCP_SERVER > target/release/git-mcp-server > cargo run."""
    env_bin = os.environ.get("GIT_MCP_SERVER")
    if env_bin and os.path.isfile(env_bin) and os.access(env_bin, os.X_OK):
        return [env_bin]
    release_bin = script_dir / "target" / "release" / "git-mcp-server"
    if release_bin.exists() and os.access(release_bin, os.X_OK):
        return [str(release_bin)]
    return ["cargo", "run", "--quiet"]


def main():
    script_dir = Path(__file__).resolve().parent
    if not (script_dir / "Cargo.toml").exists():
        print(f"{RED}Error: Cargo.toml not found in {script_dir}{RESET}", file=sys.stderr)
        return 1

    cmd = _server_cmd(script_dir)
    proc = subprocess.Popen(
        cmd,
        cwd=script_dir,
        stdin=subprocess.PIPE,
        stdout=subprocess.PIPE,
        stderr=subprocess.DEVNULL,
        text=True,
    )

    responses_by_id: Dict[int, dict] = {}

    # Send all requests
    for req_id, label, request in TOOLS:
        proc.stdin.write(json.dumps(request) + "\n")
    proc.stdin.flush()
    proc.stdin.close()

    # Read responses (one JSON-RPC message per line)
    for line in proc.stdout:
        line = line.strip()
        if not line:
            continue
        try:
            resp = json.loads(line)
            rid = resp.get("id")
            if rid is not None:
                responses_by_id[rid] = resp
        except json.JSONDecodeError:
            pass

    proc.wait()

    # Print styled output
    provider_label = "github" if _provider == "github" else "azdo"
    print(f"\n{BOLD}{GREEN}git-mcp-server test results ({provider_label}, repo={TEST_REPO}){RESET}\n", flush=True)

    for req_id, label, _ in TOOLS:
        resp = responses_by_id.get(req_id)
        step = f"id={req_id}"
        print(header(label, step), flush=True)

        if resp is None:
            print(f"{RED}No response{RESET}", flush=True)
        else:
            formatted = format_tool_result(resp)
            print(truncate(formatted), flush=True)

        print(footer(), flush=True)

    return 0 if proc.returncode == 0 else 1


if __name__ == "__main__":
    sys.exit(main())
