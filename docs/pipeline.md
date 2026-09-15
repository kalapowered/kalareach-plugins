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
index size           17800095 bytes (17.0 MiB)
bytes per entry      1780
index fits 64 MiB    37701 entries
```

The last line counts the index and nothing else. The TUF metadata over it grows with the target
count too, so the real figure is lower. Every synthetic entry comes from one template, so the size
per entry is that package's metadata: a catalogue of longer descriptions and more match rules costs
more per entry.

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

Each role's metadata version is the generation number. A client refuses metadata whose version is
lower than the one it already trusts, so a generation replayed after a later one is rejected rather
than accepted as an update.

A generation is written once. `build` checks the generation number, the destination and the keys
before it writes or removes anything, assembles the generation in a directory it creates beside the
destination, verifies it there, and moves it into place only then. `--replace` writes over a
destination that already holds a generation, and only one: a directory that holds anything besides
a root, its metadata and its targets is refused rather than deleted, so pointing a build at a
signing directory or a checkout is a mistake that stops rather than one that costs you the
directory. The generation a replacement displaces is moved beside the destination under a dotted name and left
there. The build prints where it went. The pipeline removes nothing it did not write, because a
directory it checked before a build is not necessarily the same directory afterwards.

Before signing, every staged byte is hashed again and compared with what validation saw. Staging
reopens the package files, and a file edited in between would otherwise be signed without ever
having been checked.

`verify` runs the `tough` client over a generation, with the same expiry enforcement a host uses,
and reads every target through it, one at a time. Each one's digest and length are checked as it
streams, and then against what the index declares, so a payload that was replaced with another
package's, truncated or removed is caught rather than left for a host to find. Metadata whose
signature no longer covers it fails before any target is read.

`snapshots/README.md` describes the committed development generation and what it is for.

## Keys

Four roles, four keys: root, targets, snapshot and timestamp. Separating them is the design. A
compromised timestamp key lets an attacker hold a client on an old generation; it does not let them
publish a package. A compromised targets key lets them publish a package; it does not let them
change which keys are trusted.

Each role's key is its own. A trust root that named one key for all four would grant whoever holds it
every role, and the pipeline refuses to build one.

Keys never live in this repository. Before anything is signed, the pipeline walks the working tree
and refuses to run if it finds a private key inside it. Inside a checkout, which it recognises by
finding a `.git` at the tree or above it, it asks Git which files could reach a commit, which is every tracked file wherever it sits plus every untracked file Git would add, and
scans exactly those. That leaves out build output, where the scan would otherwise find its own
header constants compiled into a binary, and it leaves nothing out that a commit could carry.
Outside a checkout it walks the tree instead, skipping the directories that hold build output.

Each file is read to its end, and the PEM header is looked for anywhere in a line: a key renamed to
`notes.txt` is found, and so is one pasted into the middle of a long file or inside a configuration
value. The signing directory itself is checked too, and one inside the repository is refused.

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

## Building against a local core checkout

`pipeline/Cargo.toml` depends on `kr-plugin-sdk` by Git URL and exact revision. That pin is the
package contract this catalogue is built against.

`scripts/with-local-core.sh` rewrites the fetch URL for the duration of one command, so Cargo reads
a checkout on this machine instead of the network:

```bash
KALAREACH_CORE=/path/to/kalareach scripts/with-local-core.sh cargo test
```

The revision is unchanged, so Cargo resolves the same commit object either way, and `Cargo.toml` and
`Cargo.lock` keep the canonical URL. The rewrite lives in that one command's environment: it changes
no Git configuration, and it applies to every Git subprocess the command starts.

## Continuous integration

`.github/workflows/plugins-ci.yml` formats, lints, generates a development key set outside the
checkout, writes a trust root over it, runs the tests, validates every package, builds and verifies
a generation, verifies the committed development generation, and measures a ten thousand entry
catalogue. The keys it makes are its own and last as long as the run; production signing happens in
the signing environment and not here.
