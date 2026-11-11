#!/bin/sh
# Installs Kleene Star (kstar and kstar-lsp) on Linux or macOS for the
# current user:
#
#   curl -fsSL https://raw.githubusercontent.com/Pratham71/kleene-star/main/install.sh | sh
#
# While the repository is private, that URL needs signing in, so use the
# GitHub CLI (after `gh auth login`) instead:
#
#   gh api repos/Pratham71/kleene-star/contents/install.sh -H "Accept: application/vnd.github.raw" | sh
#
# It downloads the latest release for this machine, checks its SHA-256
# checksum, puts the programs in ~/.kstar/bin and adds that folder to your
# PATH in your shell's startup file. Settings, all optional, as environment
# variables:
#
#   KSTAR_VERSION       a release tag such as v0.2.0 (default: the latest)
#   KSTAR_INSTALL_DIR   where to install (default: ~/.kstar)
#   KSTAR_REPO          the GitHub repository (default: Pratham71/kleene-star)
#   KSTAR_INSTALL_FROM  a folder that already holds the release files; used
#                       to test this script without downloading anything
#   KSTAR_NO_PATH       set to 1 to leave your shell's startup files alone

set -eu

repo=${KSTAR_REPO:-Pratham71/kleene-star}
tag=${KSTAR_VERSION:-}
dir=${KSTAR_INSTALL_DIR:-$HOME/.kstar}
bin=$dir/bin

fail() {
    echo "kstar install: $*" >&2
    exit 1
}

case "$(uname -s)" in
    Linux) os=unknown-linux-gnu ;;
    Darwin) os=apple-darwin ;;
    *) fail "there is no build for $(uname -s); on Windows use install.ps1" ;;
esac
case "$(uname -m)" in
    x86_64 | amd64) arch=x86_64 ;;
    arm64 | aarch64) arch=aarch64 ;;
    *) fail "there is no build for $(uname -m) processors" ;;
esac
if [ "$os" = unknown-linux-gnu ] && [ "$arch" = aarch64 ]; then
    fail "there is no build for ARM Linux yet; build from source with cargo instead"
fi
asset=kstar-$arch-$os.tar.gz

work=$(mktemp -d 2>/dev/null || mktemp -d -t kstar-install)
trap 'rm -rf "$work"' EXIT INT TERM

if [ -n "${KSTAR_INSTALL_FROM:-}" ]; then
    echo "Using the release files in $KSTAR_INSTALL_FROM"
    cp "$KSTAR_INSTALL_FROM/$asset" "$KSTAR_INSTALL_FROM/$asset.sha256" "$work/"
elif command -v gh >/dev/null 2>&1; then
    echo "Downloading $asset from $repo ${tag:-(latest release)} with the GitHub CLI"
    # shellcheck disable=SC2086
    gh release download $tag -R "$repo" -p "$asset" -p "$asset.sha256" -D "$work" ||
        fail "gh could not download the release. Is it signed in (gh auth login), and can this account see $repo?"
else
    if [ -n "$tag" ]; then
        base=https://github.com/$repo/releases/download/$tag
    else
        base=https://github.com/$repo/releases/latest/download
    fi
    echo "Downloading $base/$asset"
    for file in "$asset" "$asset.sha256"; do
        if command -v curl >/dev/null 2>&1; then
            curl -fsSL "$base/$file" -o "$work/$file" || failed=1
        else
            wget -q "$base/$file" -O "$work/$file" || failed=1
        fi
        [ -z "${failed:-}" ] ||
            fail "could not download $file. While $repo is private, install the GitHub CLI, run 'gh auth login' and try again"
    done
fi

expected=$(cut -d ' ' -f 1 "$work/$asset.sha256" | tr 'A-F' 'a-f')
if command -v sha256sum >/dev/null 2>&1; then
    actual=$(sha256sum "$work/$asset" | cut -d ' ' -f 1)
else
    actual=$(shasum -a 256 "$work/$asset" | cut -d ' ' -f 1)
fi
[ "$expected" = "$actual" ] ||
    fail "the checksum of $asset does not match (expected $expected, got $actual). Nothing was installed"
echo "Checksum verified."

mkdir -p "$work/unpacked" "$bin"
tar -xzf "$work/$asset" -C "$work/unpacked"
for program in kstar kstar-lsp; do
    found=$(find "$work/unpacked" -type f -name "$program" | head -n 1)
    [ -n "$found" ] || fail "$program is missing from $asset"
    cp "$found" "$bin/$program"
    chmod 755 "$bin/$program"
done

echo
"$bin/kstar" version
echo
echo "Installed kstar and kstar-lsp in $bin"

line="export PATH=\"$bin:\$PATH\""
if [ "${KSTAR_NO_PATH:-}" != 1 ]; then
    case ":$PATH:" in
        *":$bin:"*) echo "$bin is already on your PATH." ;;
        *)
            added=
            for rc in "$HOME/.profile" "$HOME/.bashrc" "$HOME/.zshrc"; do
                # .profile always; the others only if that shell is in use.
                if [ "$rc" = "$HOME/.profile" ] || [ -f "$rc" ]; then
                    if ! grep -qsF "$line" "$rc"; then
                        printf '\n# Added by the Kleene Star installer\n%s\n' "$line" >>"$rc"
                    fi
                    added="$added $rc"
                fi
            done
            echo "Added $bin to your PATH in:$added"
            echo "Open a new terminal, or run: $line"
            ;;
    esac
fi

echo
echo "To uninstall: rm -rf \"$dir\" and delete the line"
echo "    $line"
echo "(and the comment above it) from ~/.profile, ~/.bashrc and ~/.zshrc."
