#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
output_dir="$repo_root/data/raw/generated"
output_path="$output_dir/macos_zsh_commands.json"

mkdir -p "$output_dir"

tmp_file="$(mktemp)"
cleanup() {
  rm -f "$tmp_file"
}
trap cleanup EXIT

zsh -fc '
  for name in ${(k)builtins}; do
    print -r -- "builtin\tzsh builtins\t${name}\t"
  done
  for name in ${(k)commands}; do
    print -r -- "external\tPATH\t${name}\t${commands[$name]}"
  done
  for name in ${(k)aliases}; do
    print -r -- "alias\tzsh aliases\t${name}\t${aliases[$name]}"
  done
  for name in ${(k)functions}; do
    note=""
    if [[ -n ${functions_source[$name]-} ]]; then
      note=${functions_source[$name]}
    fi
    print -r -- "function\tzsh functions\t${name}\t${note}"
  done
' >"$tmp_file"

python3 - "$tmp_file" "$output_path" <<'PY'
import json
import sys
from pathlib import Path

input_path = Path(sys.argv[1])
output_path = Path(sys.argv[2])
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
        "source_platform": "macos",
        "shell": "zsh",
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
