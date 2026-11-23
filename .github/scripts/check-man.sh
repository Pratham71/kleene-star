#!/bin/sh
# Renders every man page in man/ with groff and man, and fails on any
# warning; checks the info manual with info and install-info when they are
# installed. Run from the repository's root: .github/scripts/check-man.sh
set -eu
status=0
for page in man/*.1; do
    out=$(groff -man -Tutf8 -ww -z "$page" 2>&1) || status=1
    if [ -n "$out" ]; then echo "$page (groff): $out"; status=1; fi
    out=$(LC_ALL=C.UTF-8 MANWIDTH=80 man --warnings -E UTF-8 -l "$page" 2>&1 >/dev/null) || status=1
    if [ -n "$out" ]; then echo "$page (man): $out"; status=1; fi
done
if command -v info >/dev/null 2>&1; then
    for node in Top run export "Standard library" "std automata"; do
        info -f man/kstar.info -n "$node" -o - 2>&1 | grep -q 'File: kstar.info' && continue
        out=$(info -f man/kstar.info -n "$node" -o - 2>&1 | head -n 3)
        echo "man/kstar.info: node $node: $out"
        status=1
    done
fi
if command -v install-info >/dev/null 2>&1; then
    dir=$(mktemp -d)
    install-info --info-dir="$dir" man/kstar.info || status=1
    grep -q '(kstar)' "$dir/dir" || { echo "install-info did not add kstar to the dir file"; status=1; }
    rm -rf "$dir"
fi
[ "$status" = 0 ] && echo "man pages and info manual: no warnings"
exit "$status"
