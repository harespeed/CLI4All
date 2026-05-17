# CLI4ALL SPEC

## Product Goal

CLI4ALL is a deterministic cross-platform command helper for Ubuntu users.

It helps users who remember commands from Windows CMD, PowerShell, macOS, or other Linux distributions find the correct Ubuntu equivalent.

The first version must not use LLMs or online APIs.

## Target Platform

Ubuntu Linux first.

## Core Commands

### check

Example:

cli4all check "ipconfig"

Expected behavior:
- Detect that `ipconfig` is a Windows CMD command.
- Explain that the current target is Ubuntu.
- Suggest Ubuntu alternatives:
  - ip addr
  - hostname -I

### translate

Example:

cli4all translate "dir" --to ubuntu

Expected behavior:
- Detect that `dir` is a Windows CMD command.
- Suggest Ubuntu equivalents:
  - ls
  - ls -la

### explain

Example:

cli4all explain "chmod -R 777 ."

Expected behavior:
- Explain chmod.
- Explain -R.
- Explain 777.
- Mark risk as high.

### risk

Example:

cli4all risk "rm -rf /"

Expected behavior:
- Risk level: destructive.
- Reason: recursively removes root filesystem.
- Never execute the command.

### fix

Example:

cli4all fix "command not found: ipconfig"

Expected behavior:
- Explain that ipconfig is not a default Ubuntu command.
- Suggest:
  - ip addr
  - hostname -I

## Non-goals for v0.1

- No GUI.
- No integrated terminal.
- No LLM agent.
- No automatic command execution.
- No shell replacement.
- No Windows/macOS package yet.

## Tech Stack

- Rust 2021 edition.
- clap for CLI argument parsing.
- serde and serde_yaml for YAML command rules.
- regex for command detection.
- anyhow for error handling.
- cargo test for testing.

## Design Rules

- Rules first.
- Deterministic output.
- Never execute user commands.
- Dangerous commands must be flagged.
- All command mappings should be stored in YAML.
