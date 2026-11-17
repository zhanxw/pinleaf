#!/bin/sh
# Pinleaf one-line installer for macOS
# Usage: curl -fsSL https://raw.githubusercontent.com/zhanxw/pinleaf/main/install.sh | sh

set -eu

main() {
    repo="${PINLEAF_REPO:-zhanxw/pinleaf}"
    requested_version="${PINLEAF_VERSION:-}"
    install_dir="${PINLEAF_INSTALL_DIR:-}"
    bin_dir="${PINLEAF_BIN_DIR:-}"
    install_cli=1

    while [ "$#" -gt 0 ]; do
        case "$1" in
            --version|-v)
                [ "$#" -ge 2 ] || { echo "Error: --version requires a value" >&2; exit 2; }
                requested_version="$2"
                shift 2
                ;;
            --app-dir)
                [ "$#" -ge 2 ] || { echo "Error: --app-dir requires a directory" >&2; exit 2; }
                install_dir="$2"
                shift 2
                ;;
            --bin-dir)
                [ "$#" -ge 2 ] || { echo "Error: --bin-dir requires a directory" >&2; exit 2; }
                bin_dir="$2"
                shift 2
                ;;
            --no-cli)
                install_cli=0
                shift
                ;;
            --help|-h)
                cat <<'EOF'
Pinleaf macOS Installer

Usage:
  curl -fsSL https://raw.githubusercontent.com/zhanxw/pinleaf/main/install.sh | sh
  curl -fsSL https://raw.githubusercontent.com/zhanxw/pinleaf/main/install.sh | sh -s -- [OPTIONS]

Options:
  --version <tag>   Install a specific release tag (e.g. v0.2.1)
  --app-dir <dir>   Target folder for Pinleaf.app (default: /Applications or ~/Applications)
  --bin-dir <dir>   Target folder for `pinleaf` CLI symlink (default: /opt/homebrew/bin, /usr/local/bin, or ~/.local/bin)
  --no-cli          Install Pinleaf.app only without creating the `pinleaf` CLI symlink
  --help            Show this help message

Environment overrides:
  PINLEAF_VERSION, PINLEAF_INSTALL_DIR, PINLEAF_BIN_DIR, PINLEAF_REPO
