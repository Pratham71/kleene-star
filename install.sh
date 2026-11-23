#!/bin/sh
# Installs Kleene Star (kstar and kstar-lsp) on Linux or macOS for the
# current user:
#
#   curl -fsSL https://raw.githubusercontent.com/Pratham71/kleene-star/main/install.sh | sh
#
# Or through the GitHub CLI, which also works for a private fork (after
# `gh auth login`):
#
#   gh api repos/Pratham71/kleene-star/contents/install.sh -H "Accept: application/vnd.github.raw" | sh
#
# It downloads the latest release for this machine, checks its SHA-256
# checksum, puts the programs in ~/.kstar/bin, the man pages in
# ~/.kstar/share/man (so `man kstar` works) and the info manual in
# ~/.kstar/share/info, and adds ~/.kstar/bin to your PATH in your shell's
# startup file.
#
# To uninstall, run it with --uninstall:
#
#   curl -fsSL https://raw.githubusercontent.com/Pratham71/kleene-star/main/install.sh | sh -s -- --uninstall
#
# Settings, all optional, as environment variables:
#
#   KSTAR_VERSION       a release tag such as v0.2.0 (default: the latest)
#   KSTAR_INSTALL_DIR   where to install (default: ~/.kstar)
#   KSTAR_REPO          the GitHub repository (default: Pratham71/kleene-star)
#   KSTAR_INSTALL_FROM  a folder that already holds the release files; used
#                       to test this script without downloading anything
#   KSTAR_NO_PATH       set to 1 to leave your shell's startup files alone
#   KSTAR_NO_VSCODE     set to 1 to leave VS Code alone
#   KSTAR_CODE          VS Code's code command, if it is not code on the PATH
#
# If VS Code is installed (the `code` command), the script also installs (or
# updates) the Kleene Star extension of the same release,
# kleene-star-<version>.vsix, after checking its checksum too, so the editor
# and the language server match.

set -eu

repo=${KSTAR_REPO:-Pratham71/kleene-star}
tag=${KSTAR_VERSION:-}
dir=${KSTAR_INSTALL_DIR:-$HOME/.kstar}
bin=$dir/bin
# `man` looks in ../share/man next to every folder on the PATH, so pages
# here are found through the PATH entry for $bin.
man_dir=$dir/share/man/man1
info_dir=$dir/share/info
line="export PATH=\"$bin:\$PATH\""
comment="# Added by the Kleene Star installer"

fail() {
    echo "kstar install: $*" >&2
    exit 1
}

# Removes what this script installed: the programs, the man pages, the info
# manual, the folders they were in if nothing else is there, and the PATH
# lines it added to the shell's startup files.
uninstall() {
    [ -e "$bin/kstar" ] || [ -e "$man_dir/kstar.1" ] || fail "Kleene Star is not installed in $dir"
    rm -f "$bin/kstar" "$bin/kstar-lsp" "$man_dir"/kstar.1 "$man_dir"/kstar-*.1
    if [ -f "$info_dir/kstar.info" ]; then
        if command -v install-info >/dev/null 2>&1; then
            install-info --delete --info-dir="$info_dir" "$info_dir/kstar.info" >/dev/null 2>&1 || true
        fi
        rm -f "$info_dir/kstar.info"
        # The dir file, if it lists no other manual: entries look like
        # "* name: (file).", and its "* Menu:" line is not one.
        if [ -f "$info_dir/dir" ] && ! grep -q '^\* [^:]*: *(' "$info_dir/dir"; then rm -f "$info_dir/dir"; fi
    fi
    for d in "$man_dir" "$dir/share/man" "$info_dir" "$dir/share" "$bin" "$dir"; do
        rmdir "$d" 2>/dev/null || true
    done
    for rc in "$HOME/.profile" "$HOME/.bashrc" "$HOME/.zshrc"; do
        if grep -qsxF "$line" "$rc"; then
            kept=$(grep -vxF -e "$line" -e "$comment" "$rc" || true)
            printf '%s\n' "$kept" >"$rc"
            echo "Removed the PATH line from $rc"
        fi
    done
    echo "Uninstalled Kleene Star from $dir."
}

case "${1:-}" in
    "") ;;
    --uninstall)
        uninstall
        exit 0
        ;;
    *) fail "unknown option $1; the only option is --uninstall" ;;
