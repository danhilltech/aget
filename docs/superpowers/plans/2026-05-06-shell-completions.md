# Shell Completions Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a `--completions <SHELL>` flag that prints a shell completion script for `aget` to stdout (bash, zsh, fish, elvish, powershell), so users can `eval` or write to their shell completion dir.

**Architecture:** Use the `clap_complete` crate, which generates completion scripts from an existing `clap` command tree. `--completions` is a top-level flag that conflicts with the normal `<URL>` argument: when present, generate the script and exit; otherwise behave normally. Make `url` optional in clap and require it via `required_unless_present = "completions"` so help/error output stays clean.

**Tech Stack:** `clap_complete` 4.x (matches existing `clap` major version), Rust.

---

## File Map

| File | Change |
|---|---|
| `cli/Cargo.toml` | Add `clap_complete = "4"` |
| `cli/src/cli.rs` | Make `url` optional, add `--completions <SHELL>` flag |
| `cli/src/main.rs` | If `cli.completions.is_some()` → print script and exit before any other work |
| `cli/tests/integration.rs` | Add three integration tests |
| `README.md` | Document the new flag under "Usage" |

---

### Task 1: Add the dependency

**Files:**
- Modify: `cli/Cargo.toml`

- [ ] **Step 1: Add `clap_complete` to `cli/Cargo.toml`**

In `cli/Cargo.toml`, in the `[dependencies]` block, add after the `clap.workspace = true` line:

```toml
clap_complete = "4"
```

The block should now read:

```toml
[dependencies]
aget-lib = { path = "../lib" }
clap.workspace = true
clap_complete = "4"
anyhow.workspace = true
tokio.workspace = true
url.workspace = true
```

- [ ] **Step 2: Verify build**

Run: `cargo build -p aget`
Expected: PASS (compiles, no errors)

- [ ] **Step 3: Commit**

```bash
git add cli/Cargo.toml Cargo.lock
git commit -m "chore: add clap_complete dependency for shell completions"
```

---

### Task 2: Add `--completions` flag and make `url` conditionally optional

**Files:**
- Modify: `cli/src/cli.rs`

- [ ] **Step 1: Replace `cli/src/cli.rs` with the new definition**

Overwrite `cli/src/cli.rs` with:

```rust
use clap::Parser;
use clap_complete::Shell;
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(name = "aget")]
#[command(about = "Fetch a URL and output its content as Markdown")]
#[command(version)]
pub struct Cli {
    /// URL to fetch and convert to Markdown
    #[arg(required_unless_present = "completions")]
    pub url: Option<String>,

    /// Write output to FILE instead of stdout
    #[arg(short = 'o', long = "output", value_name = "FILE")]
    pub output: Option<PathBuf>,

    /// Config file path
    #[arg(short = 'C', long = "config", value_name = "PATH")]
    pub config: Option<PathBuf>,

    /// Print engine attempts and quality results to stderr
    #[arg(short = 'v', long = "verbose")]
    pub verbose: bool,

    /// Force a specific engine (overrides domain rules): accept_md, dot_md, html_extract
    #[arg(long = "engine", value_name = "NAME")]
    pub engine: Option<String>,

    /// Disable HTTP response caching
    #[arg(long = "no-cache")]
    pub no_cache: bool,

    /// Print a content summary (size, tokens, title) instead of outputting Markdown
    #[arg(long = "head", conflicts_with = "output")]
    pub head: bool,

    /// Output --head result as JSON
    #[arg(long = "json", requires = "head")]
    pub json: bool,

    /// Print a shell completion script and exit
    #[arg(
        long = "completions",
        value_name = "SHELL",
        value_enum,
        conflicts_with_all = ["output", "config", "verbose", "engine", "no_cache", "head", "json"]
    )]
    pub completions: Option<Shell>,
}
```

- [ ] **Step 2: Verify build**

Run: `cargo build -p aget`
Expected: PASS

- [ ] **Step 3: Commit**

```bash
git add cli/src/cli.rs
git commit -m "feat(cli): add --completions flag definition"
```

---

### Task 3: Wire up completion generation in `main.rs`

**Files:**
- Modify: `cli/src/main.rs`

- [ ] **Step 1: Update `main.rs` to handle `--completions` and unwrap `url`**

