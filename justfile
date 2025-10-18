# peter-hook - Development Workflow
# Requires: just, peter-hook, versioneer
#

export TOOL_NAME := "peter-hook"

# Default recipe to display available commands
default:
    @just --list

# Setup development environment
setup:
    @just install-hooks
    @echo "✅ Setup complete!"

# Install git hooks using peter-hook
install-hooks:
    @echo "Installing git hooks with peter-hook..."
    @if command -v peter-hook >/dev/null 2>&1; then \
        peter-hook install; \
        echo "✅ Git hooks installed"; \
    else \
        echo "❌ peter-hook not found. Install with: cargo install peter-hook"; \
        exit 1; \
    fi

# Version management
version-show:
    @echo "Current version: $(cat VERSION)"
    @echo "Cargo.toml version: $(grep '^version' Cargo.toml | cut -d'\"' -f2)"

# Bump version (patch|minor|major)
bump-version level:
    @echo "Bumping {{ level }} version..."
    @if command -v versioneer >/dev/null 2>&1; then \
        versioneer {{ level }}; \
        echo "✅ Version bumped to: $(cat VERSION)"; \
    else \
        echo "❌ versioneer not found. Install with: cargo install versioneer"; \
        exit 1; \
    fi

# Release workflow - validates and publishes a git tag
release:
    #!/usr/bin/env bash
    set -euo pipefail

    PROJECT_NAME="$TOOL_NAME"

    echo "🚀 Starting release workflow for $PROJECT_NAME..."
    echo ""

    if [ ! -f VERSION ]; then
        echo "❌ VERSION file not found"
        exit 1
    fi
    CURRENT_VERSION=$(cat VERSION)
    TAG="v$CURRENT_VERSION"

    echo "📋 Release Information:"
    echo "  Project: $PROJECT_NAME"
    echo "  Version: $CURRENT_VERSION"
    echo "  Tag: $TAG"
    echo ""

    echo "Step 1: Checking repository is clean..."
    if ! git diff-index --quiet HEAD --; then
        echo "❌ Working directory not clean"
        git status --short
        exit 1
    fi
    echo "✅ Repository is clean"
    echo ""

    echo "Step 2: Checking local and remote HEAD are in sync..."
    git fetch origin main 2>/dev/null || true
    LOCAL_HEAD=$(git rev-parse HEAD)
    REMOTE_HEAD=$(git rev-parse origin/main)
    if [ "$LOCAL_HEAD" != "$REMOTE_HEAD" ]; then
        echo "❌ Local HEAD and origin/main are not in sync"
        echo "  Local:  $LOCAL_HEAD"
        echo "  Remote: $REMOTE_HEAD"
        echo "Run: git push origin main"
        exit 1
    fi
    echo "✅ Local and remote HEAD in sync: ${LOCAL_HEAD:0:8}"
    echo ""

    echo "Step 3: Checking tag does not exist..."
    git fetch --tags origin 2>/dev/null || true
    if git tag -l "$TAG" | grep -q "^$TAG$"; then
        echo "❌ Tag $TAG already exists locally"
        git show "$TAG" --no-patch
        exit 1
    fi
    if git ls-remote --tags origin | grep -q "refs/tags/$TAG$"; then
        echo "❌ Tag $TAG already exists on remote"
        exit 1
    fi
    echo "✅ Tag $TAG does not exist"
    echo ""

    echo "Step 4: Checking no future version tags exist..."
    FUTURE_TAGS=$(git tag -l 'v*' | sed 's/^v//' | while read -r ver; do
        if [ -z "$ver" ]; then continue; fi
        LATEST=$(printf '%s\n%s' "$CURRENT_VERSION" "$ver" | sort -V | tail -n1)
        if [ "$LATEST" = "$ver" ] && [ "$ver" != "$CURRENT_VERSION" ]; then
            echo "$ver"
        fi
    done)
    if [ -n "$FUTURE_TAGS" ]; then
        echo "❌ Future version tags exist:"
        echo "$FUTURE_TAGS" | sed 's/^/  v/'
        exit 1
    fi
    echo "✅ No future version tags found"
    echo ""

    echo "Step 5: Validating version consistency..."
    CARGO_VERSION=$(grep '^version = ' Cargo.toml | head -1 | sed 's/version = \"\(.*\)\"/\1/')
    echo "  VERSION file: $CURRENT_VERSION"
    echo "  Cargo.toml:   $CARGO_VERSION"
    if [ "$CURRENT_VERSION" != "$CARGO_VERSION" ]; then
        echo "❌ Version mismatch between VERSION and Cargo.toml"
        exit 1
    fi
    echo "✅ Version consistency validated"
    echo ""

    echo "Step 6: Creating tag..."
    git tag -a "$TAG" -m "Release $CURRENT_VERSION"
    echo "✅ Created tag: $TAG"
    echo ""

    echo "Ready to publish release:"
    echo "  Tag: $TAG"
    echo "  Version: $CURRENT_VERSION"
    echo "  Commit: ${LOCAL_HEAD:0:8}"
    echo ""

    if [ -t 0 ]; then
        read -p "Push tag to trigger release? [y/N]: " -n 1 -r
        echo
        if [[ ! $REPLY =~ ^[Yy]$ ]]; then
            echo "Release cancelled"
            echo "To push manually: git push origin $TAG"
            exit 0
        fi
    fi

    echo "Step 7: Pushing tag to remote..."
    git push origin "$TAG" --no-verify
    echo "✅ Tag pushed to remote"
    echo ""
    echo "🎉 Release $TAG published!"
    echo ""
    echo "GitHub Actions will now:"
    echo "  1. Create draft release"
    echo "  2. Build cross-platform binaries"
    echo "  3. Publish release"
    echo ""
    echo "Monitor progress: gh run list --workflow=release.yml"

