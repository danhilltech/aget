# Purl-style Build & Release Pipeline Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add purl-style CI, tag-driven release pipeline, dependabot, and Makefile/README polish to `aget`.

**Architecture:** Two GitHub Actions workflows (`ci.yml`, `release.yml`) plus `dependabot.yml`, all under `.github/`. CI runs lint + test on every push/PR. The release workflow triggers on `v*` tags, builds 5 native binaries in parallel, generates `SHA256SUMS`, attests build provenance, and publishes a GitHub Release. Makefile and README are nudged to match purl's conventions and document the new install/verify paths.

**Tech Stack:** GitHub Actions, Rust workspace (cargo), `actionlint` for static workflow validation, `dtolnay/rust-toolchain`, `Swatinem/rust-cache`, `actions/cache`, `actions/upload-artifact`, `actions/download-artifact`, `actions/attest-build-provenance`, `gh` CLI.

**Pre-req:** all work is done on a feature branch off `main`. Create one before starting:

```bash
git checkout -b purl-style-releases
```

---

## Task 1: Create CI workflow

**Files:**
- Create: `.github/workflows/ci.yml`

- [ ] **Step 1: Create the workflow file**

Create `.github/workflows/ci.yml` with this exact content:

```yaml
name: CI

on:
  push:
    branches: [main, master]
  pull_request:
    branches: [main, master]

permissions:
  contents: read

env:
  CARGO_TERM_COLOR: always

jobs:
  lint:
    name: Lint
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v6
      - uses: dtolnay/rust-toolchain@stable
        with:
          components: rustfmt, clippy
      - uses: Swatinem/rust-cache@v2
      - run: cargo fmt --all -- --check
      - run: cargo clippy --locked --workspace -- -D warnings

  test:
    name: Test (${{ matrix.os }})
    runs-on: ${{ matrix.os }}
    strategy:
      matrix:
        os: [ubuntu-latest, macos-latest]
    steps:
      - uses: actions/checkout@v6
      - uses: dtolnay/rust-toolchain@stable
      - uses: Swatinem/rust-cache@v2
      - run: cargo build --locked --workspace --verbose
      - run: cargo test --locked --workspace --verbose
```

- [ ] **Step 2: Validate locally that the project still builds clean**

Confirm the workflow's commands actually succeed locally before relying on CI:

```bash
cargo fmt --all -- --check
cargo clippy --locked --workspace -- -D warnings
cargo build --locked --workspace --verbose
cargo test --locked --workspace --verbose
```

Expected: all four exit 0. If any fails, fix the underlying issue in the codebase — do not weaken CI to make it pass.

- [ ] **Step 3: Commit**

```bash
git add .github/workflows/ci.yml
git commit -m "ci: add lint + test workflow"
```

---

## Task 2: Create release workflow

**Files:**
- Create: `.github/workflows/release.yml`

- [ ] **Step 1: Create the workflow file**

Create `.github/workflows/release.yml` with this exact content:

```yaml
name: Release

on:
  push:
    tags:
      - 'v*'

permissions:
  contents: read

env:
  CARGO_TERM_COLOR: always

jobs:
  build:
    name: Build (${{ matrix.target }})
    runs-on: ${{ matrix.os }}
    permissions:
      contents: read
      id-token: write
      attestations: write
    strategy:
      fail-fast: false
      matrix:
        include:
          - os: ubuntu-latest
            target: x86_64-unknown-linux-gnu
            asset: aget-linux-amd64
            ext: ""
          - os: ubuntu-24.04-arm
            target: aarch64-unknown-linux-gnu
            asset: aget-linux-arm64
            ext: ""
          - os: macos-13
            target: x86_64-apple-darwin
            asset: aget-darwin-amd64
            ext: ""
          - os: macos-latest
            target: aarch64-apple-darwin
            asset: aget-darwin-arm64
            ext: ""
          - os: windows-latest
            target: x86_64-pc-windows-msvc
            asset: aget-windows-amd64.exe
            ext: ".exe"
    steps:
      - uses: actions/checkout@v6
      - run: rustup toolchain install stable --profile minimal
      - run: rustup target add ${{ matrix.target }}
      - uses: actions/cache@v5
        with:
          path: |
            ~/.cargo/registry
            ~/.cargo/git
            target
          key: ${{ runner.os }}-cargo-${{ matrix.target }}-${{ hashFiles('**/Cargo.lock') }}
          restore-keys: |
            ${{ runner.os }}-cargo-${{ matrix.target }}-
      - run: cargo build --release --locked --workspace --target ${{ matrix.target }}
      - name: Strip binary
        if: runner.os != 'Windows'
        run: strip target/${{ matrix.target }}/release/aget
      - name: Stage asset
        shell: bash
        run: cp "target/${{ matrix.target }}/release/aget${{ matrix.ext }}" "${{ matrix.asset }}"
      - uses: actions/upload-artifact@v7
        with:
          name: ${{ matrix.asset }}
          path: ${{ matrix.asset }}
      - uses: actions/attest-build-provenance@v3
        with:
          subject-path: ${{ matrix.asset }}

  release:
    name: Release
    needs: build
    runs-on: ubuntu-latest
    permissions:
      contents: write
    steps:
      - uses: actions/checkout@v6
      - uses: actions/download-artifact@v8
        with:
          path: artifacts
          merge-multiple: true
      - name: Generate checksums
        working-directory: artifacts
        run: sha256sum * > SHA256SUMS
      - name: Create or update release
        env:
          GH_TOKEN: ${{ secrets.GITHUB_TOKEN }}
        run: |
          if gh release view ${{ github.ref_name }} &>/dev/null; then
            echo "Release already exists, uploading assets..."
            gh release upload ${{ github.ref_name }} artifacts/* --clobber
          else
            echo "Creating new release..."
            gh release create ${{ github.ref_name }} artifacts/* --generate-notes
          fi
```

- [ ] **Step 2: Sanity-check the file structure**

Run a quick parse to confirm the YAML is valid before committing:

```bash
python3 -c "import yaml; yaml.safe_load(open('.github/workflows/release.yml'))" && echo OK
```

Expected: `OK`. If it errors, the YAML has a structural issue — fix and re-run.

- [ ] **Step 3: Commit**

```bash
git add .github/workflows/release.yml
git commit -m "ci: add tag-driven release pipeline with checksums and attestations"
```

---

## Task 3: Add dependabot config

**Files:**
- Create: `.github/dependabot.yml`

- [ ] **Step 1: Create the config**

Create `.github/dependabot.yml` with this exact content:

```yaml
version: 2
updates:
  - package-ecosystem: "cargo"
    directory: "/"
    schedule:
      interval: "weekly"
    open-pull-requests-limit: 10

  - package-ecosystem: "github-actions"
    directory: "/"
    schedule:
      interval: "weekly"
    open-pull-requests-limit: 10
```

- [ ] **Step 2: Sanity-check YAML**

```bash
python3 -c "import yaml; yaml.safe_load(open('.github/dependabot.yml'))" && echo OK
```

Expected: `OK`.

- [ ] **Step 3: Commit**

```bash
git add .github/dependabot.yml
git commit -m "ci: enable dependabot for cargo and github-actions"
```

---

## Task 4: Update Makefile

**Files:**
- Modify: `Makefile`

- [ ] **Step 1: Replace the Makefile contents**

Overwrite `Makefile` with this exact content:

```make
.PHONY: build run release clean check test fmt install uninstall

build:
	cargo build

run:
	cargo run -q -- $(ARGS)

release:
	cargo build --release

install:
	cargo install --locked --path cli

uninstall:
	cargo uninstall aget

clean:
	cargo clean

test:
	cargo test

check:
	cargo fmt --check
	cargo clippy -- -D warnings
	cargo test
	cargo build

fmt:
	cargo fmt
	cargo clippy --fix --allow-dirty --allow-staged
```

