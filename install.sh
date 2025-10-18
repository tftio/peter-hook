#!/bin/bash
set -euo pipefail

# peter-hook installation script
# Usage: curl -fsSL https://raw.githubusercontent.com/tftio/peter-hook/main/install.sh | sh
# Or with custom install directory: INSTALL_DIR=/usr/local/bin curl ... | sh
# Or to force installation over same/newer version: FORCE_INSTALL=1 curl ... | sh

TOOL_NAME="peter-hook"
REPO_OWNER="${REPO_OWNER:-tftio}"
REPO_NAME="${REPO_NAME:-$TOOL_NAME}"
INSTALL_DIR="${INSTALL_DIR:-$HOME/.local/bin}"
FORCE_INSTALL="${FORCE_INSTALL:-0}"
GITHUB_API_URL="https://api.github.com"
GITHUB_DOWNLOAD_URL="https://github.com"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[0;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

log_info() {
    echo -e "${BLUE}[INFO]${NC} $1" >&2
}

log_success() {
    echo -e "${GREEN}[SUCCESS]${NC} $1" >&2
}

log_warn() {
    echo -e "${YELLOW}[WARN]${NC} $1" >&2
}

log_error() {
    echo -e "${RED}[ERROR]${NC} $1" >&2
}

# Detect OS and architecture
detect_platform() {
    local os arch target

    # Detect OS
    case "$(uname -s)" in
    Linux*) os="unknown-linux-gnu" ;;
    Darwin*) os="apple-darwin" ;;
    MINGW* | MSYS* | CYGWIN*) os="pc-windows-msvc" ;;
    *)
        log_error "Unsupported operating system: $(uname -s)"
        exit 1
        ;;
    esac

    # Detect architecture
    case "$(uname -m)" in
    x86_64 | amd64) arch="x86_64" ;;
    aarch64 | arm64) arch="aarch64" ;;
    *)
        log_error "Unsupported architecture: $(uname -m)"
        exit 1
        ;;
    esac

    target="${arch}-${os}"
    echo "$target"
}

# Compare two semantic versions
# Returns: 0 if v1 < v2, 1 if v1 == v2, 2 if v1 > v2
# Handles pre-release versions (e.g., 1.0.0-alpha < 1.0.0)
compare_versions() {
    local v1="$1"
    local v2="$2"

    # Strip leading 'v' if present
    v1="${v1#v}"
    v2="${v2#v}"

    # If versions are identical, return equal
    if [ "$v1" = "$v2" ]; then
        echo 1
        return
    fi

    # Split into base version and pre-release
    local v1_base="${v1%%-*}"
    local v1_pre=""
    if [[ "$v1" == *-* ]]; then
        v1_pre="${v1#*-}"
    fi

    local v2_base="${v2%%-*}"
    local v2_pre=""
    if [[ "$v2" == *-* ]]; then
        v2_pre="${v2#*-}"
    fi

    # Compare base versions using sort -V
    local sorted
    sorted=$(printf "%s\n%s\n" "$v1_base" "$v2_base" | sort -V | head -1)

    if [ "$sorted" != "$v1_base" ]; then
        # v1_base > v2_base
        echo 2
        return
    elif [ "$v1_base" != "$v2_base" ]; then
        # v1_base < v2_base
        echo 0
        return
    fi

    # Base versions are equal, check pre-release
    # Per semver: 1.0.0-alpha < 1.0.0
    if [ -n "$v1_pre" ] && [ -z "$v2_pre" ]; then
        # v1 has pre-release, v2 doesn't: v1 < v2
        echo 0
    elif [ -z "$v1_pre" ] && [ -n "$v2_pre" ]; then
        # v1 doesn't have pre-release, v2 does: v1 > v2
        echo 2
    elif [ -n "$v1_pre" ] && [ -n "$v2_pre" ]; then
        # Both have pre-release, compare lexicographically
        if [[ "$v1_pre" < "$v2_pre" ]]; then
            echo 0
        elif [[ "$v1_pre" > "$v2_pre" ]]; then
            echo 2
        else
            echo 1
        fi
    else
        # Both are equal (shouldn't reach here)
        echo 1
    fi
}

# Extract version from installed binary
# Returns the version string or empty string on failure
get_installed_version() {
    local binary_path="$1"

    if [ ! -f "$binary_path" ] || [ ! -x "$binary_path" ]; then
        echo ""
        return
    fi

    # Try different version command patterns
    local version_output

    # Try --version first (most common)
    if version_output=$("$binary_path" --version 2>/dev/null); then
        # Extract version string (common patterns: "tool 1.2.3" or "tool-name 1.2.3" or just "1.2.3")
        local version
        version=$(echo "$version_output" | grep -oE '[0-9]+\.[0-9]+\.[0-9]+(-[a-zA-Z0-9.+-]+)?' | head -1)
        if [ -n "$version" ]; then
            echo "$version"
            return
        fi
    fi

    # Try version subcommand (some tools use this)
    if version_output=$("$binary_path" version 2>/dev/null); then
        local version
        version=$(echo "$version_output" | grep -oE '[0-9]+\.[0-9]+\.[0-9]+(-[a-zA-Z0-9.+-]+)?' | head -1)
        if [ -n "$version" ]; then
            echo "$version"
            return
        fi
    fi

    # Could not determine version
    echo ""
}

