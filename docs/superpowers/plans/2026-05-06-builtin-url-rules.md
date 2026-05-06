# Built-in URL Rules Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Ship a small set of compiled-in `DomainRule`s that work out-of-the-box (notably for `github.com` → raw README) so users get good behaviour with no config file. User-defined rules in `~/.aget/config.toml` always override built-ins for the same domain.

**Architecture:** A new `lib/src/builtin_rules.rs` module exposes `builtin_rules() -> HashMap<String, DomainRule>`. `Config::load_default` (and `Config::load`) merge built-ins beneath the user config: built-ins fill in any domain that the user has *not* defined. To avoid mis-firing on URLs that look superficially like a built-in target but have a different shape (e.g. `github.com/wevm/curl.md/blob/main/README.md` vs `github.com/wevm/curl.md`), `DomainRule` gains an optional `path_pattern: Option<String>` (a regex). When set and the URL path doesn't match, the rule is skipped at pipeline time (falling back to the default engine chain).

**Tech Stack:** Rust, `regex` 1.x (new dependency).

---

## File Map

| File | Change |
|---|---|
| `lib/Cargo.toml` | Add `regex = "1"` |
| `lib/src/config.rs` | Add `path_pattern: Option<String>` to `DomainRule`; merge logic in `Config::load_default`/`load`; helper `domain_rule_matches(rule, url)` |
| `lib/src/builtin_rules.rs` | **NEW** — `builtin_rules()` returning the shipped rules map |
| `lib/src/lib.rs` | Add `pub mod builtin_rules;` |
| `lib/src/pipeline.rs` | Skip a domain rule whose `path_pattern` doesn't match the request URL |
| `lib/src/main.rs` (cli) | (no change — `Config::load_default` does merging) |
| `cli/tests/integration.rs` | Test that a `github.com/{owner}/{repo}` URL gets transformed via the built-in rule |
| `README.md` | Document built-in rules and how to override them |
| `aget.toml.example` | Note that the github.com rule is now built-in (the example becomes "how to override") |

---

### Task 1: Add `regex` dependency

**Files:**
- Modify: `lib/Cargo.toml`

- [ ] **Step 1: Add the dep**

In `lib/Cargo.toml`, in `[dependencies]`, append:

```toml
regex = "1"
```

- [ ] **Step 2: Verify build**

Run: `cargo build -p aget-lib`
Expected: PASS.

- [ ] **Step 3: Commit**

```bash
git add lib/Cargo.toml Cargo.lock
git commit -m "chore: add regex dependency for url path matching"
```

---

### Task 2: Add `path_pattern` to `DomainRule` and a matcher helper

**Files:**
- Modify: `lib/src/config.rs`

- [ ] **Step 1: Write failing tests for the new field and helper**

In `lib/src/config.rs`, append to the existing `tests` mod:

```rust
#[test]
fn test_config_parses_path_pattern() {
    let toml = r#"
[domains."github.com"]
url_transform = "https://raw.githubusercontent.com/{owner}/{repo}/HEAD/README.md"
engine = "direct"
path_pattern = "^/[^/]+/[^/]+/?$"
"#;
    let mut f = NamedTempFile::new().unwrap();
    f.write_all(toml.as_bytes()).unwrap();
    let config = Config::load(f.path()).unwrap();
    let gh = config.domains.get("github.com").unwrap();
    assert_eq!(gh.path_pattern.as_deref(), Some("^/[^/]+/[^/]+/?$"));
}

#[test]
fn test_domain_rule_matches_when_no_path_pattern() {
    let rule = DomainRule::default();
    let url = url::Url::parse("https://example.com/anything/here").unwrap();
    assert!(domain_rule_matches(&rule, &url));
}

#[test]
fn test_domain_rule_matches_only_when_pattern_matches() {
    let rule = DomainRule {
        path_pattern: Some(r"^/[^/]+/[^/]+/?$".to_string()),
        ..Default::default()
    };
    let ok = url::Url::parse("https://github.com/wevm/curl.md").unwrap();
    let bad = url::Url::parse("https://github.com/wevm/curl.md/blob/main/README.md").unwrap();
    assert!(domain_rule_matches(&rule, &ok));
    assert!(!domain_rule_matches(&rule, &bad));
}

#[test]
fn test_domain_rule_invalid_regex_does_not_panic() {
    let rule = DomainRule {
        path_pattern: Some("[unclosed".to_string()),
        ..Default::default()
    };
    let url = url::Url::parse("https://example.com/").unwrap();
    // Bad regex should NOT panic and SHOULD fall back to "matches" so user mistakes don't silently break fetches
    assert!(domain_rule_matches(&rule, &url));
}
```

