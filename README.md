# aget

A curl-like CLI that fetches URLs and outputs Markdown.

## Install

```bash
cargo install --path cli
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
```

**Examples:**

```bash
# Fetch and print as Markdown
aget https://example.com/article

# Save to file
aget -o article.md https://example.com/article

# Force HTML extraction engine
aget --engine html_extract https://example.com/article
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
make check    # fmt + clippy + tests + build
```

## License

MIT
