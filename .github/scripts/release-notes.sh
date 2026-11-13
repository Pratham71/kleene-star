#!/bin/sh
# Prints the notes of a GitHub release: the section of CHANGELOG.md for the
# version, then how to install it and how to check the download.
#
#   .github/scripts/release-notes.sh v0.2.5-beta > notes.md
#
# The release workflow runs it for every version tag, and CI runs it for the
# version in Cargo.toml, so a release cannot be tagged without its
# changelog section. It fails if CHANGELOG.md has no section for the version.

set -eu

tag=${1:?usage: release-notes.sh <tag, such as v0.2.3>}
version=${tag#v}
repo=Pratham71/kleene-star
blob=https://github.com/$repo/blob/main

cd "$(dirname "$0")/../.."

# The lines after "## [0.2.3] - date", up to the next version or the link
# definitions at the end of the file.
section=$(awk -v heading="## [$version]" '
    index($0, heading) == 1 { found = 1; next }
    found && (/^## \[/ || /^\[[^]]*\]: /) { exit }
    found { print }
' CHANGELOG.md)

if [ -z "$(printf '%s' "$section" | tr -d '[:space:]')" ]; then
    echo "release-notes.sh: CHANGELOG.md has no section for $version (a line starting \"## [$version]\")" >&2
    exit 1
fi

# Links such as (docs/install.md#updating) are relative to the repository;
# on the release page they need the full address.
printf '%s\n' "$section" |
    sed -e "s|](\([^)h#][^)]*\))|]($blob/\1)|g" |
    sed -e '/./,$!d' # drop leading blank lines

case "$tag" in
    *-*) beta=1 ;;
    *) beta= ;;
esac

raw=https://raw.githubusercontent.com/$repo/main
echo
echo "## Installing"
echo
if [ -n "$beta" ]; then
    echo "This is a pre-release. The install scripts and \`kstar update\` use the latest stable release unless you ask for this one:"
    echo
    echo "- **Windows:** download and run \`kstar-setup-x64.exe\` below, or in PowerShell:"
    echo "  \`\$env:KSTAR_VERSION = \"$tag\"; irm $raw/install.ps1 | iex\`"
    echo "- **Linux and macOS:** \`curl -fsSL $raw/install.sh | KSTAR_VERSION=$tag sh\`"
else
    echo "- **Windows:** download and run \`kstar-setup-x64.exe\` below, or in PowerShell:"
    echo "  \`powershell -c \"irm $raw/install.ps1 | iex\"\`"
    echo "- **Linux and macOS:** \`curl -fsSL $raw/install.sh | sh\`"
fi
echo "- **VS Code:** download \`kleene-star-$version.vsix\` below and run \`code --install-extension kleene-star-$version.vsix\`."
echo
echo "The [installation guide]($blob/docs/install.md) has every option, including building from source, updating and uninstalling."
echo
echo "## Checking the download"
echo
echo "Every file has its SHA-256 checksum beside it (\`<file>.sha256\`), and \`SHA256SUMS\` lists them all. The install scripts check it for you. To check a file by hand, in the folder you downloaded to:"
echo
echo "- **Linux:** \`sha256sum -c SHA256SUMS --ignore-missing\`"
echo "- **macOS:** \`shasum -a 256 -c <file>.sha256\`"
echo "- **Windows (PowerShell):** \`(Get-FileHash <file>).Hash -eq (Get-Content <file>.sha256).Split(\" \")[0]\`"
