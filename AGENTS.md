# AGENTS.md

Guidance for AI agents working with the aget codebase.

## Project Overview

**aget** is a curl-like CLI that fetches URLs and outputs Markdown. It uses a chain
of "engines" (Accept: text/markdown, .md append, HTML extraction) with per-domain
config rules for URL transforms, engine overrides, and custom headers.

## Repository Structure

```
aget/
├── cli/src/
│   ├── main.rs         # Entry point, run(), wiring
│   └── cli.rs          # Clap arg definitions
└── lib/src/
    ├── config.rs       # Config, DomainRule, apply_url_transform
    ├── error.rs        # AgetError, Result alias
    ├── fetcher.rs      # Fetcher, FetchResponse (reqwest wrapper)
    ├── quality.rs      # passes_quality heuristic
    ├── pipeline.rs     # Pipeline orchestrator
    └── engine/
        ├── mod.rs          # Engine trait, EngineResult
        ├── accept_md.rs    # Engine 1
        ├── dot_md.rs       # Engine 2
        ├── html_extract.rs # Engine 3 (dom_smoothie + htmd)
        └── registry.rs     # build_chain, engine_by_name
```

## Development

```bash
make build    # debug build
make test     # run all tests
make fmt      # format + fix lints
make check    # fmt check + clippy + tests + build
make install  # install to ~/.cargo/bin
```

## Code Style

- `thiserror` for library errors, `anyhow` in CLI
- Tests live alongside source files
- All warnings are errors in clippy check

## Adding a New Engine

1. Create `lib/src/engine/<name>.rs`, implement `Engine` trait
2. Add variant to `engine_by_name()` in `registry.rs`
3. Add to `DEFAULT_CHAIN` in `registry.rs` if it should be default
4. Document the name in `aget.toml.example`

## Environment Variables

| Variable | Purpose |
|----------|---------|
| `NO_COLOR` | Disable colored output |
| `RUST_LOG` | Log verbosity |
