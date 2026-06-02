# INTEGRATION: Resource Guardian

This document describes how `typst-resource-guardian` integrates with the Typst
compiler and CLI.

## Overview

The Resource Guardian is a **SuperInstance enhancement** that monitors and
enforces resource budgets during Typst compilation. It provides:

- **Compilation budgets**: max CPU time, memory, and page count
- **Phased escalation**: 70% → warning, 85% → degraded, 100% → hard stop
- **Per-chapter tracking**: identify which chapters are expensive
- **Incremental compilation detection**: flag full recompilation triggers

## Architecture

```
┌─────────────────────────────────────────────────────┐
│                    typst-cli                         │
│                                                      │
│  ┌──────────────┐    ┌──────────────────────────┐   │
│  │  CompileArgs  │    │     CompileConfig         │   │
│  │  --budget  ───┼───▶│  resource_guardian: Arc   │   │
│  └──────────────┘    │  of ResourceGuardian       │   │
│                      └──────────┬─────────────────┘   │
│                                 │                      │
│                      ┌──────────▼─────────────────┐   │
│                      │     compile_once()          │   │
│                      │  - prints guardian status   │   │
│                      │  - checks should_stop()     │   │
│                      └────────────────────────────┘   │
└─────────────────────────────────────────────────────┘
                          │
┌─────────────────────────▼──────────────────────────┐
│              typst-resource-guardian                │
│                                                      │
│  ┌──────────┐  ┌─────────────┐  ┌────────────────┐ │
│  │  Budget   │  │  Phase      │  │ ResourceGuardian│ │
│  │  parsing  │  │  detection  │  │  - chapters     │ │
│  └──────────┘  └─────────────┘  │  - pages/memory  │ │
│                                 │  - status/dump   │ │
│                                 └────────────────┘ │
│  ┌────────────────────────────────────────────────┐ │
│  │   ConservationCounter (one-sided tracking)     │ │
│  └────────────────────────────────────────────────┘ │
└─────────────────────────────────────────────────────┘
```

## CLI Integration

### New flag: `--budget`

The `--budget` flag is added to `typst compile` and `typst watch`:

```
typst compile --budget=time:30s,memory:500MB,pages:1000 doc.typ
```

**Budget format:**
- `time`: duration with `s` or `ms` suffix (default: `30s`)
- `memory`: value with `MB` or `GB` suffix (default: `500MB`)
- `pages`: integer (default: `1000`)

All three keys are optional — only the specified ones override defaults.

### Status output

When `--budget` is active, the guardian prints status after compilation:

```
[🌿 Resource Guardian] Resource Guardian: 2.3s/30.0s compile, 45MB/500MB memory,
0 pages/1000 pages, fonts: 42 | time: Normal, memory: Normal, pages: Normal
```

If a hard stop is triggered:

```
[🌿 Resource Guardian] ⛔ Hard stop reached — partial output may be incomplete.
```

### Chapter tracking

Chapters are tracked via the API (`begin_chapter` / `end_chapter`), allowing
per-section resource attribution. When chapters are recorded, the snapshot
includes a `chapters` field with per-chapter CPU time and page counts.

### Incremental compilation detection

Full recompilation is detected and flagged via `mark_full_recompile()`.
The snapshot includes `is_full_recompilation: true/false` and the status
string appends `[FULL RECOMPILE]` when detected.

## Hooks for Deeper Integration

The `ResourceGuardian` can be integrated deeper into the compilation pipeline:

- **In `typst-layout`**: Call `guardian.record_page()` after each page layout
- **In `typst-eval`**: Call `guardian.begin_chapter()`/`guardian.end_chapter()`
  around section evaluation
- **In `typst-utils` font cache**: Call `guardian.update_font_cache(entries)`
  when font cache changes
- **At compile start**: Call `guardian.mark_full_recompile()` when
  detecting a full vs incremental compilation

These hooks are left as future integration points. The current integration
at the CLI level provides resource status display and budget enforcement.

## Packages

- `typst-resource-guardian` — the library crate (in `crates/typst-resource-guardian/`)
- `typst-cli` — the CLI that consumes it

## Dependencies

The guardian crate depends only on:
- `serde` / `serde_json` (for serialization)
- `chrono` (for timestamps)
- `ecow` (for eco strings)

No new external dependencies are introduced to the workspace build.

## Testing

Run unit and integration tests:

```bash
cargo test -p typst-resource-guardian
```

This runs:
- 10 unit tests (phase detection, budget parsing, guardian lifecycle,
  conservation counter, chapter tracking, full-recompile flag)
- 4 integration tests (full lifecycle, status formatting, conservation
  counter, complex budget parsing)
- 1 doctest (budget parsing example)