- [ ] **Step 2: Run tests to confirm they fail**

Run: `cargo test -p aget-lib config::tests`
Expected: FAIL — `path_pattern` field and `domain_rule_matches` function don't exist yet.

- [ ] **Step 3: Add the field and helper**

In `lib/src/config.rs`, modify the `DomainRule` struct to add the new field:

```rust
#[derive(Debug, Clone, Default, Deserialize)]
pub struct DomainRule {
    pub url_transform: Option<String>,
    pub engine: Option<String>,
    pub engines: Option<Vec<String>>,
    #[serde(default)]
    pub headers: HashMap<String, String>,
    pub path_pattern: Option<String>,
}
```

At the bottom of the file (before the `#[cfg(test)] mod tests` block), add:

```rust
pub fn domain_rule_matches(rule: &DomainRule, url: &url::Url) -> bool {
    let pattern = match &rule.path_pattern {
        Some(p) => p,
        None => return true,
    };
    match regex::Regex::new(pattern) {
        Ok(re) => re.is_match(url.path()),
        Err(_) => true, // fail open on bad regex (logged elsewhere if needed)
    }
}
```

- [ ] **Step 4: Run tests to confirm they pass**

Run: `cargo test -p aget-lib config::tests`
Expected: PASS for all four new tests plus the pre-existing ones.

- [ ] **Step 5: Commit**

```bash
git add lib/src/config.rs
git commit -m "feat(lib): add optional path_pattern to DomainRule"
```

---

### Task 3: Honour `path_pattern` in the pipeline

**Files:**
- Modify: `lib/src/pipeline.rs`

- [ ] **Step 1: Write failing test**

In `lib/src/pipeline.rs`, append to `mod tests`:

```rust
#[tokio::test]
async fn test_pipeline_skips_rule_when_path_pattern_does_not_match() {
    use crate::config::DomainRule;

    let mut server = mockito::Server::new_async().await;
    // Server returns reasonable markdown via the default engine chain
    server
        .mock("GET", "/repo/blob/main/file.md")
        .match_header(
            "Accept",
            mockito::Matcher::Regex("text/markdown".to_string()),
        )
        .with_status(200)
        .with_header("content-type", "text/markdown")
        .with_body(GOOD_MD)
        .create_async()
        .await;

    let pipeline = Pipeline::new(true).unwrap();
    let url = Url::parse(&format!("{}/repo/blob/main/file.md", server.url())).unwrap();

    // Rule that would normally rewrite the URL, but path_pattern only matches "/owner/repo"
    let rule = DomainRule {
        url_transform: Some("https://should-not-fire.example.com/wrong".to_string()),
        engine: Some("direct".to_string()),
        path_pattern: Some(r"^/[^/]+/[^/]+/?$".to_string()),
        ..Default::default()
    };

    let result = pipeline.run(&url, Some(&rule), false).await.unwrap();

    // Rule was skipped → engine chain ran instead → accept_md succeeded
    assert_eq!(result.engine_used, "accept_md");
    assert!(result.quality_passed);
}
```

- [ ] **Step 2: Run test to confirm it fails**

Run: `cargo test -p aget-lib pipeline::tests::test_pipeline_skips_rule_when_path_pattern_does_not_match`
Expected: FAIL (pipeline currently honours the rule unconditionally and would try to fetch `should-not-fire.example.com`).

- [ ] **Step 3: Update `Pipeline::run` to skip non-matching rules**

In `lib/src/pipeline.rs`, change the import line `use crate::config::{apply_url_transform, DomainRule};` to include the new helper:

```rust
use crate::config::{apply_url_transform, domain_rule_matches, DomainRule};
```