# Get latest release version from GitHub API
get_latest_version() {
    local api_url="$GITHUB_API_URL/repos/$REPO_OWNER/$REPO_NAME/releases/latest"

    log_info "Fetching latest release information..."

    if command -v curl >/dev/null 2>&1; then
        curl -fsSL "$api_url" | grep '"tag_name":' | sed -E 's/.*"tag_name": *"([^"]+)".*/\1/'
    elif command -v wget >/dev/null 2>&1; then
        wget -qO- "$api_url" | grep '"tag_name":' | sed -E 's/.*"tag_name": *"([^"]+)".*/\1/'
    else
        log_error "Neither curl nor wget is available. Please install one of them."
        exit 1
    fi
}

# Download and verify checksum (mandatory)
download_and_verify() {
    local download_url="$1"
    local filename="$2"
    local temp_dir="$3"
    local version="$4"

    log_info "Downloading $filename..."

    if command -v curl >/dev/null 2>&1; then
        curl -fsSL "$download_url" -o "$temp_dir/$filename"
    elif command -v wget >/dev/null 2>&1; then
        wget -q "$download_url" -O "$temp_dir/$filename"
    else
        log_error "Neither curl nor wget is available."
        exit 1
    fi

    # Download and verify checksum (mandatory)
    # Checksum file is named without the archive extension (e.g., $TOOL_NAME-aarch64-apple-darwin.sha256)
    local base_filename="${filename%.tar.gz}"
    base_filename="${base_filename%.zip}"
    local checksum_url="${GITHUB_DOWNLOAD_URL}/${REPO_OWNER}/${REPO_NAME}/releases/download/${version}/${base_filename}.sha256"
    local checksum_file="$temp_dir/${base_filename}.sha256"

    log_info "Downloading checksum file..."
    if command -v curl >/dev/null 2>&1; then
        if ! curl -fsSL "$checksum_url" -o "$checksum_file" 2>/dev/null; then
            log_error "Checksum file not available at: $checksum_url"
            log_error "Checksum verification is mandatory for security."
            exit 1
        fi
    else
        log_error "curl is required for checksum download."
        exit 1
    fi

    log_info "Verifying checksum..."
    # Extract expected hash and verify directly
    local expected_hash
    expected_hash=$(cut -d' ' -f1 "$checksum_file")
    local actual_hash

    if command -v sha256sum >/dev/null 2>&1; then
        actual_hash=$(sha256sum "$temp_dir/$filename" | cut -d' ' -f1)
    elif command -v shasum >/dev/null 2>&1; then
        actual_hash=$(shasum -a 256 "$temp_dir/$filename" | cut -d' ' -f1)
    else
        log_error "No checksum utility available (sha256sum or shasum required)."
        log_error "Checksum verification is mandatory for security."
        exit 1
    fi

    if [ "$expected_hash" = "$actual_hash" ]; then
        log_success "Checksum verification passed"
    else
        log_error "Checksum verification failed!"
        log_error "Expected: $expected_hash"
        log_error "Actual:   $actual_hash"
        exit 1
    fi
}

# Extract archive based on file extension
extract_archive() {
    local archive_file="$1"
    local temp_dir="$2"

    case "$archive_file" in
    *.tar.gz | *.tgz)
        log_info "Extracting tar.gz archive..."
        tar -xzf "$temp_dir/$archive_file" -C "$temp_dir"
        ;;
    *.zip)
        log_info "Extracting zip archive..."
        if command -v unzip >/dev/null 2>&1; then
            unzip -q "$temp_dir/$archive_file" -d "$temp_dir"
        else
            log_error "unzip is not available. Please install unzip to extract the archive."
            exit 1
        fi
        ;;
    *)
        log_error "Unsupported archive format: $archive_file"
        exit 1
        ;;
    esac
}

# Check if binary needs to be replaced
check_existing_installation() {
    local install_path="$1"

    if [ -f "$install_path" ]; then
        if [ -t 0 ]; then # Check if we have a TTY (interactive)
            echo -n "$(basename "$install_path") is already installed at $install_path. Replace it? [y/N]: "
            read -r response
            case "$response" in
            [yY] | [yY][eE][sS])
                return 0
                ;;
            *)
                log_info "Installation cancelled by user"
                exit 0
                ;;
            esac
        else
            log_warn "$(basename "$install_path") already exists at $install_path, replacing..."
            return 0
        fi
    fi
}

