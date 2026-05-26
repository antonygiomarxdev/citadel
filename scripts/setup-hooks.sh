#!/bin/bash
# Setup git hooks for local Conventional Commits validation.
# Run this once after cloning the repo.

HOOKS_DIR="$(cd "$(dirname "$0")/.." && pwd)/.githooks"

if [ ! -d "$HOOKS_DIR" ]; then
  echo "ERROR: .githooks/ directory not found"
  exit 1
fi

chmod +x "$HOOKS_DIR/"*
git config core.hooksPath .githooks

echo "✅ Git hooks configured: $(git config core.hooksPath)"
echo ""
echo "commit-msg: validates Conventional Commits format before committing"
