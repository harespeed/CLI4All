#!/usr/bin/env python3
from __future__ import annotations

import json
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
RAW_DIRS = [
    ROOT / "data" / "raw" / "samples",
    ROOT / "data" / "raw" / "generated",
]
OUTPUT_PATH = ROOT / "data" / "commands.candidates.json"

RULES = [
    {
        "candidate_intent": "list_files",
        "matches": {
            "windows": ["dir"],
            "powershell": ["get-childitem", "ls", "dir"],
            "macos": ["ls"],
            "linux": ["ls"],
        },
        "reason": "Standard directory listing commands converge on a single read-only listing intent.",
    },
    {
        "candidate_intent": "clear_screen",
        "matches": {
            "windows": ["cls"],
            "powershell": ["clear-host", "cls", "clear"],
            "macos": ["clear"],
            "linux": ["clear"],
        },
        "reason": "Shell screen-clearing commands are stable aliases across platforms.",
    },
    {
        "candidate_intent": "show_ip_config",
        "matches": {
            "windows": ["ipconfig"],
            "powershell": ["get-netipconfiguration", "get-netipaddress", "ipconfig"],
            "macos": ["ifconfig"],
            "linux": ["ip"],
        },
        "reason": "These commands all inspect interface and address state without mutating the system.",
    },
    {
        "candidate_intent": "show_processes",
        "matches": {
            "windows": ["tasklist"],
            "powershell": ["get-process"],
            "macos": ["ps"],
            "linux": ["ps"],
        },
        "reason": "Process-listing commands are high-confidence inspection aliases.",
    },
    {
        "candidate_intent": "kill_process",
        "matches": {
            "windows": ["taskkill"],
            "powershell": ["stop-process"],
            "macos": ["kill"],
            "linux": ["kill"],
        },
        "reason": "Process termination commands share a clear intent but require reviewed safety handling.",
    },
    {
        "candidate_intent": "show_file_content",
        "matches": {
            "windows": ["type"],
            "powershell": ["get-content", "cat"],
            "macos": ["cat"],
            "linux": ["cat"],
        },
        "reason": "Text-file inspection commands are consistent read-only aliases.",
    },
    {
        "candidate_intent": "copy_file",
        "matches": {
            "windows": ["copy"],
            "powershell": ["copy-item", "cp"],
            "macos": ["cp"],
            "linux": ["cp"],
        },
        "reason": "Basic file copy commands line up well across platforms.",
    },
    {
        "candidate_intent": "move_or_rename",
        "matches": {
            "windows": ["move", "ren", "rename"],
            "powershell": ["move-item", "rename-item", "mv"],
            "macos": ["mv"],
            "linux": ["mv"],
        },
        "reason": "Move and rename commands have a well-understood shared intent but deserve confirmation review.",
    },
    {
        "candidate_intent": "remove_file",
        "matches": {
            "windows": ["del", "erase"],
            "powershell": ["remove-item", "del"],
            "macos": ["rm"],
            "linux": ["rm"],
        },
        "reason": "Deletion aliases are easy to identify, but they must remain behind CLI4ALL safety policy.",
    },
]


def load_inventory() -> list[dict]:
    records: list[dict] = []
    for directory in RAW_DIRS:
        if not directory.is_dir():
            continue
        for path in sorted(directory.glob("*.json")):
            try:
                data = json.loads(path.read_text(encoding="utf-8"))
            except json.JSONDecodeError:
                continue
            if isinstance(data, list):
                for record in data:
                    if isinstance(record, dict):
                        record = dict(record)
                        record["_source_file"] = path.name
                        records.append(record)
    return records


def platform_key(record: dict) -> str:
    platform = str(record.get("source_platform", "")).lower()
    shell = str(record.get("shell", "")).lower()
    if platform == "windows" and shell == "powershell":
        return "powershell"
    if platform == "windows":
        return "windows"
    if platform == "macos":
        return "macos"
    return "linux"


def confidence_for(rule: dict, present_platforms: set[str]) -> str:
    expected_platforms = set(rule["matches"].keys())
    if expected_platforms.issubset(present_platforms):
        return "high"
    if present_platforms:
        return "medium"
    return "low"


def build_candidates(records: list[dict]) -> dict:
    by_platform: dict[str, dict[str, list[dict]]] = {}
    for record in records:
        platform = platform_key(record)
        name = str(record.get("name", "")).strip().lower()
        if not name:
            continue
        by_platform.setdefault(platform, {}).setdefault(name, []).append(record)

    candidates = []
    for rule in RULES:
        aliases = []
        present_platforms = set()
        for platform, names in rule["matches"].items():
            for name in names:
                for record in by_platform.get(platform, {}).get(name, []):
                    present_platforms.add(platform)
                    aliases.append(
                        {
                            "platform": platform,
                            "name": record["name"],
                            "kind": record.get("kind", "unknown"),
                            "shell": record.get("shell"),
                            "detected_from": record.get("detected_from"),
                            "source_file": record.get("_source_file"),
                        }
                    )

        candidates.append(
            {
                "candidate_intent": rule["candidate_intent"],
                "possible_aliases": sorted(
                    aliases,
                    key=lambda item: (
                        item["platform"],
                        str(item["name"]).lower(),
                        str(item.get("kind", "")),
                    ),
                ),
                "platforms": sorted(present_platforms),
                "confidence": confidence_for(rule, present_platforms),
                "reason": rule["reason"],
                "requires_human_review": True,
            }
        )

    return {"candidates": candidates}


def main() -> None:
    records = load_inventory()
    payload = build_candidates(records)
    OUTPUT_PATH.write_text(
        json.dumps(payload, indent=2, sort_keys=True) + "\n",
        encoding="utf-8",
    )
    print(f"Wrote {len(payload['candidates'])} candidates to {OUTPUT_PATH}")


if __name__ == "__main__":
    main()
