# kalareach-plugins

KalaReach plugin catalogue: application plugin sources, declarative manifests, fixtures, publisher records and the catalogue build and signing pipeline.

Licensed under the BSD 3-Clause License. See [LICENSE](LICENSE).

Every crate in this repository declares the BSD 3-Clause licence with the SPDX identifier
`BSD-3-Clause`, and the repository holds no third-party source. The pipeline depends on
`kr-plugin-sdk` by Git revision, and a package pins a vendor's source by repository and revision
instead of copying it. The first-party packages under `plugins/kalareach/` are BSD 3-Clause like the
rest of the repository. `.gitignore` excludes assistant instruction files and signing keys at any
depth. `scripts/check-local-names.sh` creates each assistant instruction name in a repository that
holds only the committed `.gitignore`, asks `git check-ignore` about it and refuses a tracked file
that the section's patterns match, and CI runs it on every change.

A clean checkout is enough to set up, build and test. It needs the pinned toolchain and `openssl`,
which `scripts/generate-development-keys.sh` calls to make the development signing keys outside the
checkout, and `cargo test` reads them through `KALAREACH_SIGNING_DIR`, as "Build and test" shows.
Nothing in the repository holds a private key, and the pipeline refuses to sign when it finds one in
the tree.

## Repository layout

| Path | What it holds |
| --- | --- |
| `plugins/<publisher>/<plugin>/` | One package each: manifest, presentation document, optional connector table, assets and fixtures |
| `publishers/` | One record per publisher: identifier, display name, homepage and whether it ships with KalaReach |
| `pipeline/` | The `kalareach-catalogue` command: validation, the index build, TUF signing and verification |
| `revocations/` | One record per withdrawn release, keyed to its exact version and manifest digest |
| `fixtures/agents/` | The agent builds each connector is pinned to, and the qualification records the index takes each release's qualified builds from |
| `snapshots/` | Built generations. `snapshots/development/` is a signed fixture; everything else is build output |
| `scripts/` | Development key generation, running the pipeline against a local core checkout, and the check that `.gitignore` keeps the local workspace names out of every commit |
| `docs/` | How to publish a package, and how the pipeline works |

A package is validated with `kr-plugin-sdk` from the core repository, which is the same code a host
runs before it trusts a package. The dependency is pinned by Git revision in `Cargo.lock`, so a
package this repository publishes is a package the pinned host accepts.

## Build and test

Requirements: the toolchain pinned in `rust-toolchain.toml` (rustup installs it on first use) and
`openssl`, which makes the development signing keys.

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
scripts/generate-development-keys.sh
export KALAREACH_SIGNING_DIR="${XDG_DATA_HOME:-$HOME/.local/share}/kalareach-plugins/signing"
cargo run -p kalareach-catalogue -- root --signing-dir "$KALAREACH_SIGNING_DIR"
cargo test
cargo run -p kalareach-catalogue -- validate
cargo run -p kalareach-catalogue -- verify snapshots/development
```

Validation checks every package against the package contract, then evaluates each package's
fixtures against its own control predicates. Verification runs the TUF client over the committed
development generation, which is a really signed generation produced with a development key, and
reads every target through it.

## Building a generation

Signing keys live outside this repository, and the pipeline refuses to run if it finds a private key
inside it. The development set that "Build and test" makes signs a generation: build it and verify
it.

```bash
cargo run -p kalareach-catalogue -- build --signing-dir "$KALAREACH_SIGNING_DIR" --out /tmp/generation
cargo run -p kalareach-catalogue -- verify /tmp/generation
```

[docs/publishing.md](docs/publishing.md) covers what a package must contain and what a review pins.
[docs/pipeline.md](docs/pipeline.md) covers the build, the trust roles and the key handling.
