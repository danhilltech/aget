# aget

A curl-like CLI that fetches URLs and outputs Markdown.

## Install

### Pre-built binaries

Download the binary for your platform from the [latest release](https://github.com/danhilltech/aget/releases/latest):

```bash
# macOS Apple Silicon
curl -L https://github.com/danhilltech/aget/releases/latest/download/aget-darwin-arm64 -o aget
chmod +x aget && sudo mv aget /usr/local/bin/

# macOS Intel
curl -L https://github.com/danhilltech/aget/releases/latest/download/aget-darwin-amd64 -o aget
chmod +x aget && sudo mv aget /usr/local/bin/

# Linux x86_64
curl -L https://github.com/danhilltech/aget/releases/latest/download/aget-linux-amd64 -o aget
chmod +x aget && sudo mv aget /usr/local/bin/

# Linux arm64
curl -L https://github.com/danhilltech/aget/releases/latest/download/aget-linux-arm64 -o aget
chmod +x aget && sudo mv aget /usr/local/bin/
```

For Windows, download `aget-windows-amd64.exe` from the release page and put it on your `PATH`.

Each release publishes a `SHA256SUMS` file. Verify integrity with:

```bash
sha256sum -c SHA256SUMS
```

Each binary also has a [GitHub build provenance attestation](https://docs.github.com/en/actions/security-guides/using-artifact-attestations-to-establish-provenance-for-builds). Verify with:

```bash
gh attestation verify aget --repo danhilltech/aget
```

### From source

```bash
cargo install --locked --path cli
```

Or via make:

```bash
make install
```

## Usage

```
aget [OPTIONS] <URL>

Arguments:
  <URL>  URL to fetch and convert to Markdown

Options:
  -o, --output <FILE>   Write output to FILE instead of stdout
  -C, --config <PATH>   Config file path
  -v, --verbose         Print engine attempts and quality results to stderr
      --engine <NAME>   Force a specific engine: accept_md, dot_md, html_extract
      --chunk-size <N>  Split output into N-char chunks (requires --output)
      --completions <SHELL>  Print shell completion script (bash, zsh, fish, elvish, powershell)
```

**Examples:**

```bash
# Fetch and print as Markdown
aget https://example.com/article

# Save to file
aget -o article.md https://example.com/article

# Force HTML extraction engine
aget --engine html_extract https://example.com/article

# Save a long page as multiple files (page-001.md, page-002.md, ...)
aget -o page.md --chunk-size 8000 https://example.com/long-doc

# Install bash completions
aget --completions bash > ~/.local/share/bash-completion/completions/aget

# Or for zsh (e.g. into a directory in $fpath)
aget --completions zsh > ~/.zfunc/_aget
```

## How it works

aget tries a chain of engines in order, stopping at the first that returns quality output:

1. **accept_md** — requests `text/markdown` via `Accept` header
2. **dot_md** — appends `.md` to the URL and fetches that
3. **html_extract** — fetches HTML and extracts readable content via [dom_smoothie](https://github.com/niklaslong/dom_smoothie) + [htmd](https://github.com/letmutex/htmd)

## Configuration

### Built-in rules

aget ships with a small set of compiled-in rules so common URLs Just Work
without any config:

| Domain | Behaviour |
|---|---|
| `github.com/{owner}/{repo}` | Rewrites to `raw.githubusercontent.com/{owner}/{repo}/HEAD/README.md` and fetches directly |
| `raw.githubusercontent.com` | Fetches directly (skips engine chain) |

To override a built-in for a domain, define your own rule for that same
domain in `~/.aget/config.toml` — your rule replaces the built-in entirely
(no field-level merging).

### User config

Copy `aget.toml.example` to `~/.aget/config.toml` and customize:

```toml
# Per-domain auth headers
[domains."api.example.com".headers]
X-API-Key = "your-api-key-here"
```

Per-domain rules support:
- `url_transform` — rewrite the URL before fetching
- `engine` / `engines` — override the engine chain
- `headers` — add custom request headers
- `path_pattern` — a regex on the URL path; rule only applies when it matches

## Development

```bash
make build    # debug build
make test     # run all tests
make fmt      # format + fix lints
make check    # fmt check + clippy + tests + build
make release  # release build
```

### Cutting a release

Bump `version` in the workspace `Cargo.toml`, refresh the lockfile,
commit, tag, push:

```bash
cargo build --locked
git commit -am "release: vX.Y.Z"
git tag vX.Y.Z
git push && git push --tags
```

The tag push triggers `.github/workflows/release.yml`, which builds
binaries for Linux (amd64/arm64), macOS (amd64/arm64), and Windows
(amd64), publishes a `SHA256SUMS` file, and attests build provenance.

## License

MIT