Then near the top of `Pipeline::run`, immediately after the `let url = match …` block (the URL transform), wrap the rule usage so that a non-matching `path_pattern` causes the rule to be ignored entirely. Replace the `let url = match …` and following lines, up to and including the `if rule.and_then(|r| r.engine.as_deref()) == Some("direct")` block, with:

```rust
        // If the rule's path_pattern doesn't match this URL, drop the rule entirely
        // so we use defaults for engines, headers, and URL transform.
        let effective_rule: Option<&DomainRule> = rule.filter(|r| domain_rule_matches(r, raw_url));

        // Apply URL transform if configured
        let url = match effective_rule.and_then(|r| r.url_transform.as_ref()) {
            Some(template) => apply_url_transform(raw_url, template)?,
            None => raw_url.clone(),
        };

        let domain_headers: &HashMap<String, String> =
            effective_rule.map(|r| &r.headers).unwrap_or(&EMPTY_HEADERS);

        // "direct" mode — skip engine chain, fetch transformed URL as-is
        if effective_rule.and_then(|r| r.engine.as_deref()) == Some("direct") {
            if verbose {
                eprintln!("[aget] direct fetch: {}", url);
            }
            let resp = self.fetcher.get(&url, domain_headers).await?;
            return Ok(PipelineResult {
                content: resp.body,
                engine_used: "direct".to_string(),
                quality_passed: true,
            });
        }

        // Run engine chain
        let engines = registry::build_chain(effective_rule);
```

(The remainder of the function stays as it is — just make sure all subsequent references to `rule` in the engine-chain section now reference `effective_rule` if they did before. Specifically: `registry::build_chain(rule)` becomes `registry::build_chain(effective_rule)` as shown.)

- [ ] **Step 4: Run test to confirm it passes**

Run: `cargo test -p aget-lib pipeline::tests`
Expected: PASS for the new test and all pre-existing tests.

- [ ] **Step 5: Commit**

```bash
git add lib/src/pipeline.rs
git commit -m "feat(lib): skip domain rule when path_pattern does not match"
```

---

### Task 4: Add `builtin_rules.rs`

**Files:**
- Create: `lib/src/builtin_rules.rs`
- Modify: `lib/src/lib.rs`

- [ ] **Step 1: Write the module with one shipped rule and unit tests**

Create `lib/src/builtin_rules.rs`:

```rust
use crate::config::DomainRule;
use std::collections::HashMap;

/// Returns rules shipped with aget. User-supplied rules override these by domain key.
pub fn builtin_rules() -> HashMap<String, DomainRule> {
    let mut rules = HashMap::new();

    // github.com/{owner}/{repo} — fetch the default-branch README directly
    rules.insert(
        "github.com".to_string(),
        DomainRule {
            url_transform: Some(
                "https://raw.githubusercontent.com/{owner}/{repo}/HEAD/README.md".to_string(),
            ),
            engine: Some("direct".to_string()),
            engines: None,
            headers: HashMap::new(),
            path_pattern: Some(r"^/[^/]+/[^/]+/?$".to_string()),
        },
    );

    // raw.githubusercontent.com — already plain text, skip the engine chain
    rules.insert(
        "raw.githubusercontent.com".to_string(),
        DomainRule {
            url_transform: None,
            engine: Some("direct".to_string()),
            engines: None,
            headers: HashMap::new(),
            path_pattern: None,
        },
    );

    rules
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_github_rule_present_with_expected_fields() {
        let rules = builtin_rules();
        let gh = rules.get("github.com").expect("github.com rule should exist");
        assert_eq!(gh.engine.as_deref(), Some("direct"));
        assert!(
            gh.url_transform.as_deref().unwrap().contains("raw.githubusercontent.com"),
            "transform should target raw.githubusercontent.com",
        );
        assert!(gh.path_pattern.is_some(), "github rule should be path-scoped");
    }

    #[test]
    fn test_raw_githubusercontent_present() {
        let rules = builtin_rules();
        let raw = rules
            .get("raw.githubusercontent.com")
            .expect("raw.githubusercontent.com rule should exist");
        assert_eq!(raw.engine.as_deref(), Some("direct"));
    }
}
```

- [ ] **Step 2: Register the module**

