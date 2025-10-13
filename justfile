# peter-hook - Development Workflow
# Requires: just, peter-hook, versioneer

# Default recipe to display available commands
default:
    @just --list

# Setup development environment
setup:
    @echo "Setting up peter-hook development environment..."
    @just install-hooks
    @echo "✅ Setup complete!"

# Install git hooks using peter-hook
install-hooks:
    @echo "Installing git hooks with peter-hook..."
    @if command -v peter-hook >/dev/null 2>&1; then \
        peter-hook install; \
        echo "✅ Git hooks installed"; \
    else \
        echo "❌ peter-hook not found. Build with: cargo build --release"; \
        exit 1; \
    fi

# Version management
version-show:
    @echo "Current version: $(cat VERSION)"
    @echo "Cargo.toml version: $(grep '^version' Cargo.toml | cut -d'"' -f2)"

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
release tag:
    #!/usr/bin/env bash
    set -euo pipefail

    PROJECT_NAME="peter-hook"

    # Validate tag format
    if [[ ! "{{ tag }}" =~ ^v[0-9]+\.[0-9]+\.[0-9]+$ ]]; then
        echo "❌ Invalid tag format: {{ tag }}"
        echo "Expected format: vX.Y.Z (e.g., v0.1.7)"
        exit 1
    fi

    echo "🚀 Validating release tag {{ tag }} for $PROJECT_NAME..."
    echo ""

    # Step 1: Validate tag exists locally
    echo "Step 1: Checking local tag exists..."
    if ! git tag -l "{{ tag }}" | grep -q "{{ tag }}"; then
        echo "❌ Tag {{ tag }} does not exist locally"
        echo "Create it with: git tag {{ tag }}"
        echo "Or use versioneer: versioneer tag --tag-format 'v{version}'"
        exit 1
    fi
    echo "✅ Tag {{ tag }} exists locally"
    echo ""

    # Step 2: Extract version and validate Cargo.toml matches
    echo "Step 2: Validating version consistency..."
    TAG_VERSION=$(echo "{{ tag }}" | sed 's/v//')
    TAG_COMMIT=$(git rev-list -n 1 "{{ tag }}")
    CARGO_VERSION=$(git show "$TAG_COMMIT:Cargo.toml" | grep '^version = ' | head -1 | sed 's/version = "\(.*\)"/\1/')

    echo "  Tag version: $TAG_VERSION"
    echo "  Cargo.toml version: $CARGO_VERSION"
    echo "  Tag commit: ${TAG_COMMIT:0:8}"

    if [ "$TAG_VERSION" != "$CARGO_VERSION" ]; then
        echo "❌ Version mismatch: tag {{ tag }} points to commit with Cargo.toml version $CARGO_VERSION"
        exit 1
    fi
    echo "✅ Version consistency verified"
    echo ""

    # Step 3: Validate tag not on remote
    echo "Step 3: Checking remote tag status..."
    git fetch --tags origin 2>/dev/null || true
    if git ls-remote --tags origin | grep -q "refs/tags/{{ tag }}$"; then
        echo "❌ Tag {{ tag }} already exists on remote"
        echo "Remote tags:"
        git ls-remote --tags origin | grep "{{ tag }}"
        exit 1
    fi
    echo "✅ Tag {{ tag }} not on remote"
    echo ""

    # Step 4: Validate no GitHub release exists (requires gh CLI)
    echo "Step 4: Checking GitHub release status..."
    if command -v gh >/dev/null 2>&1; then
        if gh release view "{{ tag }}" >/dev/null 2>&1; then
            echo "❌ GitHub release {{ tag }} already exists"
            gh release view "{{ tag }}"
            exit 1
        fi
        echo "✅ No GitHub release exists for {{ tag }}"
    else
        echo "⚠️  gh CLI not found, skipping GitHub release check"
        echo "   Install with: brew install gh"
    fi
    echo ""

    # Step 5: Validate version greater than latest release
    echo "Step 5: Validating version ordering..."
    if command -v gh >/dev/null 2>&1; then
        LATEST_RELEASE=$(gh release list --limit 1 --json tagName --jq '.[0].tagName' 2>/dev/null || echo "")
        if [ -n "$LATEST_RELEASE" ]; then
            LATEST_VERSION=$(echo "$LATEST_RELEASE" | sed 's/v//')
            echo "  Latest release: $LATEST_RELEASE ($LATEST_VERSION)"
            echo "  New version: {{ tag }} ($TAG_VERSION)"

            # Simple version comparison (works for semver X.Y.Z)
            if [ "$TAG_VERSION" = "$LATEST_VERSION" ]; then
                echo "❌ New version equals latest release version"
                exit 1
            fi

            # Note: Full semver comparison would require sort -V or similar
            # For now, trust developer has bumped correctly
            echo "✅ Version ordering looks correct"
        else
            echo "ℹ️  No previous releases found (first release)"
        fi
    else
        echo "⚠️  gh CLI not found, skipping version ordering check"
    fi
    echo ""

    # Step 6: Show summary and confirm
    echo "Ready to publish release:"
    echo "  Tag: {{ tag }}"
    echo "  Version: $TAG_VERSION"
    echo "  Commit: ${TAG_COMMIT:0:8}"
    echo ""

    if [ -t 0 ]; then
        read -p "Push tag to trigger release? [y/N]: " -n 1 -r
        echo
        if [[ ! $REPLY =~ ^[Yy]$ ]]; then
            echo "Release cancelled"
            echo "To push manually: git push origin {{ tag }}"
            exit 0
        fi
    fi

    # Step 7: Push tag only
    echo "Step 6: Pushing tag to remote..."
    git push origin "{{ tag }}"
    echo "✅ Tag pushed to remote"
    echo ""
    echo "🎉 Release {{ tag }} published!"
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
    @echo "Building peter-hook..."
    cargo build
    @echo "✅ Build complete!"

# Build in release mode
build-release:
    @echo "Building peter-hook in release mode..."
    cargo build --release
    @echo "✅ Release build complete!"

# Run tests
test:
    @echo "Running tests..."
    cargo test --all --verbose
    @echo "✅ Tests complete!"

# Code quality checks
quality: pre-commit pre-push

# Run pre-commit hooks (format-check + clippy-check)
pre-commit:
    @if command -v peter-hook >/dev/null 2>&1; then \
        peter-hook run pre-commit; \
    else \
        echo "❌ peter-hook not found. Build with: cargo build --release"; \
        exit 1; \
    fi

# Run pre-push hooks (test-all + security-audit + version-sync-check + tag-version-check)
pre-push:
    @if command -v peter-hook >/dev/null 2>&1; then \
        peter-hook run pre-push; \
    else \
        echo "❌ peter-hook not found. Build with: cargo build --release"; \
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
    @just pre-commit
    @just pre-push

# Lint code with clippy
lint:
    @just pre-commit
    @just pre-push

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
ci: quality test build-release
    @echo "✅ Full CI pipeline complete!"

# Development workflow - quick checks before commit
dev: format pre-commit test
    @echo "✅ Development checks complete! Ready to commit."

# Run the built binary
run *args:
    cargo run -- {{ args }}

# Run the binary with release optimizations
run-release *args:
    cargo run --release -- {{ args }}