- [ ] **Step 2: Verify each target invokes**

These should all parse and either succeed or fail for code reasons, not Makefile reasons:

```bash
make -n build
make -n run
make -n release
make -n install
make -n uninstall
make -n clean
make -n test
make -n check
make -n fmt
```

Expected: each prints the underlying `cargo …` command and exits 0. (`-n` is dry-run.)

- [ ] **Step 3: Smoke-test the new `run` target with ARGS**

```bash
make -n run ARGS="https://example.com"
```

Expected output contains: `cargo run -q -- https://example.com`.

- [ ] **Step 4: Commit**

```bash
git add Makefile
git commit -m "build: align Makefile with purl conventions"
```

---

## Task 5: Update README install section

**Files:**
- Modify: `README.md`

- [ ] **Step 1: Replace the Install section**

Find this block in `README.md` (currently lines 5–15):

```markdown
## Install

```bash
cargo install --path cli
```

Or via make:

```bash
make install
```
```

Replace it with this exact block:

```markdown
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
```

- [ ] **Step 2: Confirm the file still renders cleanly**

Eyeball with:

```bash
head -60 README.md
```

Expected: the new "Pre-built binaries" subsection appears, followed by "From source", followed by the existing "Usage" heading.

- [ ] **Step 3: Commit**

```bash
git add README.md
git commit -m "docs: document pre-built binary installation"
```

---

## Task 6: Static-validate workflows with actionlint

**Files:**
- (none — pure validation step)

- [ ] **Step 1: Install actionlint**

```bash
brew install actionlint
```

