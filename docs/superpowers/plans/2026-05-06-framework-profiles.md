# Doc-Framework Profiles Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** When fetching pages from popular documentation frameworks (VitePress, Docusaurus, Mintlify, Starlight, MkDocs), detect the framework and extract using a known content-root selector instead of relying on generic readability heuristics. This produces dramatically better markdown for the docs sites that AI agents hit most.

**Architecture:** A new `lib/src/profile.rs` module defines a `Profile` struct (`key`, `generator_pattern`, `needles`, `content_selectors`), a registry of compiled-in profiles, a `detect_profile(html)` function (cheap: regex on `<meta name="generator">` + substring scan for needles), and an `extract_with_profile(html, profile, url)` function (parses with `scraper`, finds the first matching content selector, hands its inner HTML to `htmd`). The existing `HtmlExtractEngine::fetch` is modified to try profile-based extraction first; on no match or empty result it falls back to the current `dom_smoothie` + `htmd` pipeline. No changes to the public CLI surface — this is a pure quality upgrade.

**Tech Stack:** Rust, `scraper` 0.20+ (CSS-selector-based HTML querying built on `html5ever`), `regex` 1.x (added by the built-in-rules plan; safe to add independently here too).

---

## File Map

| File | Change |
|---|---|
| `lib/Cargo.toml` | Add `scraper = "0.20"` and (if not present) `regex = "1"` |
| `lib/src/profile.rs` | **NEW** — `Profile`, `PROFILES`, `detect_profile`, `extract_with_profile` |
| `lib/src/lib.rs` | Add `pub mod profile;` |
| `lib/src/engine/html_extract.rs` | Try `extract_with_profile` first; fall back to readability on miss |
| `lib/src/engine/html_extract.rs` (tests) | Add per-framework HTML fixture tests |
| `AGENTS.md` | Add an "Adding a New Profile" section beside "Adding a New Engine" |

---

### Task 1: Add dependencies

**Files:**
- Modify: `lib/Cargo.toml`

- [ ] **Step 1: Add deps**

In `lib/Cargo.toml`, in `[dependencies]`, append:

```toml
scraper = "0.20"
regex = "1"
```

