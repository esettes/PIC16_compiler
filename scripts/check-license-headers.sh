#!/usr/bin/env bash
# SPDX-License-Identifier: GPL-3.0-or-later

set -euo pipefail

repo_root="$(cd "$(dirname "$0")/.." && pwd)"
cd "$repo_root"

gpl_header="SPDX-License-Identifier: GPL-3.0-or-later"
runtime_header="SPDX-License-Identifier: GPL-3.0-or-later WITH GCC-exception-3.1"
status=0

check_header() {
    local file="$1"
    local expected="$2"
    local top

    top="$(sed -n '1,3p' "$file")"

    if ! printf '%s\n' "$top" | grep -Fq "$expected"; then
        printf 'missing or wrong SPDX header: %s\n' "$file" >&2
        status=1
        return
    fi

    if [ "$expected" = "$gpl_header" ] && printf '%s\n' "$top" | grep -Fq "$runtime_header"; then
        printf 'runtime exception not allowed here: %s\n' "$file" >&2
        status=1
    fi

    if [ "$expected" = "$runtime_header" ] && ! printf '%s\n' "$top" | grep -Fq "$runtime_header"; then
        printf 'runtime exception required here: %s\n' "$file" >&2
        status=1
    fi
}

while IFS= read -r file; do
    check_header "$file" "$gpl_header"
done < <(
    {
        find src tests -type f -name '*.rs'
        find docs -type f \( -name '*.md' -o -name '*.py' \)
        find examples -type f \( -name '*.c' -o -name 'Makefile' -o -name 'Makefile.template' \)
        find scripts -type f
        printf '%s\n' Cargo.toml README.md DESIGN.md CONTRIBUTING.md
    } | sort -u
)

while IFS= read -r file; do
    check_header "$file" "$runtime_header"
done < <(
    {
        if [ -d include ]; then
            find include -type f
        fi
        if [ -d runtime ]; then
            find runtime -type f
        fi
    } | sort -u
)

if [ "$status" -ne 0 ]; then
    exit "$status"
fi

printf 'SPDX headers OK\n'
