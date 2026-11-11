#!/usr/bin/env bash
# Packages a release build for the release workflow:
#
#   .github/scripts/package.sh <target> <exe suffix>
#
# puts kstar, kstar-lsp and the README into dist/kstar-<target>.zip (on
# Windows) or .tar.gz, with a .sha256 file beside it, after checking that
# the built kstar runs.
set -euo pipefail

target=$1
exe=${2:-}
name=kstar-$target
release=target/$target/release

"$release/kstar$exe" version
"$release/kstar$exe" run examples/01_dfa.kstar >/dev/null

rm -rf stage dist
mkdir -p stage dist
cp "$release/kstar$exe" "$release/kstar-lsp$exe" README.md stage/

if [ "$exe" = .exe ]; then
    archive=$name.zip
    (cd stage && 7z a -tzip "../dist/$archive" . >/dev/null)
else
    archive=$name.tar.gz
    tar -czf "dist/$archive" -C stage .
fi

cd dist
if command -v sha256sum >/dev/null; then
    sha256sum "$archive" >"$archive.sha256"
else
    shasum -a 256 "$archive" >"$archive.sha256"
fi
cat "$archive.sha256"