In `lib/src/lib.rs`, add `pub mod builtin_rules;` alongside the other `pub mod` declarations.

- [ ] **Step 3: Run the tests**

Run: `cargo test -p aget-lib builtin_rules::tests`
Expected: PASS.

- [ ] **Step 4: Commit**

```bash
git add lib/src/builtin_rules.rs lib/src/lib.rs
git commit -m "feat(lib): add builtin_rules with github.com and raw.githubusercontent.com"
```

---

### Task 5: Merge built-in rules into `Config::load_default` and `Config::load`

**Files:**
- Modify: `lib/src/config.rs`

- [ ] **Step 1: Write failing tests**

In `lib/src/config.rs`, append to the `tests` mod:

```rust
#[test]
fn test_load_default_includes_builtin_rules_when_no_user_config() {
    // We cannot guarantee ~/.aget/config.toml does not exist on the runner, but
    // calling Config::default().with_builtins() should always produce the merged map.
    let config = Config::default().with_builtins();
    assert!(
        config.domains.contains_key("github.com"),
        "default+builtins should include github.com"
    );
}

#[test]
fn test_user_rule_overrides_builtin_for_same_domain() {
    let toml = r#"
[domains."github.com"]
engine = "html_extract"
"#;
    let mut f = NamedTempFile::new().unwrap();
    f.write_all(toml.as_bytes()).unwrap();
    let config = Config::load(f.path()).unwrap();
    let gh = config.domains.get("github.com").unwrap();
    // User won — engine is the user-supplied one, not "direct"
    assert_eq!(gh.engine.as_deref(), Some("html_extract"));
    // User's rule does NOT inherit url_transform from the built-in
    assert!(gh.url_transform.is_none());
}

#[test]
fn test_load_fills_in_builtins_for_undefined_domains() {
    let toml = r#"
[domains."example.com"]
engine = "html_extract"
"#;
    let mut f = NamedTempFile::new().unwrap();
    f.write_all(toml.as_bytes()).unwrap();
    let config = Config::load(f.path()).unwrap();
    // User domain present
    assert!(config.domains.contains_key("example.com"));
    // Built-in github.com still present (user didn't override it)
    let gh = config.domains.get("github.com").expect("github.com built-in should remain");
    assert_eq!(gh.engine.as_deref(), Some("direct"));
}
```

- [ ] **Step 2: Run tests to confirm they fail**

Run: `cargo test -p aget-lib config::tests`
Expected: FAIL on the three new tests (`with_builtins` doesn't exist; `Config::load` doesn't merge yet).

- [ ] **Step 3: Add merge logic**

In `lib/src/config.rs`, in the `impl Config` block, modify `load` and `load_default` and add a new `with_builtins` method:

```rust
impl Config {
    pub fn load(path: &Path) -> Result<Self> {
        let content = std::fs::read_to_string(path).map_err(AgetError::Io)?;
        let parsed: Config = toml::from_str(&content).map_err(AgetError::TomlParse)?;
        Ok(parsed.with_builtins())
    }

    pub fn load_default() -> Result<Self> {
        let user = match Self::default_path() {
            Some(path) if path.exists() => {
                let content = std::fs::read_to_string(&path).map_err(AgetError::Io)?;
                toml::from_str(&content).map_err(AgetError::TomlParse)?
            }
            _ => Self::default(),
        };
        Ok(user.with_builtins())
    }

    pub fn default_path() -> Option<PathBuf> {
        dirs::home_dir().map(|h| h.join(".aget").join("config.toml"))
    }

    /// Merge built-in rules under the user-supplied rules (user wins by domain key).
    pub fn with_builtins(mut self) -> Self {
        for (domain, rule) in crate::builtin_rules::builtin_rules() {
            self.domains.entry(domain).or_insert(rule);
        }
        self
    }
}
```

- [ ] **Step 4: Run tests to confirm they pass**

Run: `cargo test -p aget-lib config::tests`
Expected: PASS for all tests.

- [ ] **Step 5: Commit**

```bash
git add lib/src/config.rs
git commit -m "feat(lib): merge builtin rules into Config::load and load_default"
```

---

### Task 6: Integration test — github.com URL is transformed end-to-end

**Files:**
- Modify: `cli/tests/integration.rs`

