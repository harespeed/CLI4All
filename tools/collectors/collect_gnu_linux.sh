#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
output_dir="$repo_root/data/raw/generated"
output_path="$output_dir/gnu_linux_commands.json"
shell_name="${SHELL##*/}"

mkdir -p "$output_dir"

tmp_file="$(mktemp)"
cleanup() {
  rm -f "$tmp_file"
}
trap cleanup EXIT

if command -v bash >/dev/null 2>&1; then
  bash -lc '
    if builtin compgen -b >/dev/null 2>&1; then
      compgen -b | while read -r name; do
        printf "builtin\tbash builtins\t%s\t\n" "$name"
      done
    fi
    if builtin alias -p >/dev/null 2>&1; then
      alias -p | while IFS= read -r line; do
        name=${line#alias }
        name=${name%%=*}
        printf "alias\tbash aliases\t%s\t%s\n" "$name" "$line"
      done
    fi
    declare -F | while read -r _ name _; do
      printf "function\tbash functions\t%s\t\n" "$name"
    done
    if builtin compgen -c >/dev/null 2>&1; then
      compgen -c | sort -u | while read -r name; do
        path=$(command -v "$name" 2>/dev/null || true)
        printf "external\tPATH\t%s\t%s\n" "$name" "$path"
      done
    fi
  ' >"$tmp_file"
else
  env | awk -F= '/^PATH=/{print $2}' | tr ':' '\n' | while read -r dir; do
    [ -d "$dir" ] || continue
    find "$dir" -maxdepth 1 -type f -perm -u+x -print 2>/dev/null | while read -r path; do
      name="$(basename "$path")"
      printf "external\tPATH\t%s\t%s\n" "$name" "$path"
    done
  done | sort -u >"$tmp_file"
fi

python3 - "$tmp_file" "$output_path" "$shell_name" <<'PY'
import json
import sys
from pathlib import Path

input_path = Path(sys.argv[1])
output_path = Path(sys.argv[2])
shell_name = sys.argv[3] or "sh"
records = []
seen = set()

for raw_line in input_path.read_text(encoding="utf-8").splitlines():
    if not raw_line.strip():
        continue
    parts = raw_line.split("\t", 3)
    if len(parts) < 4:
        parts += [""] * (4 - len(parts))
    kind, detected_from, name, notes = parts
    key = (name, kind, detected_from, notes)
    if key in seen:
        continue
    seen.add(key)
    record = {
        "name": name,
        "source_platform": "linux",
        "shell": shell_name,
        "kind": kind,
        "detected_from": detected_from,
    }
    if notes:
        record["notes"] = notes
    records.append(record)

records.sort(key=lambda item: (item["name"].lower(), item["kind"], item["detected_from"]))
output_path.write_text(json.dumps(records, indent=2, sort_keys=True) + "\n", encoding="utf-8")
print(f"Wrote {len(records)} records to {output_path}")
PY
