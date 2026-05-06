# AGENTS.md

Guidance for AI agents working with the aget codebase.

## Project Overview

**aget** is a curl-like CLI that fetches URLs and outputs Markdown. It uses a chain
of "engines" (Accept: text/markdown, .md append, HTML extraction) with per-domain
config rules for URL transforms, engine overrides, and custom headers.

## Repository Structure

```
aget/
├── .github/
│   ├── workflows/
│   │   ├── ci.yml         # lint + test on push/PR
│   │   └── release.yml    # build + attest + publish on v* tag
│   └── dependabot.yml     # weekly cargo + github-actions PRs
├── cli/src/
│   ├── main.rs            # Entry point, run(), wiring
│   └── cli.rs             # Clap arg definitions
└── lib/src/
    ├── lib.rs             # Crate root, re-exports
    ├── config.rs          # Config, DomainRule, apply_url_transform
    ├── builtin_rules.rs   # Compiled-in domain rules
    ├── error.rs           # AgetError, Result alias
    ├── fetch.rs           # Fetch trait (HTTP GET abstraction)
    ├── fetcher.rs         # reqwest-backed Fetcher impl
    ├── caching_fetcher.rs # Fetch decorator that consults the on-disk cache
    ├── cache.rs           # On-disk SQLite response cache
    ├── head.rs            # HeadResult — preview metadata (title, size, tokens, ...)
    ├── chunk.rs           # --chunk-size output splitting
    ├── profile.rs         # Doc-framework profiles (VitePress, Docusaurus, ...)
    ├── quality.rs         # passes_quality heuristic
    ├── pipeline.rs        # Pipeline orchestrator
    └── engine/
        ├── mod.rs              # Engine trait, EngineResult
        ├── accept_md.rs        # Engine 1
        ├── dot_md.rs           # Engine 2
        ├── html_extract.rs     # Engine 3 (profile → dom_smoothie + htmd)
        └── registry.rs         # build_chain, engine_by_name
```

## Development

```bash
make build      # debug build
make run        # cargo run -q -- $(ARGS)   e.g. make run ARGS="https://example.com"
make test       # run all tests
make fmt        # format + fix lints
make check      # fmt check + clippy + tests + build
make release    # release build
make install    # install to ~/.cargo/bin (uses --locked)
make uninstall  # remove from ~/.cargo/bin
make clean      # cargo clean
```

## Releases

Releases are tag-driven. Pushing a `v*` tag triggers
`.github/workflows/release.yml`, which builds binaries for five
targets in parallel (linux amd64/arm64, darwin amd64/arm64,
windows amd64), generates a `SHA256SUMS` manifest, and produces a
GitHub build-provenance attestation per binary.

To cut a release:

1. Bump `version` in the workspace `Cargo.toml`.
2. `cargo build --locked` to refresh `Cargo.lock`.
3. `git commit -am "release: vX.Y.Z" && git tag vX.Y.Z`
4. `git push && git push --tags`

CI (`.github/workflows/ci.yml`) runs lint + test on every push/PR
to `main`/`master` (Ubuntu and macOS).

Dependabot opens weekly PRs for `cargo` and `github-actions`
ecosystems (max 10 open per ecosystem). Treat these as normal PRs —
they go through the same CI gate.

## Code Style

- `thiserror` for library errors, `anyhow` in CLI
- Tests live alongside source files
- All warnings are errors in clippy check

## Adding a New Engine

1. Create `lib/src/engine/<name>.rs`, implement `Engine` trait
2. Add variant to `engine_by_name()` in `registry.rs`
3. Add to `DEFAULT_CHAIN` in `registry.rs` if it should be default
4. Document the name in `aget.toml.example`

## Adding a New Profile

A "profile" lets `html_extract` recognise a documentation framework and
extract its content root directly, instead of relying on generic readability.

1. In `lib/src/profile.rs`, add a `pub static MY_FRAMEWORK: Profile = Profile { … }`
   with:
   - `key`: short identifier
   - `generator_pattern`: substring to match against `<meta name="generator">` (or `None`)
   - `needles`: substrings unique to the framework's HTML
   - `content_selectors`: CSS selectors (priority order) for the content root
2. Add the new static to the `PROFILES` slice.
3. Add detection and extraction tests in `profile::tests`.
4. Run `cargo test -p aget-lib profile::tests` to confirm.

Detection runs in registry order — first match wins. Put more specific
profiles before more generic ones.

## Environment Variables

| Variable | Purpose |
|----------|---------|
| `NO_COLOR` | Disable colored output |
| `RUST_LOG` | Log verbosity |
