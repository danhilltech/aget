# Purl-style build & release pipeline

Adopt the build/release/CI system used by [stripe/purl](https://github.com/stripe/purl) for `aget`, with the platform matrix expanded to five targets and supply-chain integrity (SHA256 checksums + GitHub build provenance attestations) added.

## Goals

- Lint and test every push and PR.
- On every `v*` tag, build release binaries for the five supported platforms in parallel on native runners and publish them as a GitHub Release.
- Each release includes a `SHA256SUMS` manifest and a build-provenance attestation per binary.
- No external secrets to manage. No third-party signing infrastructure.
- Stay close to purl's structure so the system is recognisable to anyone familiar with it.

## Non-goals

- Homebrew tap automation. (Out of scope; can be added later in a separate spec.)
- Publishing to `crates.io`. (Out of scope; can be added later.)
- Code signing for the host OS (Apple notarization, Windows Authenticode).
- Cross-compilation via `cross` or QEMU. We use native runners for every target.
- Windows in the CI test matrix. The Windows binary is built on tag only.

## Architecture

Three workflow/config files under `.github/`, plus small Makefile tweaks. Tag-driven release model: `git push --tags` triggers everything else.

```
.github/
├── workflows/
│   ├── ci.yml         # push/PR → lint + test
│   └── release.yml    # v* tag → build + attest + publish release
└── dependabot.yml     # weekly cargo + github-actions PRs
Makefile               # adjusted to match purl conventions
```

## Components

### 1. `.github/workflows/ci.yml`

Verbatim port of purl's CI workflow. Triggers on push and PR to `main` or `master`.

- `permissions: contents: read` (least privilege).
- `lint` job, on `ubuntu-latest`:
  - `actions/checkout@v6`
  - `dtolnay/rust-toolchain@stable` with `components: rustfmt, clippy`
  - `Swatinem/rust-cache@v2`
  - `cargo fmt --all -- --check`
  - `cargo clippy --locked --workspace -- -D warnings`
- `test` job, matrix on `[ubuntu-latest, macos-latest]`:
  - `actions/checkout@v6`
  - `dtolnay/rust-toolchain@stable`
  - `Swatinem/rust-cache@v2`
  - `cargo build --locked --workspace --verbose`
  - `cargo test --locked --workspace --verbose`

### 2. `.github/workflows/release.yml`

Triggers on push of any `v*` tag.

Top-level `permissions: contents: read`. Per-job permissions override below.

#### `build` job

Permissions: `contents: read`, `id-token: write`, `attestations: write` (the latter two are required by `actions/attest-build-provenance`).

Strategy matrix (five entries):

| target                          | runner            | asset                    |
| ------------------------------- | ----------------- | ------------------------ |
| `x86_64-unknown-linux-gnu`      | `ubuntu-latest`   | `aget-linux-amd64`       |
| `aarch64-unknown-linux-gnu`     | `ubuntu-24.04-arm`| `aget-linux-arm64`       |
| `x86_64-apple-darwin`           | `macos-latest`*   | `aget-darwin-amd64`      |
| `aarch64-apple-darwin`          | `macos-latest`    | `aget-darwin-arm64`      |
| `x86_64-pc-windows-msvc`        | `windows-latest`  | `aget-windows-amd64.exe` |

\* The Intel-Mac build cross-compiles from `macos-latest` (Apple Silicon) using `--target x86_64-apple-darwin`. GitHub retired the `macos-13` Intel runner; remaining native Intel runners (`macos-15-intel`, `*-large`, `*-xlarge`) are paid. Cross-compilation is free, well-supported by the Rust + Apple toolchain, and works because all our deps use `rustls-tls` (no OpenSSL/C cross-compile pain).

Steps per matrix entry:

1. `actions/checkout@v6`
2. `rustup toolchain install stable --profile minimal`
3. `rustup target add ${{ matrix.target }}`
4. `actions/cache@v5` keyed on `${{ runner.os }}-cargo-${{ matrix.target }}-${{ hashFiles('**/Cargo.lock') }}` covering `~/.cargo/registry`, `~/.cargo/git`, and `target`.
5. `cargo build --release --locked --workspace --target ${{ matrix.target }}`
6. Strip step, conditional `if: matrix.os != 'windows-latest'`: `strip target/${{ matrix.target }}/release/aget`
7. Rename / move binary to the target asset name (use `aget.exe` source on Windows).
8. `actions/upload-artifact@v7` with `name: ${{ matrix.asset }}` and `path: ${{ matrix.asset }}`.
9. `actions/attest-build-provenance@v3` with `subject-path: ${{ matrix.asset }}`.

#### `release` job

`needs: build`. `runs-on: ubuntu-latest`. Permissions: `contents: write`.

1. `actions/checkout@v6`
2. `actions/download-artifact@v8` with `path: artifacts` and `merge-multiple: true`.
3. Generate checksums:
   ```yaml
   - name: Generate checksums
     working-directory: artifacts
     run: sha256sum * > SHA256SUMS
   ```
4. Create or update the release (purl's exact pattern):
   ```bash
   if gh release view ${{ github.ref_name }} &>/dev/null; then
     gh release upload ${{ github.ref_name }} artifacts/* --clobber
   else
     gh release create ${{ github.ref_name }} artifacts/* --generate-notes
   fi
   ```
   With `env: GH_TOKEN: ${{ secrets.GITHUB_TOKEN }}`.

Release-notes are auto-generated from PR/commit titles since the last tag. Attestations are stored on GitHub itself, not as release assets.

### 3. `.github/dependabot.yml`

Verbatim copy of purl's:

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

### 4. `Makefile`

Targeted edits to align with purl's conventions:

- `install`: add `--locked` → `cargo install --locked --path cli`
- New `uninstall`: `cargo uninstall aget`
- New `clean`: `cargo clean`
- New `run`: `cargo run -q -- $(ARGS)` (so `make run ARGS="https://example.com"` works)
- `fmt`: append `--allow-staged` to the clippy fix line so it doesn't refuse with staged changes.
- `.PHONY` line updated to list every target.

`build`, `release`, `test`, `check` are already correct and unchanged.

### 5. `README.md`

Add a "Pre-built binaries" subsection under Install. Existing `cargo install --path cli` block becomes "From source". Example for the new section:

```bash
# macOS Apple Silicon
curl -L https://github.com/danhilltech/aget/releases/latest/download/aget-darwin-arm64 -o aget
chmod +x aget && mv aget /usr/local/bin/

# Verify provenance (optional)
gh attestation verify aget --repo danhilltech/aget
```

A short note that `SHA256SUMS` is published with each release and can be checked with `sha256sum -c SHA256SUMS`.

## Release process

1. Bump `version` in the workspace `Cargo.toml`.
2. `cargo build --locked` to refresh `Cargo.lock`.
3. `git commit -am "release: vX.Y.Z"`
4. `git tag vX.Y.Z`
5. `git push && git push --tags`

The tag push triggers `release.yml`. No manual artifact uploads.

## Verification (consumer side)

- Integrity: `sha256sum -c SHA256SUMS` against the binary.
- Provenance: `gh attestation verify <binary> --repo danhilltech/aget`.

## Trade-offs and rationale

- **Native runners over `cross`**: with `rustls-tls` already in use, no OpenSSL/glibc cross-build complications exist, and ubuntu-24.04-arm is a free GA runner. Native is faster, simpler, and more debuggable than cross-compilation.
- **GitHub attestations over GPG/cosign**: zero secret management, no extra signing infrastructure, verification only requires the `gh` CLI consumers already have. Cosign and GPG remain additive future options.
- **No Windows in CI test matrix**: matches purl's CI minimalism. Tag builds will catch outright compile failures; runtime regressions on Windows are accepted as low-risk for this CLI.
- **Release notes auto-generated**: `--generate-notes` produces "What's Changed" from PR titles, sufficient for a small project. A `CHANGELOG.md` can be added later if releases warrant curated notes.
