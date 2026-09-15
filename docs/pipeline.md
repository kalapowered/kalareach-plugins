# The catalogue pipeline

`kalareach-catalogue` turns the packages in this repository into a signed catalogue generation. It
does three things in order, and stops at the first one that fails.

```bash
cargo run -p kalareach-catalogue -- validate
cargo run -p kalareach-catalogue -- root --signing-dir <dir>
cargo run -p kalareach-catalogue -- build --signing-dir <dir> --out <dir>
cargo run -p kalareach-catalogue -- verify <generation>
cargo run -p kalareach-catalogue -- bench --entries 10000
```

## Validation

Every publisher record is read, every package under `plugins/` is validated with `kr-plugin-sdk`,
and every package fixture is evaluated against that package's own control predicates.

The validator is the host's. That is the point of depending on the SDK by revision rather than
reimplementing the checks here: a package this repository accepts is a package the pinned host
accepts, and the two cannot drift.

Validation reports every defect it finds rather than stopping at the first, because somebody fixing
a package wants the whole list.

## The index

The index is the complete signed metadata snapshot a host synchronises: compact descriptions,
declarative match rules, capability declarations and immutable payload hashes and sizes. Offline
search covers all of it, so everything a host needs to search, match and decide is in the index, and
everything it would only need after deciding stays behind a content hash.

The build is deterministic. Entries are ordered by publisher, plugin name and version, object keys
are sorted, and the rendering carries no insignificant whitespace. Pass `--produced-at` to fix the
one field that would otherwise come from the clock, and the same packages produce the same bytes.
Without that, a signature over an index would only prove which run produced it.

The rendering is compact because the index is measured against a byte budget. A repository's default
budgets are 64 MiB of metadata and 100,000 entries, and a host holds the whole index so that search
works offline. `bench` measures where those two limits bind for the packages you actually have:

```text
entries              10000
index size           17070095 bytes (16.3 MiB)
bytes per entry      1707
fits 64 MiB budget   39313 entries
```

## The generation

A generation is one immutable directory:

```text
<generation>/
  root.json          the trust root a host ships or adopts out of band
  metadata/          root, timestamp, snapshot and targets metadata
  targets/           index.json and every package payload
```

Targets are named by their whole path, so one repository can hold several versions of the same
package and a host can pin one of them:

```text
index.json
packages/kalareach/example-declarative/0.1.0/plugin.json
packages/kalareach/example-declarative/0.1.0/presentation.json
```

`verify` runs the `tough` client over a generation, with the same expiry enforcement a host uses,
and reads the index back through it. A target whose bytes changed fails the digest check that the
targets metadata pins, and metadata whose signature no longer covers it fails before any target is
read.

`snapshots/README.md` describes the committed development generation and what it is for.

## Keys

Four roles, four keys: root, targets, snapshot and timestamp. Separating them is the design. A
compromised timestamp key lets an attacker hold a client on an old generation; it does not let them
publish a package. A compromised targets key lets them publish a package; it does not let them
change which keys are trusted.

Keys never live in this repository. Before anything is signed, the pipeline walks the working tree
and refuses to run if it finds a private key inside it. The scan checks names and content, so a key
renamed to `notes.txt` is still found: every PEM key opens with a header this looks for. The signing
directory itself is also checked, and one inside the repository is refused.

That check is not a formality. A key that reaches a commit has to be rotated, and rotation means
publishing a new root, waiting for every client to pick it up, and explaining why.

Generate a development set:

```bash
scripts/generate-development-keys.sh
```

It writes four RSA keys, mode 600, into `${XDG_DATA_HOME:-$HOME/.local/share}/kalareach-plugins/signing`,
or into a directory you name, and refuses to write inside this repository. Then write a trust root
over them:

```bash
cargo run -p kalareach-catalogue -- root --signing-dir "$KALAREACH_SIGNING_DIR"
```

Production keys are generated in the signing environment, on the machine that will hold them, and
never leave it. The pipeline reads keys through a source abstraction, so a key store that is not a
file plugs into the same path.

## Working against an unreleased core revision

`pipeline/Cargo.toml` depends on `kr-plugin-sdk` by Git URL and exact revision. That pin is the
package contract this catalogue is built against.

When the SDK and the catalogue change together, the revision exists in a local checkout before it is
anywhere else. `scripts/with-local-core.sh` rewrites the fetch URL for the duration of one command:

```bash
KALAREACH_CORE=/path/to/kalareach scripts/with-local-core.sh cargo test
```

`Cargo.toml` and `Cargo.lock` are untouched, so what this builds is what a fetch from the published
repository builds.

## Continuous integration

`.github/workflows/plugins-ci.yml` formats, lints, tests, validates every package and verifies the
committed development generation. It does not sign: signing needs keys, and keys are not in CI.