EOF
                exit 0
                ;;
            *)
                echo "Unknown option: $1 (pass --help for usage)" >&2
                exit 2
                ;;
        esac
    done

    os_name="$(uname -s)"
    if [ "$os_name" != "Darwin" ]; then
        echo "Pinleaf prebuilt app bundles currently target macOS (Darwin), detected '$os_name'." >&2
        echo "You can build from source with Cargo: cargo install --git https://github.com/${repo} pinleaf" >&2
        exit 1
    fi

    for cmd in curl mktemp uname; do
        if ! command -v "$cmd" >/dev/null 2>&1; then
            echo "Error: required tool '$cmd' not found in PATH." >&2
            exit 1
        fi
    done

    tag=""
    if [ -n "$requested_version" ] && [ "$requested_version" != "latest" ]; then
        case "$requested_version" in
            v*) tag="$requested_version" ;;
            *) tag="v$requested_version" ;;
        esac
    else
        latest_url="$(curl -fsSLI -o /dev/null -w '%{url_effective}' "https://github.com/${repo}/releases/latest" 2>/dev/null || true)"
        case "$latest_url" in
            */releases/tag/*)
                tag="${latest_url##*/releases/tag/}"
                tag="${tag%%\?*}"
                ;;
        esac
        if [ -z "$tag" ] || [ "$tag" = "latest" ]; then
            api_json="$(curl -fsSL "https://api.github.com/repos/${repo}/releases/latest" 2>/dev/null || true)"
            tag="$(printf '%s' "$api_json" | sed -n 's/.*"tag_name":[[:space:]]*"\([^"]*\)".*/\1/p' | head -n 1)"
        fi
    fi

    if [ -z "$tag" ]; then
        echo "Error: could not resolve latest Pinleaf release tag from https://github.com/${repo}/releases." >&2
        exit 1
    fi

    archive_name="Pinleaf-${tag}-macos-universal.app.zip"
    base_url="https://github.com/${repo}/releases/download/${tag}"
    archive_url="${base_url}/${archive_name}"
    checksum_url="${base_url}/SHA256SUMS.txt"

    tmp_dir="$(mktemp -d "${TMPDIR:-/tmp}/pinleaf-install.XXXXXX")"
    trap 'rm -rf "$tmp_dir"' EXIT INT TERM

    echo "==> Installing Pinleaf ${tag} for macOS (universal arm64 + x86_64)..."
    echo "==> Downloading ${archive_name}..."
    curl -fL --progress-bar "$archive_url" -o "$tmp_dir/$archive_name"

    if curl -fsSL "$checksum_url" -o "$tmp_dir/SHA256SUMS.txt" 2>/dev/null; then
        expected_line="$(grep "  ${archive_name}\$" "$tmp_dir/SHA256SUMS.txt" || grep " ${archive_name}\$" "$tmp_dir/SHA256SUMS.txt" || true)"
        if [ -n "$expected_line" ]; then
            expected_sha="$(printf '%s' "$expected_line" | awk '{print $1}')"
            actual_sha="$(/usr/bin/shasum -a 256 "$tmp_dir/$archive_name" | awk '{print $1}')"
            if [ "$expected_sha" != "$actual_sha" ]; then
                echo "Error: SHA-256 mismatch for ${archive_name}!" >&2
                echo "  expected: $expected_sha" >&2
                echo "  actual:   $actual_sha" >&2
                exit 1
            fi
            echo "==> SHA-256 verified (${actual_sha})."
        else
            echo "Warning: ${archive_name} entry not found in SHA256SUMS.txt; skipping checksum check." >&2
        fi
    else
        echo "Warning: could not fetch SHA256SUMS.txt; skipping checksum check." >&2
    fi

    unpack_dir="$tmp_dir/unpack"
    mkdir -p "$unpack_dir"
    /usr/bin/ditto -x -k "$tmp_dir/$archive_name" "$unpack_dir"

    if [ ! -d "$unpack_dir/Pinleaf.app" ]; then
        echo "Error: Pinleaf.app was not found inside ${archive_name}." >&2
        exit 1
    fi

    if [ -z "$install_dir" ]; then
        if [ -d "/Applications" ] && [ -w "/Applications" ]; then
            install_dir="/Applications"
        else
            install_dir="$HOME/Applications"
        fi
    fi

    mkdir -p "$install_dir"
    target_app="${install_dir%/}/Pinleaf.app"
    staging_app="${install_dir%/}/.Pinleaf.app.staging.$$"

    rm -rf "$staging_app"
    /usr/bin/ditto "$unpack_dir/Pinleaf.app" "$staging_app"
    rm -rf "$target_app"
    mv "$staging_app" "$target_app"

    /usr/bin/xattr -dr com.apple.quarantine "$target_app" 2>/dev/null || true
    /usr/bin/codesign --verify --deep --strict "$target_app"

    lsregister="/System/Library/Frameworks/CoreServices.framework/Frameworks/LaunchServices.framework/Support/lsregister"
    if [ -x "$lsregister" ]; then
        "$lsregister" -f "$target_app" >/dev/null 2>&1 || true
    fi

    echo "==> Installed app bundle: ${target_app}"

    if [ "$install_cli" -eq 1 ]; then
        if [ -z "$bin_dir" ]; then
            case ":${PATH}:" in
                *:/opt/homebrew/bin:*)
                    if [ -d "/opt/homebrew/bin" ] && [ -w "/opt/homebrew/bin" ]; then
                        bin_dir="/opt/homebrew/bin"
                    fi
                    ;;
            esac
        fi
        if [ -z "$bin_dir" ]; then
            case ":${PATH}:" in
                *:/usr/local/bin:*)
                    if [ -d "/usr/local/bin" ] && [ -w "/usr/local/bin" ]; then
                        bin_dir="/usr/local/bin"
                    fi
                    ;;
            esac
        fi
        if [ -z "$bin_dir" ]; then
            bin_dir="$HOME/.local/bin"
        fi

        mkdir -p "$bin_dir"
        cli_link="${bin_dir%/}/pinleaf"
        ln -sf "$target_app/Contents/MacOS/pinleaf" "$cli_link"
        echo "==> Linked CLI binary: ${cli_link}"

        case ":${PATH}:" in
            *:"${bin_dir%/}":*) ;;
            *)
                echo "Note: add ${bin_dir} to your PATH to invoke \`pinleaf\` from any shell:"
                echo "  export PATH=\"${bin_dir}:\$PATH\""
                ;;
        esac
    fi

    echo ""
    echo "Pinleaf ${tag} is ready! Launch with:"
    echo "  open -a \"${target_app}\""
}

main "$@"
