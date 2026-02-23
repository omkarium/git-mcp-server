#!/bin/bash
# Test git-mcp-server via JSON-RPC (stdio).
#
# Azure DevOps: AZDO_PAT="your-token" ./test-mcp.sh
#
# GitHub (e.g. github.com/omkarium):
#   GIT_PROVIDER=github GITHUB_TOKEN="your-token" GITHUB_OWNER=omkarium GITHUB_OWNER_TYPE=user ./test-mcp.sh
#
# Without Cargo: Set GIT_MCP_SERVER to the binary path, or run from a dir with target/release/git-mcp-server.
#   GIT_MCP_SERVER=/path/to/git-mcp-server ./test-mcp.sh
#
# Override: TEST_REPO, TEST_BRANCH, TEST_FILES (or AZDO_TEST_*, GITHUB_TEST_*)
# Author: Venkatesh Omkaram

cd "$(dirname "$0")"

# Resolve server command: GIT_MCP_SERVER > target/release/git-mcp-server > cargo run
if [ -n "$GIT_MCP_SERVER" ] && [ -x "$GIT_MCP_SERVER" ]; then
  MCP_CMD="$GIT_MCP_SERVER"
elif [ -x "target/release/git-mcp-server" ]; then
  MCP_CMD="./target/release/git-mcp-server"
else
  MCP_CMD="cargo run"
fi

PROVIDER="${GIT_PROVIDER:-azdo}"
if [ "$PROVIDER" = "github" ]; then
  REPO="${TEST_REPO:-${GITHUB_TEST_REPO:-rufendec}}"
  BRANCH="${TEST_BRANCH:-${GITHUB_TEST_BRANCH:-main}}"
  FILES="${TEST_FILES:-${GITHUB_TEST_FILES:-[\".gitignore\",\"README.md\",\"Cargo.toml\"]}}"
else
  REPO="${TEST_REPO:-${AZDO_TEST_REPO:-example-repo}}"
  BRANCH="${TEST_BRANCH:-${AZDO_TEST_BRANCH:-main}}"
  FILES="${TEST_FILES:-${AZDO_TEST_FILES:-[\".gitignore\",\"README.md\",\"Cargo.toml\"]}}"
fi

# Handshake: establish MCP protocol version and client info
INIT='{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-11-25","clientInfo":{"name":"test","version":"1.0"},"capabilities":{}}}'

# List all files recursively from repo root
LIST_FILES="{\"jsonrpc\":\"2.0\",\"id\":2,\"method\":\"tools/call\",\"params\":{\"name\":\"list_files\",\"arguments\":{\"repo\":\"$REPO\",\"folder_path\":\"/\",\"recursive\":true,\"branch\":\"$BRANCH\"}}}"

# Read a single file (.gitignore)
READ_FILE="{\"jsonrpc\":\"2.0\",\"id\":3,\"method\":\"tools/call\",\"params\":{\"name\":\"read_file\",\"arguments\":{\"repo\":\"$REPO\",\"file_path\":\"/.gitignore\",\"branch\":\"$BRANCH\"}}}"

# Read multiple files in one call
READ_FILES="{\"jsonrpc\":\"2.0\",\"id\":4,\"method\":\"tools/call\",\"params\":{\"name\":\"read_files\",\"arguments\":{\"repo\":\"$REPO\",\"file_paths\":$FILES,\"branch\":\"$BRANCH\"}}}"

# List all repos in the project
LIST_REPOS='{"jsonrpc":"2.0","id":5,"method":"tools/call","params":{"name":"list_repos","arguments":{}}}'

# Stargazers count (GitHub: numeric; AZDO: N/A)
STARGAZERS="{\"jsonrpc\":\"2.0\",\"id\":6,\"method\":\"tools/call\",\"params\":{\"name\":\"stargazers\",\"arguments\":{\"repo\":\"$REPO\"}}}"

# Total pull requests count
TOTAL_PRS="{\"jsonrpc\":\"2.0\",\"id\":7,\"method\":\"tools/call\",\"params\":{\"name\":\"total_pull_requests\",\"arguments\":{\"repo\":\"$REPO\"}}}"

# List active (open) pull requests
LIST_ACTIVE_PRS="{\"jsonrpc\":\"2.0\",\"id\":8,\"method\":\"tools/call\",\"params\":{\"name\":\"list_active_pull_requests\",\"arguments\":{\"repo\":\"$REPO\"}}}"

# List closed pull requests
LIST_CLOSED_PRS="{\"jsonrpc\":\"2.0\",\"id\":9,\"method\":\"tools/call\",\"params\":{\"name\":\"list_closed_pull_requests\",\"arguments\":{\"repo\":\"$REPO\"}}}"

# Pull request details (use first PR from list - GitHub: 3, AZDO: 33403 for mule-s-services-australia-hi)
PR_ID="${TEST_PR_ID:-3}"
if [ "$PROVIDER" = "azdo" ]; then
  PR_ID="${TEST_PR_ID:-33403}"
  REPO_PR="${TEST_REPO:-mule-s-services-australia-hi}"
else
  REPO_PR="$REPO"
fi
PR_DETAILS="{\"jsonrpc\":\"2.0\",\"id\":10,\"method\":\"tools/call\",\"params\":{\"name\":\"pull_request_details\",\"arguments\":{\"repo\":\"$REPO_PR\",\"pr_id\":\"$PR_ID\"}}}"

(
  echo "[1/10] initialize ($PROVIDER, repo=$REPO)" >&2
  printf '%s\n' "$INIT"
  echo "[2/10] list_files" >&2
  printf '%s\n' "$LIST_FILES"
  echo "[3/10] read_file" >&2
  printf '%s\n' "$READ_FILE"
  echo "[4/10] read_files" >&2
  printf '%s\n' "$READ_FILES"
  echo "[5/10] list_repos" >&2
  printf '%s\n' "$LIST_REPOS"
  echo "[6/10] stargazers" >&2
  printf '%s\n' "$STARGAZERS"
  echo "[7/10] total_pull_requests" >&2
  printf '%s\n' "$TOTAL_PRS"
  echo "[8/10] list_active_pull_requests" >&2
  printf '%s\n' "$LIST_ACTIVE_PRS"
  echo "[9/10] list_closed_pull_requests" >&2
  printf '%s\n' "$LIST_CLOSED_PRS"
  echo "[10/10] pull_request_details (repo=$REPO_PR pr=$PR_ID)" >&2
  printf '%s\n' "$PR_DETAILS"
) | $MCP_CMD 2>/dev/null
