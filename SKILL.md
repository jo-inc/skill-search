---
name: skill-search
description: Search and discover agent skills across clawdhub, anthropic, openai, and openai-experimental registries. Use when you need to find skills by keyword, discover new capabilities, view skill details, or get install URLs. Triggers on "find skill", "search skills", "discover skills", "install skill", or when looking for agent capabilities.
compatibility: Requires git installed for cloning skill registries
metadata:
  author: jo-inc
  version: "0.2.0"
---

# skill-search

Search and discover agent skills across multiple registries with full-text search.

## Quick Start

```bash
# Search for skills (auto-syncs on first launch)
skill-search search "calendar integration"

# Get install URL for a skill
skill-search url trello
```

## Commands

### Search for skills

```bash
# Basic search
skill-search search "browser automation"

# JSON output for programmatic use
skill-search search "pdf" --json

# Filter by registry
skill-search search "document" --registry anthropic

# Only trusted skills (anthropic + openai curated)
skill-search search "api" --trusted

# Limit results
skill-search search "calendar" --limit 5
```

### View skill details

```bash
# Show full SKILL.md content
skill-search show trello
```

### Get install URL

```bash
# Get GitHub URL for installation
skill-search url trello
# Output: https://github.com/openclaw/skills/tree/main/skills/steipete/trello
```

### Top skills by stars

```bash
# Show top 20 skills by star count
skill-search top

# Only trusted skills
skill-search top --trusted
```

### Sync registries

```bash
# Update skill index
skill-search sync

# Force full resync
skill-search sync --force
```

## Registries

| Registry | Source | Trust |
|----------|--------|-------|
| clawdhub | github.com/openclaw/skills | ⚠ Community |
| anthropic | github.com/anthropics/skills | ✓ Official |
| openai | github.com/openai/skills/.curated | ✓ Official |
| openai-experimental | github.com/openai/skills/.experimental | ⚠ Experimental |

Trust indicators in output: `[✓]` trusted, `[⚠]` untrusted

## Output Format

### Human-readable (default)

```
1. [✓] pdf ★0 (anthropic) - Comprehensive PDF manipulation toolkit...
   https://github.com/anthropics/skills/tree/main/skills/pdf

2. [⚠] browser-use ★6 (clawdhub) - Browser automation via cloud API...
   https://github.com/openclaw/skills/tree/main/skills/shawnpana/browser-use
```

### JSON (--json flag)

```json
[
  {
    "slug": "pdf",
    "name": "pdf",
    "registry": "anthropic",
    "description": "Comprehensive PDF manipulation toolkit...",
    "github_url": "https://github.com/anthropics/skills/tree/main/skills/pdf",
    "stars": 0,
    "trusted": true,
    "score": 25.3
  }
]
```

## Installation

### Pre-built Binary

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

## Data Storage

All data stored in `~/.local/share/skill-search/`:
- `skills.db` - SQLite database with skill metadata
- `index/` - Tantivy full-text search index
- `repos/` - Cloned git repositories (~100MB total)
