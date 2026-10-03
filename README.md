# kalareach-plugins

This repo contains the catalogue for the KalaReach plugins: application plugin sources, their
declarative manifests, fixtures, and the records for publishers of plugins. It also contains the
code for the pipeline that builds and signs the catalogue.

This software is licensed under the terms of the BSD 3-Clause license. A copy of this license can be
found in [LICENSE](LICENSE).

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

The plugin SDK crate `kr-plugin-sdk`, from the core repo, contains the code for validating plugin
packages. This is used here to run the same code that a host would use to validate packages before
trusting them. It is pinned to a specific revision in `Cargo.lock`, so packages published from this
repository are guaranteed to be accepted by a host pinned to the corresponding revision.

## Build and test

This project requires the rust toolchain specified in the `rust-toolchain.toml` file. If using
rustup, this will be automatically installed when first used. It also requires `openssl`, as it is
used to generate the signing keys used in development.

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

To validate packages, the pipeline verifies that all packages in the catalogue conform to the
package contract, and then runs the fixtures for each package against their own control predicates
to verify that they are correct. To verify the package data, the pipeline runs the TUF client
against a generation committed in this repository. This generation is "really" signed, but uses a
development key. All targets are read through the TUF client to verify them.

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