> **Note for the implementer:** The existing integration tests use `mockito` so they hit a local mock server. We can't easily intercept `raw.githubusercontent.com` from a sub-process binary (no DNS shim). Instead, this integration test exercises the *config-merge* path: it ensures that `Config::load_default()` (which the binary calls when no `--config` is passed) returns a built-in rule for `github.com`. This is a unit-level test of behaviour, not a network test.

- [ ] **Step 1: Add a unit test in `lib/src/config.rs` that verifies end-to-end transform shape**

Append to `config::tests`:

```rust
#[test]
fn test_builtin_github_transform_renders_expected_url() {
    use super::apply_url_transform;
    let config = Config::default().with_builtins();
    let rule = config.domains.get("github.com").unwrap();
    let template = rule.url_transform.as_deref().unwrap();
    let url = url::Url::parse("https://github.com/wevm/curl.md").unwrap();
    let result = apply_url_transform(&url, template).unwrap();
    assert_eq!(
        result.as_str(),
        "https://raw.githubusercontent.com/wevm/curl.md/HEAD/README.md"
    );
}

#[test]
fn test_builtin_github_path_pattern_rejects_blob_urls() {
    let config = Config::default().with_builtins();
    let rule = config.domains.get("github.com").unwrap();
    let blob_url = url::Url::parse("https://github.com/wevm/curl.md/blob/main/README.md").unwrap();
    assert!(!domain_rule_matches(rule, &blob_url));
}
```

- [ ] **Step 2: Run tests**

Run: `cargo test -p aget-lib config::tests::test_builtin_github`
Expected: PASS.

- [ ] **Step 3: Run full check**

Run: `make check`
Expected: PASS.

- [ ] **Step 4: Commit**

```bash
git add lib/src/config.rs
git commit -m "test: verify builtin github rule renders expected raw url and rejects blob paths"
```

---

### Task 7: Update docs

**Files:**
- Modify: `README.md`
- Modify: `aget.toml.example`

- [ ] **Step 1: Add a "Built-in rules" subsection to README**

In `README.md`, under the existing `## Configuration` section, **before** the `Per-domain rules support:` line, insert:

```markdown
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
```

Then update the `Per-domain rules support:` list to include the new field:

```markdown
Per-domain rules support:
- `url_transform` — rewrite the URL before fetching
- `engine` / `engines` — override the engine chain
- `headers` — add custom request headers
- `path_pattern` — a regex on the URL path; rule only applies when it matches
```

- [ ] **Step 2: Update `aget.toml.example`**

Replace the contents of `aget.toml.example` with:

```toml
# Example aget configuration file.
# Copy to ~/.aget/config.toml and customize.
#
# Note: rules for github.com and raw.githubusercontent.com are built into aget.
# Defining your own rule for those domains here will REPLACE the built-in.

# Example: docs site that supports native markdown — prefer accept_md first
[domains."docs.example.com"]
engines = ["accept_md", "dot_md", "html_extract"]

[domains."docs.example.com".headers]
Authorization = "Bearer your-token-here"

# Example: API that requires a key
[domains."api.example.com".headers]
X-API-Key = "your-api-key-here"

# Example: scope a rule to a specific path shape
[domains."example.com"]
url_transform = "https://example.com/raw/{slug}.md"
engine = "direct"
path_pattern = "^/articles/[^/]+/?$"
```

- [ ] **Step 3: Commit**

```bash
git add README.md aget.toml.example
git commit -m "docs: document builtin rules and path_pattern"
```

---

## Self-Review Checklist (run before merging)

- [ ] `make check` passes (fmt, clippy with -D warnings, all tests, build).
- [ ] `cargo run -p aget -- https://github.com/wevm/curl.md` produces the curl.md README content (you can verify by piping to `head` and checking for the curl.md tagline).
- [ ] `cargo run -p aget -- https://github.com/wevm/curl.md/blob/main/README.md` does NOT trigger the built-in github rule and falls back to the default engine chain (will likely return GitHub's HTML page rendered via html_extract — that's the existing behaviour).
- [ ] User config with `[domains."github.com"]` overrides the built-in (use a temp config and verify the user-defined engine wins).
