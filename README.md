<p align="center">
  <img alt="Kasetto logo" src="assets/logo.svg" width="450" />
</p>

<p align="center">
  <img alt="License" src="https://img.shields.io/github/license/pivoshenko/kasetto?style=flat-square&logo=opensourceinitiative&logoColor=white&color=0A6847">
  <img alt="Rust" src="https://img.shields.io/badge/Rust-Stable-0A6847?style=flat-square&logo=rust&logoColor=white">
  <img alt="Release" src="https://img.shields.io/github/v/release/pivoshenko/kasetto?style=flat-square&logo=github&logoColor=white&color=4856CD&label=Release">
</p>

- [Overview](#overview)
- [Features](#features)
- [Install](#install)
- [Quick Start](#quick-start)
- [Commands](#commands)
  - [`sync`](#sync)
  - [`list`](#list)
  - [`doctor`](#doctor)
- [Configuration](#configuration)
- [Supported Agents](#supported-agents)
- [Common Workflows](#common-workflows)
- [Storage Model](#storage-model)
- [Why Kasetto](#why-kasetto)
  - [Compared to Vercel Skills](#compared-to-vercel-skills)
  - [Compared to Claude Plugins](#compared-to-claude-plugins)
- [License](#license)

## Overview

`kasetto` is a CLI for managing skill packs for AI coding agents.

It pulls skills from repositories or local folders, installs them into the right agent directory, tracks what is installed in a local manifest database, and gives you a `doctor` command when something looks off.

The name comes from the Japanese word **カセット** (*kasetto*), meaning **cassette**. That is the mental model: each skill source is a cassette you can plug in, swap out, and keep organized.

## Features

- Sync skills from GitHub or local directories
- Use either a local YAML file or a remote HTTPS config
- Target many agent CLIs through built-in destination presets
- Browse installed skills in an interactive `list` UI
- Track installs and sync reports in `~/.kst/manifest.db`
- Run `doctor` to inspect paths, version, and recent sync failures
- Use `--json` output for scripting and automation
- Run the same binary as `kasetto` or `kst`

## Install

TBA

## Quick Start

Create a config file:

```yaml
agent: codex

skills:
  - source: https://github.com/pivoshenko/pivoshenko.ai
    skills:
      - name: pivoshenko-brand-guidelines
      - name: skill-creator
```

Sync the configured skills:

```bash
kasetto sync --config skills.config.yaml
```

You can also point `--config` at an HTTPS URL:

```bash
kasetto sync --config https://example.com/skills.config.yaml
```

Then inspect what is installed:

```bash
kasetto list
kasetto doctor
```

## Commands

### `sync`

Reads the config, discovers the requested skills, and makes the destination match it.

```bash
kasetto sync [--config <path-or-url>] [--dry-run] [--quiet] [--json] [--plain] [--verbose]
```

Notes:
- `--config` accepts a local file path or an HTTP(S) URL
- `--dry-run` shows planned changes without writing files
- missing skills are reported as broken, but do not stop the whole run
- the exit code is non-zero only for source-level failures

### `list`

Shows skills currently tracked in the manifest database.

```bash
kasetto list [--json]
```

Notes:
- in a TTY, `kasetto list` opens the interactive browser UI
- outside a TTY, output falls back to a plain list
- `NO_TUI=1` forces non-interactive output

### `doctor`

Prints local diagnostics for the current Kasetto setup.

```bash
kasetto doctor [--json]
```

Includes:
- version
- manifest DB path
- installation path
- last sync timestamp
- failed skills from the latest sync report

## Configuration

Configuration can come from a local file path or an HTTPS URL passed to `--config`.

Top-level keys:
- `agent` (optional): one of the supported agent presets above
- `destination` (optional): explicit install path, which overrides `agent`
- `skills` (required): list of skill sources to sync

Each source entry supports:
- `source` (required): local path or GitHub URL
- `branch` (optional): branch for remote source, default `main` with fallback to `master`
- `skills` (required):
  - `"*"` to sync every discovered skill
  - a list of names such as `- my-skill`
  - a list of objects such as `- name: my-skill`, with optional `path`

Example:

```yaml
agent: codex

skills:
  - source: https://github.com/openai/skills
    branch: main
    skills:
      - code-reviewer
      - name: design-system

  - source: ~/Development/my-skills
    skills: "*"

  - source: https://github.com/acme/skill-pack
    skills:
      - name: custom-skill
        path: tools/skills
```

## Supported Agents

When you set `agent`, Kasetto resolves the destination automatically.

| Agent          | `agent:` value   | Path                            |
| -------------- | ---------------- | ------------------------------- |
| Amp            | `amp`            | `~/.config/agents/skills/`      |
| Kimi Code CLI  | `kimi-cli`       | `~/.config/agents/skills/`      |
| Replit         | `replit`         | `~/.config/agents/skills/`      |
| Universal      | `universal`      | `~/.config/agents/skills/`      |
| Antigravity    | `antigravity`    | `~/.gemini/antigravity/skills/` |
| Augment        | `augment`        | `~/.augment/skills/`            |
| Claude Code    | `claude-code`    | `~/.claude/skills/`             |
| OpenClaw       | `openclaw`       | `~/.openclaw/skills/`           |
| Cline          | `cline`          | `~/.agents/skills/`             |
| Warp           | `warp`           | `~/.agents/skills/`             |
| CodeBuddy      | `codebuddy`      | `~/.codebuddy/skills/`          |
| Codex          | `codex`          | `~/.codex/skills/`              |
| Command Code   | `command-code`   | `~/.commandcode/skills/`        |
| Continue       | `continue`       | `~/.continue/skills/`           |
| Cortex Code    | `cortex`         | `~/.snowflake/cortex/skills/`   |
| Crush          | `crush`          | `~/.config/crush/skills/`       |
| Cursor         | `cursor`         | `~/.cursor/skills/`             |
| Deep Agents    | `deepagents`     | `~/.deepagents/agent/skills/`   |
| Droid          | `droid`          | `~/.factory/skills/`            |
| Gemini CLI     | `gemini-cli`     | `~/.gemini/skills/`             |
| GitHub Copilot | `github-copilot` | `~/.copilot/skills/`            |
| Goose          | `goose`          | `~/.config/goose/skills/`       |
| Junie          | `junie`          | `~/.junie/skills/`              |
| iFlow CLI      | `iflow-cli`      | `~/.iflow/skills/`              |
| Kilo Code      | `kilo`           | `~/.kilocode/skills/`           |
| Kiro CLI       | `kiro-cli`       | `~/.kiro/skills/`               |
| Kode           | `kode`           | `~/.kode/skills/`               |
| MCPJam         | `mcpjam`         | `~/.mcpjam/skills/`             |
| Mistral Vibe   | `mistral-vibe`   | `~/.vibe/skills/`               |
| Mux            | `mux`            | `~/.mux/skills/`                |
| OpenCode       | `opencode`       | `~/.config/opencode/skills/`    |
| OpenHands      | `openhands`      | `~/.openhands/skills/`          |
| Pi             | `pi`             | `~/.pi/agent/skills/`           |
| Qoder          | `qoder`          | `~/.qoder/skills/`              |
| Qwen Code      | `qwen-code`      | `~/.qwen/skills/`               |
| Roo Code       | `roo`            | `~/.roo/skills/`                |
| Trae           | `trae`           | `~/.trae/skills/`               |
| Trae CN        | `trae-cn`        | `~/.trae-cn/skills/`            |
| Windsurf       | `windsurf`       | `~/.codeium/windsurf/skills/`   |
| Zencoder       | `zencoder`       | `~/.zencoder/skills/`           |
| Neovate        | `neovate`        | `~/.neovate/skills/`            |
| Pochi          | `pochi`          | `~/.pochi/skills/`              |
| AdaL           | `adal`           | `~/.adal/skills/`               |

`claude` is still accepted as a legacy alias for `claude-code`.

If you set `destination`, it overrides `agent`.

## Common Workflows

Team bootstrap from a shared config:

```bash
kasetto sync --config https://example.com/skills.config.yaml
```

Local experimentation from a private skill folder:

```yaml
skills:
  - source: ~/Development/my-skills
    skills: "*"
```

Curated bundle from multiple sources:

```yaml
agent: codex
skills:
  - source: https://github.com/pivoshenko/pivoshenko.ai
    skills:
      - pivoshenko-brand-guidelines
  - source: https://github.com/vercel-labs/skills
    skills:
      - frontend-design
```

## Storage Model

Kasetto stores state in SQLite at `~/.kst/manifest.db`.

Tables:
- `skills`: installed skill state, including hash, source, destination, and update time
- `meta`: general metadata such as `last_run`
- `reports`: JSON sync reports for each run

This makes it possible to:
- detect changes via hashes
- persist state incrementally
- inspect the latest sync results with `doctor`

## Why Kasetto

There are already good options in this space, including [Vercel Skills](https://github.com/vercel-labs/skills) and [Claude Plugins](https://claude.com/plugins).

Kasetto is aimed at a different job: **managing reproducible, repo-driven skill bundles across machines and agent environments**.

### Compared to Vercel Skills

Vercel Skills gives you a curated catalog and a smooth install path.

Kasetto is a better fit when you need:
- more than one source in a single config
- a versioned YAML file that can bootstrap a whole team
- manifest-backed install tracking in `~/.kst/manifest.db`
- one config field that targets many agent environments

### Compared to Claude Plugins

Claude Plugins are built for runtime integrations inside Claude.

Kasetto is a better fit when you need:
- skill distribution from Git or local sources
- a repository-first workflow instead of a marketplace workflow
- predictable sync, update, and remove behavior
- a CLI that can be scripted in setup flows or CI

## License

Licensed under either [MIT](LICENSE-MIT) or [Apache-2.0](LICENSE-APACHE), at your option.
