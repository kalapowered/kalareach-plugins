# kalareach-plugins

KalaReach plugin catalogue: application plugin sources, declarative manifests, fixtures, publisher records and the catalogue build and signing pipeline.

Licensed under the BSD 3-Clause License. See [LICENSE](LICENSE).

## Repository layout

| Path | What it holds |
| --- | --- |
| `plugins/<publisher>/<plugin>/` | One package each: manifest, presentation document, optional connector table, assets and fixtures |
| `publishers/` | One record per publisher: identifier, display name, homepage and whether it ships with KalaReach |
| `pipeline/` | The `kalareach-catalogue` command: validation, the index build, TUF signing and verification |
| `snapshots/` | Built generations. `snapshots/development/` is a signed fixture; everything else is build output |
| `scripts/` | Development key generation, and running the pipeline against a local core checkout |
| `docs/` | How to publish a package, and how the pipeline works |

A package is validated with `kr-plugin-sdk` from the core repository, which is the same code a host
runs before it trusts a package. The dependency is pinned by Git revision in `Cargo.lock`, so a
package this repository publishes is a package the pinned host accepts.

## Build and test

Requirements: the toolchain pinned in `rust-toolchain.toml` (rustup installs it on first use).

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test
cargo run -p kalareach-catalogue -- validate
cargo run -p kalareach-catalogue -- verify snapshots/development
```

Validation checks every package against the package contract, then evaluates each package's
fixtures against its own control predicates. Verification runs the TUF client over the committed
development generation, which is a really signed generation produced with a development key.

## Building a generation

Signing keys live outside this repository, and the pipeline refuses to run if it finds a private key
inside it. Generate a development set once:

```bash
scripts/generate-development-keys.sh
export KALAREACH_SIGNING_DIR="${XDG_DATA_HOME:-$HOME/.local/share}/kalareach-plugins/signing"
cargo run -p kalareach-catalogue -- root --signing-dir "$KALAREACH_SIGNING_DIR"
```

Then build and verify:

```bash
cargo run -p kalareach-catalogue -- build --signing-dir "$KALAREACH_SIGNING_DIR" --out /tmp/generation
cargo run -p kalareach-catalogue -- verify /tmp/generation
```

[docs/publishing.md](docs/publishing.md) covers what a package must contain and what a review pins.
[docs/pipeline.md](docs/pipeline.md) covers the build, the trust roles and the key handling.