The existing `cli.url` is now `Option<String>`. Two changes:
1. Early-return when `cli.completions.is_some()`.
2. Unwrap `cli.url` (clap guarantees it's present in the normal path because of `required_unless_present`).

Replace the body of `run()` in `cli/src/main.rs` with:

```rust
async fn run() -> Result<()> {
    let cli = Cli::parse();

    if let Some(shell) = cli.completions {
        let mut cmd = <Cli as clap::CommandFactory>::command();
        let bin_name = cmd.get_name().to_string();
        clap_complete::generate(shell, &mut cmd, bin_name, &mut std::io::stdout());
        return Ok(());
    }

    let url_str = cli.url.as_deref().expect("clap guarantees url is set when --completions is absent");
    let url = Url::parse(url_str).context("invalid URL")?;

    let config = match &cli.config {
        Some(path) => Config::load(path).context("failed to load config")?,
        None => Config::load_default().context("failed to load default config")?,
    };

    let domain = url.host_str().unwrap_or("").to_string();
    let mut rule: Option<DomainRule> = config.domains.get(&domain).cloned();

    if let Some(ref engine_name) = cli.engine {
        if engine_by_name(engine_name).is_none() {
            anyhow::bail!(
                "unknown engine '{}'. Valid: accept_md, dot_md, html_extract",
                engine_name
            );
        }
        rule = Some(DomainRule {
            engines: Some(vec![engine_name.clone()]),
            headers: rule.as_ref().map(|r| r.headers.clone()).unwrap_or_default(),
            ..Default::default()
        });
    }

    let pipeline = Pipeline::new(cli.no_cache).context("failed to create pipeline")?;

    if cli.head {
        let result = head(&url, &pipeline, rule.as_ref())
            .await
            .context("head failed")?;
        let output = if cli.json {
            result.to_json()
        } else {
            result.to_plain_text()
        };
        println!("{}", output);
        return Ok(());
    }

    let result = pipeline
        .run(&url, rule.as_ref(), cli.verbose)
        .await
        .context("fetch failed")?;

    match &cli.output {
        Some(path) => {
            std::fs::write(path, &result.content)
                .with_context(|| format!("failed to write to {}", path.display()))?;
        }
        None => {
            let stdout = std::io::stdout();
            let mut out = stdout.lock();
            out.write_all(result.content.as_bytes())
                .context("failed to write to stdout")?;
            if !result.content.ends_with('\n') {
                out.write_all(b"\n").ok();
            }
        }
    }

    Ok(())
}
```

Add this import at the top of the file (if not already there): `use clap::CommandFactory;` is referenced via the fully-qualified `<Cli as clap::CommandFactory>::command()`, so no new use line is strictly needed — but if you prefer, add `use clap::CommandFactory;` to the imports.

- [ ] **Step 2: Verify build**

Run: `cargo build -p aget`
Expected: PASS

- [ ] **Step 3: Manual smoke check**

Run: `cargo run -p aget -- --completions bash | head -5`
Expected: First lines of a bash completion script (starts with `_aget()` or `complete -F _aget aget`).

Run: `cargo run -p aget -- --completions zsh | head -5`
Expected: zsh-style completion (starts with `#compdef aget`).

Run: `cargo run -p aget --` (no args)
Expected: Exit non-zero, with clap error mentioning `<URL>` is required.

- [ ] **Step 4: Commit**

```bash
git add cli/src/main.rs
git commit -m "feat(cli): wire up --completions to generate shell scripts"
```

---

### Task 4: Add integration tests

**Files:**
- Modify: `cli/tests/integration.rs`

- [ ] **Step 1: Append three new tests**

At the bottom of `cli/tests/integration.rs`, append:

```rust
#[test]
fn test_completions_bash_outputs_script() {
    let output = aget().arg("--completions").arg("bash").output().unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.len() > 100, "expected substantial completion script, got {} bytes", stdout.len());
    assert!(stdout.contains("aget"), "completion script should mention the binary name");
}

#[test]
fn test_completions_zsh_outputs_script() {
    let output = aget().arg("--completions").arg("zsh").output().unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("#compdef"), "zsh script should contain #compdef directive");
}

#[test]
fn test_completions_unknown_shell_exits_nonzero() {
    let output = aget().arg("--completions").arg("nonsense").output().unwrap();
    assert!(!output.status.success());
}

#[test]
fn test_url_required_when_no_completions() {
    let output = aget().output().unwrap();
    assert!(!output.status.success());
}
```

- [ ] **Step 2: Run tests**

Run: `cargo test -p aget --test integration test_completions`
Expected: PASS for all three completions tests.

Run: `cargo test -p aget --test integration test_url_required_when_no_completions`
Expected: PASS.

- [ ] **Step 3: Run full test suite**

Run: `make check`
Expected: PASS — fmt, clippy (no warnings), all tests, build.

- [ ] **Step 4: Commit**

```bash
git add cli/tests/integration.rs
git commit -m "test: cover --completions flag with integration tests"
```

---

### Task 5: Document in README

**Files:**
- Modify: `README.md`

- [ ] **Step 1: Add `--completions` to the Options block in README**

In `README.md`, in the `Options:` block under the Usage section, add a line:

```
      --completions <SHELL>  Print shell completion script (bash, zsh, fish, elvish, powershell)
```

So the full block reads:

```
Options:
  -o, --output <FILE>      Write output to FILE instead of stdout
  -C, --config <PATH>      Config file path
  -v, --verbose            Print engine attempts and quality results to stderr
      --engine <NAME>      Force a specific engine: accept_md, dot_md, html_extract
      --no-cache           Disable HTTP response caching
      --head               Print content summary instead of Markdown
      --json               Output --head result as JSON
      --completions <SHELL>  Print shell completion script (bash, zsh, fish, elvish, powershell)
```

(If your README's existing options list looks slightly different, just add the new line in the same style.)

- [ ] **Step 2: Add a usage example**

Under the existing `**Examples:**` block in `README.md`, add:

````markdown
```bash
# Install bash completions
aget --completions bash > ~/.local/share/bash-completion/completions/aget

# Or for zsh (e.g. into a directory in $fpath)
aget --completions zsh > ~/.zfunc/_aget
```
````

- [ ] **Step 3: Commit**

```bash
git add README.md
git commit -m "docs: document --completions flag"
```

---

## Self-Review Checklist (run before merging)

- [ ] `--completions bash`, `zsh`, `fish` all exit 0 and produce non-empty output.
- [ ] `aget` (no args) still exits non-zero with a "URL is required" message.
- [ ] All other existing flags still work (run a quick `aget https://example.com --help`).
- [ ] `make check` passes.