Expected: actionlint installs successfully. (If brew is unavailable, download the binary from <https://github.com/rhysd/actionlint/releases/latest> and put it on `PATH`.)

- [ ] **Step 2: Run actionlint over the workflow files**

```bash
actionlint .github/workflows/ci.yml .github/workflows/release.yml
```

Expected: zero output and exit 0. If actionlint reports errors:
- Read each error carefully — it will name the file, line, and rule.
- Fix the underlying YAML/expression issue in the workflow.
- Re-run actionlint until clean.
- If actionlint flags `ubuntu-24.04-arm` as an unknown runner label, suppress just that warning by re-running with `-ignore '"ubuntu-24.04-arm"'` or equivalent — `ubuntu-24.04-arm` is a real GA runner that older actionlint releases may not yet recognize. Do not change the runner.

- [ ] **Step 3: Verify dependabot config**

```bash
python3 -c "import yaml; yaml.safe_load(open('.github/dependabot.yml'))" && echo OK
```

Expected: `OK`. (GitHub's own dependabot validation only runs server-side once the file is on the default branch; the YAML parse is the strongest local check.)

- [ ] **Step 4: No commit needed**

This task is verification only. If any fixes were made during Step 2, they should already be committed under their relevant prior task — amend nothing. If you discover a fix is needed and have already left the task that owned the file, create a new commit:

```bash
git add .github/workflows/<file>
git commit -m "ci: fix actionlint findings"
```

---

## Task 7: Push branch and verify CI

**Files:**
- (none — push + observe)

- [ ] **Step 1: Push the branch**

```bash
git push -u origin purl-style-releases
```

Expected: push succeeds. The push triggers `ci.yml` (because the workflow file already exists on the branch, GitHub picks it up).

- [ ] **Step 2: Watch the CI run**

```bash
gh run watch
```

Or list the runs:

```bash
gh run list --branch purl-style-releases --limit 5
```

Expected: one CI run with three jobs (`Lint`, `Test (ubuntu-latest)`, `Test (macos-latest)`). All three should finish green.

- [ ] **Step 3: If CI fails, diagnose**

For each failing job:

```bash
gh run view --log-failed
```

Common causes and fixes:
- Clippy/format failure → fix locally, `git commit --amend` or new commit, push again.
- Test failure on macOS only → likely a real cross-platform bug; debug in the code, not the workflow.
- Workflow syntax error → actionlint should have caught it; re-run actionlint from Task 6.

Iterate until all CI jobs pass on the branch.

- [ ] **Step 4: Open a PR**

```bash
gh pr create --fill
```

Expected: PR opens. CI re-runs on the PR (this is fine; it should still pass). Do **not** merge yet — Task 8 needs the branch unmerged so we can validate the release pipeline before exposing it on `main`.

---

## Task 8: Dry-run the release pipeline on a pre-release tag

**Files:**
- (none — tag, push, observe, clean up)

This task validates the release workflow end-to-end against a throwaway pre-release tag, before the user ever cuts a real `v0.1.1`. We cut `v0.1.1-rc.1` on the feature branch, watch the workflow, sanity-check the artifacts, then delete the tag and the GH release.

- [ ] **Step 1: Create the dry-run tag on the current branch HEAD**

```bash
git tag v0.1.1-rc.1
git push origin v0.1.1-rc.1
```

Expected: tag pushes. The push triggers `release.yml`.

- [ ] **Step 2: Watch the release run**

```bash
gh run watch
```

Expected: one Release run with 6 jobs total — five `Build (...)` jobs (one per matrix entry) and one `Release` job at the end. All should finish green.

- [ ] **Step 3: Inspect the release**

```bash
gh release view v0.1.1-rc.1
```

Expected output includes 6 assets:
- `aget-linux-amd64`
- `aget-linux-arm64`
- `aget-darwin-amd64`
- `aget-darwin-arm64`
- `aget-windows-amd64.exe`
- `SHA256SUMS`

- [ ] **Step 4: Verify a binary downloads, runs, and attests**

Pick the asset matching the local machine. On Apple Silicon macOS:

```bash
mkdir -p /tmp/aget-rc && cd /tmp/aget-rc
gh release download v0.1.1-rc.1 -p aget-darwin-arm64 -p SHA256SUMS
chmod +x aget-darwin-arm64
shasum -a 256 -c SHA256SUMS --ignore-missing
./aget-darwin-arm64 --version
gh attestation verify aget-darwin-arm64 --repo danhilltech/aget
```

Expected:
- `shasum -c` prints `aget-darwin-arm64: OK`.
- `--version` prints `aget 0.1.0` (or whatever the current workspace version is).
- `gh attestation verify` prints a "Loaded digest ... was signed by ..." success message.

(Linux: substitute `sha256sum -c` for `shasum -a 256 -c`.)

- [ ] **Step 5: Delete the dry-run tag and release**

```bash
gh release delete v0.1.1-rc.1 --yes --cleanup-tag
```

Expected: release and tag both removed. `git tag -l v0.1.1-rc.1` should produce no output after fetching:

```bash
git fetch --prune --prune-tags origin
git tag -l v0.1.1-rc.1
```

- [ ] **Step 6: Merge the PR from Task 7**

Now that both CI and the release pipeline are confirmed working:

```bash
gh pr merge --squash --delete-branch
```

Expected: PR merges to `main`, branch deleted locally and remotely. CI runs once more on `main`; this is the final green signal.

---

## Task 9: Cut the first real release (optional, on demand)

This task is intentionally separate — only run it when you're ready to actually publish.

**Files:**
- Modify: `Cargo.toml` (workspace `version`)

- [ ] **Step 1: Bump the workspace version**

Edit `Cargo.toml`. In the `[workspace.package]` block, change `version = "0.1.0"` to the new release version, e.g. `version = "0.1.1"`.

- [ ] **Step 2: Refresh the lockfile**

```bash
cargo build --locked
```

Expected: builds successfully and updates `Cargo.lock`.

- [ ] **Step 3: Commit, tag, push**

```bash
git add Cargo.toml Cargo.lock
git commit -m "release: v0.1.1"
git tag v0.1.1
git push && git push --tags
```

Expected: tag push triggers the release workflow. After it completes, `gh release view v0.1.1` shows all 6 assets.