main() {
    log_info "Installing $TOOL_NAME..."

    # Detect platform
    local target
    target=$(detect_platform)
    log_info "Detected platform: $target"

    # Get latest version
    local version
    version=$(get_latest_version)
    if [ -z "$version" ]; then
        log_error "Failed to get latest version"
        exit 1
    fi
    log_info "Latest version: $version"

    # Create install directory if it doesn't exist
    mkdir -p "$INSTALL_DIR"

    # Determine install path
    local install_path="$INSTALL_DIR/$TOOL_NAME"
    if [ "$(uname -s)" = "MINGW*" ] || [ "$(uname -s)" = "MSYS*" ] || [ "$(uname -s)" = "CYGWIN*" ]; then
        install_path="${install_path}.exe"
    fi

    # Check version BEFORE downloading anything
    if [ -f "$install_path" ]; then
        log_info "Found existing installation at $install_path"

        local installed_version
        installed_version=$(get_installed_version "$install_path")

        if [ -z "$installed_version" ]; then
            log_error "Cannot determine version of installed binary at $install_path"
            log_error "This may indicate a corrupted or incompatible installation"
            log_error "Please manually remove the file and try again"
            exit 1
        fi

        log_info "Installed version: $installed_version"
        log_info "Available version: $version"

        # Compare versions
        local comparison
        comparison=$(compare_versions "$installed_version" "$version")

        case $comparison in
        1) # Equal
            if [ "$FORCE_INSTALL" = "1" ]; then
                log_warn "Installed version $installed_version equals available version $version"
                log_info "Proceeding with reinstallation due to FORCE_INSTALL=1"
            else
                log_success "Already have version $installed_version installed (same as latest)"
                log_info "Use FORCE_INSTALL=1 to reinstall anyway"
                exit 0
            fi
            ;;
        2) # Installed > Available
            if [ "$FORCE_INSTALL" = "1" ]; then
                log_warn "Installed version $installed_version is newer than available version $version"
                log_info "Proceeding with downgrade due to FORCE_INSTALL=1"
            else
                log_success "Already have version $installed_version installed (newer than $version)"
                log_info "Use FORCE_INSTALL=1 to downgrade anyway"
                exit 0
            fi
            ;;
        0) # Installed < Available
            log_info "Upgrading from $installed_version to $version"
            ;;
        esac
    fi

    # Construct download URL
    local filename="${TOOL_NAME}-${target}.tar.gz"
    local download_url="$GITHUB_DOWNLOAD_URL/$REPO_OWNER/$REPO_NAME/releases/download/$version/$filename"

    # Create temporary directory
    local temp_dir
    temp_dir=$(mktemp -d)
    # shellcheck disable=SC2064
    trap "rm -rf \"$temp_dir\"" EXIT

    # Download and verify
    download_and_verify "$download_url" "$filename" "$temp_dir" "$version"

    # Extract archive
    extract_archive "$filename" "$temp_dir"

    # Find the binary (handle potential directory structure)
    local binary_name="$TOOL_NAME"
    if [ "$(uname -s)" = "MINGW*" ] || [ "$(uname -s)" = "MSYS*" ] || [ "$(uname -s)" = "CYGWIN*" ]; then
        binary_name="${TOOL_NAME}.exe"
    fi

    local binary_path
    if [ -f "$temp_dir/$binary_name" ]; then
        binary_path="$temp_dir/$binary_name"
    else
        # Look for binary in subdirectories
        binary_path=$(find "$temp_dir" -name "$binary_name" -type f | head -1)
        if [ -z "$binary_path" ]; then
            log_error "Could not find $binary_name in the extracted archive"
            exit 1
        fi
    fi

    # Check for existing installation and prompt if needed
    check_existing_installation "$install_path"

    # Install binary
    log_info "Installing to $install_path..."
    cp "$binary_path" "$install_path"
    chmod +x "$install_path"

    log_success "$TOOL_NAME installed successfully!"
    log_info "Binary location: $install_path"

    # Check if install directory is in PATH
    case ":$PATH:" in
    *":$INSTALL_DIR:"*)
        log_success "$INSTALL_DIR is already in your PATH"
        ;;
    *)
        log_warn "$INSTALL_DIR is not in your PATH"
        log_info "Add it to your PATH by adding this line to your shell configuration file:"
        log_info "  export PATH=\"$INSTALL_DIR:\$PATH\""
        ;;
    esac

    # Test installation
    if command -v "$TOOL_NAME" >/dev/null 2>&1; then
        log_success "Installation verified: $TOOL_NAME is available"
        log_info "Version: $("$TOOL_NAME" --version 2>/dev/null || "$TOOL_NAME" version 2>/dev/null || echo "unable to determine")"
    else
        log_warn "Installation completed, but $TOOL_NAME is not immediately available"
        log_info "You may need to restart your shell or source your shell configuration"
    fi
}

main "$@"