esac

# Downloads release file $2 of tag $1 into $3.
fetch() {
    if [ -n "${KSTAR_INSTALL_FROM:-}" ]; then
        cp "$KSTAR_INSTALL_FROM/$2" "$3/"
    elif command -v gh >/dev/null 2>&1; then
        gh release download "$1" -R "$repo" -p "$2" -D "$3"
    elif command -v curl >/dev/null 2>&1; then
        curl -fsSL "https://github.com/$repo/releases/download/$1/$2" -o "$3/$2"
    else
        wget -q "https://github.com/$repo/releases/download/$1/$2" -O "$3/$2"
    fi
}

sha256() {
    if command -v sha256sum >/dev/null 2>&1; then
        sha256sum "$1" | cut -d ' ' -f 1
    else
        shasum -a 256 "$1" | cut -d ' ' -f 1
    fi
}

# Installs the extension of release $1 in VS Code, if VS Code is installed.
# A problem here is a warning: kstar itself is installed already.
install_extension() {
    code=${KSTAR_CODE:-code}
    command -v "$code" >/dev/null 2>&1 || return 0
    vsix=kleene-star-$1.vsix
    if fetch "v$1" "$vsix" "$work" >/dev/null 2>&1 && fetch "v$1" "$vsix.sha256" "$work" >/dev/null 2>&1 &&
        [ "$(cut -d ' ' -f 1 "$work/$vsix.sha256" | tr 'A-F' 'a-f')" = "$(sha256 "$work/$vsix")" ] &&
        "$code" --install-extension "$work/$vsix" --force >/dev/null 2>&1; then
        echo "Installed the Kleene Star extension $1 in VS Code."
    else
        echo "kstar install: warning: the VS Code extension was not installed; run 'kstar update' later to try again" >&2
    fi
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
            fail "could not download $file. If $repo is private, install the GitHub CLI, run 'gh auth login' and try again"
    done
fi

expected=$(cut -d ' ' -f 1 "$work/$asset.sha256" | tr 'A-F' 'a-f')
actual=$(sha256 "$work/$asset")
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

# The man pages and the info manual (in the Linux and macOS archives).
if [ -n "$(find "$work/unpacked" -type f -name 'kstar.1')" ]; then
    mkdir -p "$man_dir"
    # Pages of an older release go, in case one was renamed.
    rm -f "$man_dir"/kstar.1 "$man_dir"/kstar-*.1
    find "$work/unpacked" -type f -name 'kstar*.1' -exec cp {} "$man_dir/" \;
    chmod 644 "$man_dir"/kstar*.1
fi
info=$(find "$work/unpacked" -type f -name kstar.info | head -n 1)
if [ -n "$info" ]; then
    mkdir -p "$info_dir"
    cp "$info" "$info_dir/kstar.info"
    chmod 644 "$info_dir/kstar.info"
    if command -v install-info >/dev/null 2>&1; then
        install-info --info-dir="$info_dir" "$info_dir/kstar.info" >/dev/null 2>&1 || true
    fi
fi

echo
"$bin/kstar" version
echo
echo "Installed kstar and kstar-lsp in $bin"
if [ -f "$man_dir/kstar.1" ]; then
    echo "Installed the man pages (man kstar) in $dir/share/man and the info manual in $info_dir"
fi

if [ "${KSTAR_NO_VSCODE:-}" != 1 ]; then
    install_extension "$("$bin/kstar" version | head -n 1 | cut -d ' ' -f 2)"
fi

if [ "${KSTAR_NO_PATH:-}" != 1 ]; then
    case ":$PATH:" in
        *":$bin:"*) echo "$bin is already on your PATH." ;;
        *)
            added=
            for rc in "$HOME/.profile" "$HOME/.bashrc" "$HOME/.zshrc"; do
                # .profile always; the others only if that shell is in use.
                if [ "$rc" = "$HOME/.profile" ] || [ -f "$rc" ]; then
                    if ! grep -qsF "$line" "$rc"; then
                        printf '\n%s\n%s\n' "$comment" "$line" >>"$rc"
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
echo "To uninstall: sh install.sh --uninstall (or the same curl line, ending in | sh -s -- --uninstall)."