# Clean build artifacts
clean:
    @echo "Cleaning build artifacts..."
    cargo clean
    @rm -rf target/
    @echo "✅ Clean complete!"

# Build in debug mode
build:
    @echo "Building {{TOOL_NAME}}"
    cargo build
    @echo "✅ Build complete!"

# Build in release mode
build-release:
    @echo "Building {{TOOL_NAME}} in release mode..."
    cargo build --release
    @echo "✅ Release build complete!"

# Generate shell completions for all supported shells
completions:
    @./scripts/generate-completions.sh

manpage:
    @./scripts/generate-man.sh

# Run tests
test:
    @echo "Running tests..."
    cargo test --all --verbose
    @echo "✅ Tests complete!"

# Code quality checks
quality: format-check lint test

# Run pre-commit hooks (format-check + clippy-check)
pre-commit:
    @if command -v peter-hook >/dev/null 2>&1; then \
        peter-hook run pre-commit; \
    else \
        echo "❌ peter-hook not found. Install with: cargo install peter-hook"; \
        exit 1; \
    fi

# Run pre-push hooks (test-all + security-audit + version-sync-check + tag-version-check)
pre-push:
    @if command -v peter-hook >/dev/null 2>&1; then \
        peter-hook run pre-push; \
    else \
        echo "❌ peter-hook not found. Install with: cargo install peter-hook"; \
        exit 1; \
    fi

# Format code (requires nightly rustfmt)
format:
    @echo "Formatting code..."
    @if rustup toolchain list | grep -q nightly; then \
        cargo +nightly fmt; \
        echo "✅ Code formatted"; \
    else \
        echo "❌ Nightly toolchain required for formatting"; \
        echo "Install with: rustup install nightly"; \
        exit 1; \
    fi

# Check code formatting
format-check:
    @echo "Checking formatting..."
    cargo fmt --all -- --check
    @echo "✅ Formatting looks good!"

# Lint code with clippy
lint:
    @echo "Running clippy..."
    cargo clippy --all-targets -- -D warnings
    @echo "✅ Clippy checks passed!"

# Security audit
audit:
    @echo "Running security audit..."
    @if command -v cargo-audit >/dev/null 2>&1; then \
        cargo audit; \
        echo "✅ Security audit passed"; \
    else \
        echo "❌ cargo-audit not found. Install with: cargo install cargo-audit"; \
        exit 1; \
    fi

# Dependency compliance check
deny:
    @echo "Checking dependency compliance..."
    @if command -v cargo-deny >/dev/null 2>&1; then \
        cargo deny check; \
        echo "✅ Dependency compliance check passed"; \
    else \
        echo "❌ cargo-deny not found. Install with: cargo install cargo-deny"; \
        exit 1; \
    fi

# Full CI pipeline
ci: quality build-release
    @echo "✅ Full CI pipeline complete!"

# Development workflow - quick checks before commit
dev: format-check lint test
    @echo "✅ Development checks complete! Ready to commit."

# Run the built binary
run *args:
    cargo run -- {{ args }}

# Run the binary with release optimizations
run-release *args:
    cargo run --release -- {{ args }}