(If `regex` was already added by the builtin-rules plan, don't duplicate the line.)

- [ ] **Step 2: Verify build**

Run: `cargo build -p aget-lib`
Expected: PASS.

- [ ] **Step 3: Commit**

```bash
git add lib/Cargo.toml Cargo.lock
git commit -m "chore: add scraper and regex deps for framework profiles"
```

---

### Task 2: Scaffold `profile.rs` with the `Profile` struct and an empty registry

**Files:**
- Create: `lib/src/profile.rs`
- Modify: `lib/src/lib.rs`

- [ ] **Step 1: Create `lib/src/profile.rs`**

```rust
/// A documentation-framework profile: how to detect it and where its content lives.
pub struct Profile {
    pub key: &'static str,
    /// Substring matched against the value of <meta name="generator"> (case-insensitive).
    pub generator_pattern: Option<&'static str>,
    /// If any of these substrings appear in the HTML, treat the page as a match.
    pub needles: &'static [&'static str],
    /// CSS selectors (in priority order) used to locate the content root for extraction.
    pub content_selectors: &'static [&'static str],
}

pub static PROFILES: &[&Profile] = &[];

/// Detect which (if any) profile best matches the given HTML. First match wins.
pub fn detect_profile(html: &str) -> Option<&'static Profile> {
    let _ = html;
    None
}

/// Try to extract markdown from `html` using `profile`'s content selectors.
/// Returns `None` if no selector matches or the result is empty.
pub fn extract_with_profile(html: &str, profile: &Profile, url: &url::Url) -> Option<String> {
    let _ = (html, profile, url);
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_profile_returns_none_for_empty_html() {
        assert!(detect_profile("").is_none());
    }

    #[test]
    fn test_detect_profile_returns_none_when_no_profile_matches() {
        assert!(detect_profile("<html><body>plain</body></html>").is_none());
    }
}
```

- [ ] **Step 2: Register the module**

Add `pub mod profile;` to `lib/src/lib.rs` alongside the other `pub mod` lines.

- [ ] **Step 3: Run scaffold tests**

Run: `cargo test -p aget-lib profile::tests`
Expected: PASS.

- [ ] **Step 4: Commit**

```bash
git add lib/src/profile.rs lib/src/lib.rs
git commit -m "feat(lib): scaffold profile module"
```

---

### Task 3: TDD — implement `detect_profile` (substring needle match)

**Files:**
- Modify: `lib/src/profile.rs`

- [ ] **Step 1: Add a failing test**

In `lib/src/profile.rs`, replace `pub static PROFILES: &[&Profile] = &[];` with one real profile so the test has something to detect:

```rust
pub static VITEPRESS: Profile = Profile {
    key: "vitepress",
    generator_pattern: Some("vitepress"),
    needles: &["id=\"VPContent\"", "class=\"VPDoc", "class=\"vp-doc"],
    content_selectors: &["#VPContent", ".VPDoc", ".vp-doc"],
};

pub static PROFILES: &[&Profile] = &[&VITEPRESS];
```

In the same file, append to `mod tests`:

```rust
#[test]
fn test_detect_vitepress_via_needle() {
    let html = r#"<html><body><div id="VPContent">hi</div></body></html>"#;
    let p = detect_profile(html).expect("vitepress should match");
    assert_eq!(p.key, "vitepress");
}

#[test]
fn test_detect_vitepress_via_generator_meta() {
    let html = r#"<html><head><meta name="generator" content="VitePress 1.0.0"></head><body></body></html>"#;
    let p = detect_profile(html).expect("vitepress generator should match");
    assert_eq!(p.key, "vitepress");
}

#[test]
fn test_detect_generator_match_is_case_insensitive() {
    let html = r#"<meta name="generator" content="VITEPRESS">"#;
    assert!(detect_profile(html).is_some());
}
```

- [ ] **Step 2: Run tests to confirm they fail**

Run: `cargo test -p aget-lib profile::tests`
Expected: FAIL on the three new tests (`detect_profile` still returns None).

- [ ] **Step 3: Implement `detect_profile`**

Replace the body of `detect_profile` in `lib/src/profile.rs`:

```rust
pub fn detect_profile(html: &str) -> Option<&'static Profile> {
    let generator = extract_generator_meta(html);
    for profile in PROFILES {
        if let (Some(pattern), Some(value)) = (profile.generator_pattern, generator.as_deref()) {
            if value.to_lowercase().contains(&pattern.to_lowercase()) {
                return Some(*profile);
            }
        }
        for needle in profile.needles {
            if html.contains(needle) {
                return Some(*profile);
            }
        }
    }
    None
}

fn extract_generator_meta(html: &str) -> Option<String> {
    use std::sync::OnceLock;
    static RE: OnceLock<regex::Regex> = OnceLock::new();
    let re = RE.get_or_init(|| {
        regex::Regex::new(
            r#"(?is)<meta\s+[^>]*name\s*=\s*["']generator["'][^>]*content\s*=\s*["']([^"']+)["']"#,
        )
        .expect("static regex must compile")
    });
    re.captures(html).and_then(|c| c.get(1)).map(|m| m.as_str().to_string())
}
```

- [ ] **Step 4: Run tests to confirm they pass**

Run: `cargo test -p aget-lib profile::tests`
Expected: PASS for all five tests.

- [ ] **Step 5: Commit**

```bash
git add lib/src/profile.rs
git commit -m "feat(lib): implement detect_profile via needles and meta generator"
```

---

### Task 4: TDD — implement `extract_with_profile` using `scraper` + `htmd`

**Files:**
- Modify: `lib/src/profile.rs`

- [ ] **Step 1: Add failing tests**

Append to `mod tests` in `lib/src/profile.rs`:

```rust
#[test]
fn test_extract_with_profile_finds_content_root_and_returns_markdown() {
    let html = r#"
        <html><body>
          <nav>Should be excluded</nav>
          <div id="VPContent">
            <h1>Hello</h1>
            <p>This is a paragraph with <strong>bold</strong> text.</p>
          </div>
          <footer>Also excluded</footer>
        </body></html>
    "#;
    let url = url::Url::parse("https://example.com/page").unwrap();
    let md = extract_with_profile(html, &VITEPRESS, &url).expect("should extract");
    assert!(md.contains("Hello"), "title should be present, got: {}", md);
    assert!(md.contains("**bold**") || md.contains("__bold__"), "bold should be present");
    assert!(!md.contains("Should be excluded"), "nav should NOT be present");
    assert!(!md.contains("Also excluded"), "footer should NOT be present");
}

#[test]
fn test_extract_with_profile_returns_none_when_no_selector_matches() {
    let html = r#"<html><body><p>nothing matches the VP selectors here</p></body></html>"#;
    let url = url::Url::parse("https://example.com/").unwrap();
    assert!(extract_with_profile(html, &VITEPRESS, &url).is_none());
}

#[test]
fn test_extract_with_profile_returns_none_when_content_root_is_empty() {
    let html = r#"<html><body><div id="VPContent">   </div></body></html>"#;
    let url = url::Url::parse("https://example.com/").unwrap();
    assert!(extract_with_profile(html, &VITEPRESS, &url).is_none());
}
```

- [ ] **Step 2: Run tests to confirm they fail**

Run: `cargo test -p aget-lib profile::tests::test_extract`
Expected: FAIL (`extract_with_profile` is still a stub).

- [ ] **Step 3: Implement `extract_with_profile`**

Replace the body of `extract_with_profile` in `lib/src/profile.rs`:

```rust
pub fn extract_with_profile(html: &str, profile: &Profile, url: &url::Url) -> Option<String> {
    let _ = url; // reserved for future use (e.g. resolving relative links)
    let document = scraper::Html::parse_document(html);
    for selector_str in profile.content_selectors {
        let selector = match scraper::Selector::parse(selector_str) {
            Ok(s) => s,
            Err(_) => continue,
        };
        let element = match document.select(&selector).next() {
            Some(e) => e,
            None => continue,
        };
        let inner_html = element.inner_html();
        let markdown = htmd::convert(&inner_html).ok()?;
        let trimmed = markdown.trim();
        if !trimmed.is_empty() {
            return Some(trimmed.to_string());
        }
    }
    None
}
```

- [ ] **Step 4: Run tests to confirm they pass**

Run: `cargo test -p aget-lib profile::tests`
Expected: PASS for all eight tests.

- [ ] **Step 5: Commit**

```bash
git add lib/src/profile.rs
git commit -m "feat(lib): extract markdown from profile content selectors"
```

---

### Task 5: Add the remaining four profiles (Docusaurus, Mintlify, Starlight, MkDocs)

**Files:**
- Modify: `lib/src/profile.rs`

- [ ] **Step 1: Add failing per-framework detection tests**

Append to `mod tests`:

```rust
#[test]
fn test_detect_docusaurus_via_class() {
    let html = r#"<html><body><div class="theme-doc-markdown markdown"><p>x</p></div></body></html>"#;
    let p = detect_profile(html).expect("docusaurus should match");
    assert_eq!(p.key, "docusaurus");
}

#[test]
fn test_detect_mintlify_via_id() {
    let html = r#"<html><body><div id="content-area">x</div></body></html>"#;
    let p = detect_profile(html).expect("mintlify should match");
    assert_eq!(p.key, "mintlify");
}

#[test]
fn test_detect_starlight_via_generator() {
    let html = r#"<meta name="generator" content="Starlight 0.30.0">"#;
    let p = detect_profile(html).expect("starlight should match");
    assert_eq!(p.key, "starlight");
}

#[test]
fn test_detect_mkdocs_via_data_attr() {
    let html = r#"<html><body><div data-md-component="content"><p>x</p></div></body></html>"#;
    let p = detect_profile(html).expect("mkdocs should match");
    assert_eq!(p.key, "mkdocs");
}

#[test]
fn test_extract_docusaurus_content() {
    let html = r#"
        <html><body>
          <header>nav</header>
          <article class="theme-doc-markdown markdown">
            <h1>Doc Title</h1>
            <p>Body text.</p>
          </article>
        </body></html>
    "#;
    let url = url::Url::parse("https://example.com/").unwrap();
    let p = detect_profile(html).unwrap();
    assert_eq!(p.key, "docusaurus");
    let md = extract_with_profile(html, p, &url).unwrap();
    assert!(md.contains("Doc Title"));
    assert!(!md.contains("nav"));
}
```

- [ ] **Step 2: Run tests to confirm they fail**

Run: `cargo test -p aget-lib profile::tests`
Expected: FAIL for the five new tests (only VitePress is registered today).

- [ ] **Step 3: Add four more profiles to `lib/src/profile.rs`**

Below the existing `VITEPRESS` definition, add:

```rust
pub static DOCUSAURUS: Profile = Profile {
    key: "docusaurus",
    generator_pattern: Some("docusaurus"),
    needles: &["class=\"theme-doc-markdown", "class=\"markdown markdown"],
    content_selectors: &[".theme-doc-markdown", ".markdown"],
};

pub static MINTLIFY: Profile = Profile {
    key: "mintlify",
    generator_pattern: Some("mintlify"),
    needles: &["id=\"content-area\"", "id=\"content-container\""],
    content_selectors: &["#content-container", "#content-area"],
};

pub static STARLIGHT: Profile = Profile {
    key: "starlight",
    generator_pattern: Some("starlight"),
    needles: &["class=\"sl-markdown-content\"", "id=\"starlight__sidebar\""],
    content_selectors: &[".sl-markdown-content"],
};

pub static MKDOCS: Profile = Profile {
    key: "mkdocs",
    generator_pattern: Some("mkdocs"),
    needles: &[
        "data-md-component=\"content\"",
        "data-md-component=content",
        "id=\"mkdocs_search_modal\"",
    ],
    content_selectors: &[".md-content__inner", ".md-content"],
};
```

Replace the existing `PROFILES` line with:

```rust
pub static PROFILES: &[&Profile] = &[&VITEPRESS, &DOCUSAURUS, &MINTLIFY, &STARLIGHT, &MKDOCS];
```

- [ ] **Step 4: Run tests to confirm they pass**

Run: `cargo test -p aget-lib profile::tests`
Expected: PASS for all tests.

- [ ] **Step 5: Commit**

```bash
git add lib/src/profile.rs
git commit -m "feat(lib): add docusaurus, mintlify, starlight, mkdocs profiles"
```

---

### Task 6: Wire profile detection into `HtmlExtractEngine`

**Files:**
- Modify: `lib/src/engine/html_extract.rs`

- [ ] **Step 1: Add failing tests for engine-level wiring**

In `lib/src/engine/html_extract.rs`, append to `mod tests`:

```rust
#[tokio::test]
async fn test_html_extract_uses_profile_when_detected() {
    let mut server = mockito::Server::new_async().await;
    let _mock = server
        .mock("GET", "/")
        .with_status(200)
        .with_header("content-type", "text/html")
        .with_body(
            r#"<!doctype html><html><head><meta name="generator" content="VitePress"></head>
            <body>
              <nav>SHOULD_NOT_APPEAR</nav>
              <div id="VPContent">
                <h1>From Profile</h1>
                <p>This came via the VitePress profile path.</p>
              </div>
            </body></html>"#,
        )
        .create_async()
        .await;

    let fetcher = Fetcher::new().unwrap();
    let url = Url::parse(&server.url()).unwrap();
    let result = HtmlExtractEngine
        .fetch(&url, &fetcher, &HashMap::new())
        .await
        .unwrap();

    let content = match result {
        EngineResult::Success(s) => s,
        _ => panic!("expected Success"),
    };
    assert!(content.contains("From Profile"));
    assert!(!content.contains("SHOULD_NOT_APPEAR"));
}

#[tokio::test]
async fn test_html_extract_falls_back_to_readability_when_no_profile_matches() {
    let mut server = mockito::Server::new_async().await;
    // No generator meta, no profile needles — generic article
    let _mock = server
        .mock("GET", "/")
        .with_status(200)
        .with_header("content-type", "text/html")
        .with_body(SIMPLE_HTML)
        .create_async()
        .await;

    let fetcher = Fetcher::new().unwrap();
    let url = Url::parse(&server.url()).unwrap();
    let result = HtmlExtractEngine
        .fetch(&url, &fetcher, &HashMap::new())
        .await
        .unwrap();

    assert!(matches!(result, EngineResult::Success(_)));
    // Existing readability path should still extract "Hello World"
    if let EngineResult::Success(content) = result {
        assert!(content.contains("Hello"), "fallback should still extract content");
    }
}
```

- [ ] **Step 2: Run tests to confirm the profile-path test fails**

Run: `cargo test -p aget-lib engine::html_extract::tests::test_html_extract_uses_profile_when_detected`
Expected: FAIL — engine doesn't try profiles yet, falls through to readability which strips the wrapper but may include nav text or otherwise differ.

- [ ] **Step 3: Update the engine to try profiles first**

Replace the `html_to_markdown` and `Engine impl` in `lib/src/engine/html_extract.rs` with:

```rust
fn html_to_markdown(html: &str, url: &Url) -> Result<String> {
    // 1) Try framework-profile-based extraction first.
    if let Some(profile) = crate::profile::detect_profile(html) {
        if let Some(md) = crate::profile::extract_with_profile(html, profile, url) {
            return Ok(md);
        }
    }

    // 2) Fall back to readability + htmd.
    let mut readability = dom_smoothie::Readability::new(html, Some(url.as_str()), None)
        .map_err(|e| AgetError::extraction(e.to_string()))?;

    let article = readability
        .parse()
        .map_err(|e| AgetError::extraction(e.to_string()))?;

    htmd::convert(&article.content)
        .map_err(|e| AgetError::extraction(e.to_string()))
        .or_else(|_| Ok(article.text_content.to_string()))
}
```

(The `Engine` impl below is unchanged — it just calls `html_to_markdown`.)

- [ ] **Step 4: Run tests to confirm they pass**

Run: `cargo test -p aget-lib engine::html_extract::tests`
Expected: PASS — both new tests and the existing two.

- [ ] **Step 5: Commit**

```bash
git add lib/src/engine/html_extract.rs
git commit -m "feat(engine): try framework profile before readability fallback"
```

---

### Task 7: Pipeline-level smoke test

**Files:**
- Modify: `lib/src/pipeline.rs`

- [ ] **Step 1: Add a pipeline test that exercises the html_extract → profile path end-to-end**

Append to `pipeline::tests` in `lib/src/pipeline.rs`:

```rust
#[tokio::test]
async fn test_pipeline_uses_profile_via_html_extract_engine() {
    let mut server = mockito::Server::new_async().await;
    // accept_md and dot_md both miss; html_extract should detect Mintlify and extract.
    server
        .mock("GET", "/")
        .match_header(
            "Accept",
            mockito::Matcher::Regex("text/markdown".to_string()),
        )
        .with_status(404)
        .create_async()
        .await;
    server
        .mock("GET", "/.md")
        .with_status(404)
        .create_async()
        .await;
    server
        .mock("GET", "/")
        .with_status(200)
        .with_header("content-type", "text/html")
        .with_body(
            r#"<html><body>
              <header>NAV</header>
              <main id="content-area">
                <h1>Mintlify Page</h1>
                <p>Lots of doc body text here that should land in the markdown output.</p>
                <p>Second paragraph with **emphasis** and a [link](https://example.com).</p>
              </main>
              <footer>FOOT</footer>
            </body></html>"#,
        )
        .create_async()
        .await;

    let pipeline = Pipeline::new(true).unwrap();
    let url = Url::parse(&server.url()).unwrap();
    let result = pipeline.run(&url, None, false).await.unwrap();

    assert_eq!(result.engine_used, "html_extract");
    assert!(result.content.contains("Mintlify Page"));
    assert!(!result.content.contains("NAV"));
    assert!(!result.content.contains("FOOT"));
}
```

- [ ] **Step 2: Run the test**

Run: `cargo test -p aget-lib pipeline::tests::test_pipeline_uses_profile_via_html_extract_engine`
Expected: PASS.

- [ ] **Step 3: Run full check**

Run: `make check`
Expected: PASS — fmt, clippy (no warnings), all tests, build.

- [ ] **Step 4: Commit**

```bash
git add lib/src/pipeline.rs
git commit -m "test: pipeline uses framework profile via html_extract"
```

---

### Task 8: Update `AGENTS.md` with "Adding a New Profile"

**Files:**
- Modify: `AGENTS.md`

- [ ] **Step 1: Add the new section**

In `AGENTS.md`, immediately after the existing `## Adding a New Engine` section, add:

```markdown
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
```

Also update the file map at the top of `AGENTS.md` to include the new files. Replace the existing tree block with:

```
aget/
├── cli/src/
│   ├── main.rs         # Entry point, run(), wiring
│   └── cli.rs          # Clap arg definitions
└── lib/src/
    ├── config.rs       # Config, DomainRule, apply_url_transform
    ├── error.rs        # AgetError, Result alias
    ├── fetcher.rs      # Fetcher, FetchResponse (reqwest wrapper)
    ├── profile.rs      # Doc-framework profiles (VitePress, Docusaurus, ...)
    ├── quality.rs      # passes_quality heuristic
    ├── pipeline.rs     # Pipeline orchestrator
    └── engine/
        ├── mod.rs          # Engine trait, EngineResult
        ├── accept_md.rs    # Engine 1
        ├── dot_md.rs       # Engine 2
        ├── html_extract.rs # Engine 3 (profile → dom_smoothie + htmd)
        └── registry.rs     # build_chain, engine_by_name
```

- [ ] **Step 2: Commit**

```bash
git add AGENTS.md
git commit -m "docs(agents): document profile system and registry"
```

---

## Self-Review Checklist (run before merging)

- [ ] `make check` — fmt, clippy with `-D warnings`, all tests, build.
- [ ] Manual smoke test against a real VitePress page:
  ```bash
  cargo run -p aget --release -- https://vitepress.dev/guide/what-is-vitepress | head -40
  ```
  Expected: clean markdown of the doc body, no nav/footer/sidebar text.
- [ ] Manual smoke test against a real Docusaurus page (e.g. `https://docusaurus.io/docs`).
- [ ] Manual smoke test against a Mintlify page (e.g. `https://mintlify.com/docs`).
- [ ] Diff the output of an arbitrary news/blog page (no profile match) before vs after this change — readability fallback should produce the same output as before.

---

## Out of Scope (deferred for later plans)

- **Per-profile URL rewrites** (e.g. VitePress's `/page.html → /page.md`). curl.md does this via `resolve(url)` callbacks. Add later if profile extraction proves insufficient.
- **`Accept: text/markdown` routing for profiles that support it natively** (Mintlify, GitBook, Rspress, ExDoc). The existing `accept_md` engine already runs first in the chain, so most of this is already covered indirectly — but a profile-aware `accept_md` could send `Accept: text/markdown` *only* to known-supporting hosts and avoid the extra round-trip elsewhere.
- **Additional profiles**: GitBook, Sphinx, Rspress, Fumadocs, ReadTheDocs, ExDoc. Once the registration pattern is solid, these can be added one at a time as a follow-up PR per profile.
- **Profile-specific normalisation** (e.g. strip Mintlify's `<AgentInstructions>` block, GitBook's footer). Add as a `normalize: fn(&str) -> String` field on `Profile` when the first concrete need arises.
