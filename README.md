# skill-search

Fast local search across agent skill registries (clawdhub, anthropic, openai) using full-text search.

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)

## Features

- **Fast local search**: Tantivy (BM25) full-text search engine
- **Git-based sync**: Clones repos locally for instant access (~10 seconds for 3400+ skills)
- **Multiple registries**: Searches clawdhub (3400+), anthropic (16), openai curated (31), and openai experimental skills
- **Trust indicators**: `[✓]` for trusted (anthropic/openai curated), `[⚠]` for untrusted (clawdhub/experimental)
- **Star ratings**: Shows popularity from clawdhub API
- **Auto-sync**: Downloads on first launch, `sync` to update

## Requirements

- **git**: Required for cloning skill registries

## Installation

### Pre-built Binaries

Download from [releases](https://github.com/jo-inc/skill-search/releases):

| Platform | Binary |
|----------|--------|
| macOS (Apple Silicon) | `skill-search-aarch64-apple-darwin.tar.gz` |
| macOS (Intel) | `skill-search-x86_64-apple-darwin.tar.gz` |
| Linux (x86_64) | `skill-search-x86_64-unknown-linux-gnu.tar.gz` |
| Linux (ARM64) | `skill-search-aarch64-unknown-linux-gnu.tar.gz` |
| Windows (x86_64) | `skill-search-x86_64-pc-windows-msvc.zip` |

### From Source

```bash
git clone https://github.com/jo-inc/skill-search
cd skill-search
cargo install --path .
```

## Usage

```bash
# Search for skills (auto-syncs on first launch)
skill-search search "trello integration"

# Search with JSON output (for programmatic use)
skill-search search "browser automation" --json

# Filter by registry
skill-search search "pdf" --registry anthropic

# Only trusted skills (anthropic + openai official)
skill-search search "document" --trusted

# Show top skills by stars
skill-search top

# Show skill details including full SKILL.md
skill-search show trello

# Get install URL for a skill
skill-search url trello

# Force resync from GitHub
skill-search sync --force
```

## Registries

| Registry | Source | Skills | Trust |
|----------|--------|--------|-------|
| clawdhub | github.com/openclaw/skills | ~3400 | ⚠ Community |
| anthropic | github.com/anthropics/skills | ~16 | ✓ Official |
| openai | github.com/openai/skills/.curated | ~31 | ✓ Official |
| openai-experimental | github.com/openai/skills/.experimental | varies | ⚠ Experimental |
| jo | github.com/jo-inc/skills | varies | ✓ Official |

## As an Agent Skill

This tool can be installed as a skill itself. See [SKILL.md](SKILL.md) for usage instructions.

### Claude Code

```bash
/install-skill https://github.com/jo-inc/skill-search
```

### Codex

```bash
$skill-installer https://github.com/jo-inc/skill-search
```

## Data Storage

All data stored in `~/.local/share/skill-search/`:
- `skills.db` - SQLite database with skill metadata  
- `index/` - Tantivy full-text search index
- `repos/` - Cloned git repositories (~100MB total)

## Building

```bash
cargo build --release
# Binary at target/release/skill-search
```

## License

MIT
