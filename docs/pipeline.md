# The catalogue pipeline

`kalareach-catalogue` turns the packages in this repository into a signed catalogue generation. It
does three things in order, and stops at the first one that fails.

```bash
cargo run -p kalareach-catalogue -- validate
cargo run -p kalareach-catalogue -- root --signing-dir <dir>
cargo run -p kalareach-catalogue -- build --signing-dir <dir> --out <dir>
cargo run -p kalareach-catalogue -- verify <generation>
cargo run -p kalareach-catalogue -- development-root --out <dir>
cargo run -p kalareach-catalogue -- rotate-root --development --epoch <n> --roots-dir <dir>
cargo run -p kalareach-catalogue -- check-generation <new> --previous <old>
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

It then reads the build list and the qualification records under `fixtures/agents` and decides which
executable builds each release names, as the next section describes.

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

### Qualified builds

Each entry names the executable builds of the application its release is qualified against: the
application, the distribution it came from, the upstream version, the operating system and
architecture, and the SHA-256 of the executable. A host takes an executable's version from these and
nowhere else, and holds it to the ranges the release declares for its native bridge recipe and its
connector table, so naming a build opens those version checks for it. An entry that names no build
has no `builds` member at all, which is how every index was written before an entry could name one.

The builds come from `fixtures/agents`, not from the packages. The build list, `builds.json`, pins
each build, and one qualification record per release, build and platform states the outcome of
every part of the specification's eight qualification cases (`fixtures/agents/README.md`). A pinned
build is named in its release's entry only when all of these hold:

- its pinned file is what a process runs, a `native` or `child` launch. A script runs as its
  interpreter and a wheel is an archive, so neither names an executable a host runs;
- its record is in good form and is for this release and this build: the release's version,
  manifest digest and attachment paths, and, as the build every part ran against, the first of the
  record's `run.applications`, installed, with the build's application, version and SHA-256. The
  newer build an upgrade part moved to is listed after it and qualifies nothing;
- the record's run is whole: no failure outside its identifiers, no problem and no failed step;
- no part failed, and every part of the eight cases passed. A part that did not run is not a pass,
  whatever kept it from running.

`validate` prints each build it names and each pinned build it leaves out, with every condition that
build does not meet. Input that cannot be read for what it shows stops the pipeline instead: a record
in bad form (a record has the top-level members its form names and no others, so a `capabilities`
or a `grant` member added there is refused), a record run on another platform than its file name
says, a platform the list does not know, a build pinned twice or for a package this repository
does not publish with a connector table, and one executable named as two versions.

The pipeline writes an entry's `builds` and leaves its `qualification` list empty. The host release
record in the core repository refuses a listed build whose plugin entry cites no qualification
result, so the release record for a build's platform stops at the first build the pipeline names,
until the results are written.

The builds are signed with the index and kept apart from the package's bytes, so a generation adds or
withdraws a build without a new package. A build is added when its record newly qualifies, and
withdrawn when its pin is removed or a newly measured record no longer qualifies; the package's
digest, and every copy a host installed, stay as they are. A withdrawn build's record stays too, and
still has to meet every check the repository makes of it.

Before anything is signed, `build` holds every entry's builds to the SDK's own rule: no more than
`MAX_QUALIFIED_BUILDS` (256) in one entry, each on a platform the release lists, and one version for
each executable. A generation that breaks it is refused, naming the entry, in the SDK's words, and
`verify` refuses a generation that carries one, as a host does.

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
than accepted as an update. A generation number also names one index: a host that has accepted a
number refuses different bytes under it, so a rebuild after any change to a package takes the next
number.

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

## The development lineage and the chain of roots

A host that adopts a root follows signed rotations from it, so a generation ships every root from
version 1 to the highest as `metadata/N.root.json`, and `root.json` is the highest. A rotation is a
root of the next version, signed under a threshold of the root before it and a threshold of its own.

The committed development generation is signed with keys that anybody can derive. Each role's key is
an Ed25519 key whose seed is the SHA-256 of `kalareach-plugins development key ` followed by the
role's name; a later epoch adds ` rotation <n>` after the name. Nothing is written to disk, and no
private-key block appears in the tree. The keys show that the pipeline works and say nothing about
who produced a package: a host trusts a release only for the production root key identifiers its own
build commits.

`development-root` writes the version-1 root. `build --development` signs a generation with the
derived keys and ships the roots it is given. `rotate-root` writes the root that follows another,
signed under both sets of keys. Fix `--produced-at` and `--expires-at`, and a rebuild of the same
packages writes the same bytes: every signed role is written with its members in name order and its
signatures in key identifier order, and the snapshot and the timestamp pin those bytes.

`check-generation <new> --previous <old>` accepts a generation only when its number is higher and
its roots continue the previous generation's. They continue when the highest root is the same, or
when every earlier root is unchanged and each later one carries a threshold of the root before it
and a threshold of its own. The one exception is the break from the root the development generation
had before its first Ed25519 root, pinned by its SHA-256, to the version-1 root the phrase derives.
While the keys are public this is a consistency check and not a trust boundary.

## Keys

There are four roles, each with its own key: root, targets, snapshot and timestamp. Separating them
is the design. A compromised timestamp key lets an attacker hold a client on an old generation; it
does not let them publish a package. A compromised targets key lets them sign package metadata, which a client
accepts only when the snapshot and the timestamp also name it, so on its own it publishes nothing; it does
not let them change which keys are trusted.

Each role's key is its own. A trust root that named one key for two roles would grant whoever holds
it both, and the pipeline refuses to build one. A build checks every root a generation ships, the
ones before the last included, because a host can adopt any of them, and before it writes anything.
The roles are compared by key identifier and by the public key itself: an Ed25519 key spelled as its
32 raw bytes under one role and wrapped in its `SubjectPublicKeyInfo` under another is one key, and
so is an elliptic-curve key as a compressed and as an uncompressed point. The library that checks a
signature reads some spellings that cannot be compared by their bytes (an Ed25519 key with bytes
after its wrapper, an elliptic-curve key as a hybrid point or as a whole `SubjectPublicKeyInfo`, an
RSA key as a whole `SubjectPublicKeyInfo`), and a root that names one is refused, since a key the
comparison cannot place is a key it cannot say is not shared. The spellings it accepts include the
ones the pipeline writes: an Ed25519 key as its 32 raw bytes or in a wrapper of exactly 44 bytes, an
elliptic-curve key as a point of 65 bytes opening `04` or of 33 bytes opening `02` or `03`, and an
RSA key as one `RSAPublicKey`. The comparison finds the spellings of one key and, for an
elliptic-curve key, its negation; it does not find an Ed25519 key and its negation or a small-order
shift of it, or two RSA keys of one modulus with different exponents, which need arithmetic that
the pipeline does not do.

The release owner rotates all four keys. A rotation publishes a new root, signed by the root it
replaces, and a client carries its verification forward from the root it last ended on. If the
root's key is lost, no new root can be signed over it, and a host trusts a new root only when its
owner adopts it out of band. If it is leaked, its holder can sign a successor root that hosts follow
at their next sync, so the owner rotates at once and a host that may have followed another root
re-anchors out of band; the procedure is written at the key ceremony.

Keys never live in this repository. Before each `build`, each `root` and each `rotate-root` that
names a signing directory, the pipeline walks the working tree and refuses to run if it finds a
private key inside it; `development-root` and `rotate-root --development` sign with development keys
that anybody can derive, and do not scan. Inside a checkout, which it recognises by finding a `.git` at
the tree or above it, it asks Git which files could reach a commit, which is every tracked file
wherever it sits plus every untracked file Git would add, and scans exactly those. That leaves out
build output, where the scan would otherwise find its own header constants compiled into a binary,
and whatever Git ignores and does not track, which a commit carries only if somebody forces it in.
This repository ignores `*.pem`, `*.key` and `signing/`, so a key file of such a name elsewhere in
the checkout is not scanned (the signing directory itself is checked, below).
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
or into a directory you name, and refuses to write inside this repository. Then name the directory
and write a trust root over the keys:

```bash
export KALAREACH_SIGNING_DIR="${XDG_DATA_HOME:-$HOME/.local/share}/kalareach-plugins/signing"
cargo run -p kalareach-catalogue -- root --signing-dir "$KALAREACH_SIGNING_DIR"
```

Production keys are generated in the signing environment, on the machine that will hold them, and
never leave it. The pipeline reads each key from a file in its signing directory; the source abstraction it reads
through is where a key store that is not a file would plug in, and none is implemented. A build reads
all four keys from one signing directory and refuses a directory with a key file missing, the root key's
included, so the root key is on the machine that builds a generation. Separating the roles limits what the
leak of one key file does; it does not protect against a compromised build machine.

## Building against a local core checkout

The root `Cargo.toml` names `kr-plugin-sdk` by Git URL and exact revision, which `pipeline/Cargo.toml` reads. That pin is the
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

`.github/workflows/plugins-ci.yml` runs on a push to `main`, on a pull request and on a manual run.
It has three jobs.

`catalogue` first proves that `.gitignore` ignores the local workspace names. It then formats,
lints, generates a development key set outside the checkout, writes a trust root over it, runs the
tests, validates every package, builds and verifies a generation, verifies the committed development
generation, and measures a ten thousand entry catalogue. The keys it makes are its own and last as
long as the run; production signing happens in the signing environment and not here.

`records` checks out the whole history. It fails when a commit being sent, or a file a published
package carries, holds a local-only record, a credential or a build-agent instruction file,
including one added in a commit and deleted in a later commit of the same push. It also fails when
the catalogue's dependency on the core repository is anything but one Git revision that a package
release was packed from, with no path dependency.

`advisories` checks every crate in `Cargo.lock` against the RustSec advisory database as it stands
when the job runs. `.cargo/audit.toml` says what fails the check. A finding fails this job without
hiding the result of another.

An advisory published against a lock that nothing changes would show only at the next push to
`main`, pull request or manual run. So `.github/workflows/advisories.yml` runs the same check,
through the same `.github/actions/rustsec-audit` action, every day at 04:47 UTC on `main` and on a
manual run. It reads the repository with the automatic read-only token, needs no other secret,
reaches no deployment and opens no issue or comment; a finding fails the run, and the failed run is
the report. GitHub can delay a scheduled run at the start of an hour, which is why the time is off
the hour, and under high load it can drop one. It also switches a schedule off in a public
repository after 60 days with no repository activity.

## What the checks hold

Each property of a signed catalogue has a check that fails when the property is removed.

| Property | Check |
| --- | --- |
| Every entry pins its publisher, its source and its package: the publisher has a record here and in the index, the source is a repository and a 40-character revision, and the digest and the source are those of the package's own `plugin.json` | `kr_req_04_13_every_entry_pins_its_publisher_source_and_package_digest` |
| Every entry carries the compatibility data a host admits from: bounded SDK and WIT ranges the pinned versions fall in, the platforms it runs on, and builds only on those platforms | `kr_req_18_06_every_entry_carries_the_compatibility_data_a_host_admits_from` |
| A generation altered after it was signed does not verify: an index, a package file or a revocation | `release.rs`, and `kr_req_25_21_a_revocation_altered_after_signing_fails_verification` |
| A qualification record has only the top-level members its form names, so a `capabilities` or `grant` member added at the top level is refused; what an index carries for a build is what the SDK's types allow, which core's index parse holds | `kr_req_11_18_a_record_with_an_added_capabilities_or_grant_member_is_refused` |
| Every connector package has a qualification record for the build its table is pinned to, in good form and matching its package | `every_connector_package_has_a_qualification_record_for_its_pinned_build`, which this repository's continuous integration runs with the rest of the tests |

That check shows that each connector package has a record that reads and matches its package. It
does not decide that a build qualifies: a record can name a part as not run, and the index names a
build only when every part passed (see Qualified builds). An executable no signed build names has
no version, and a host keeps its terminal as the route for it and offers no typed action; that half
is the host's, and its own tests hold it.

The tests also hold what this repository has reviewed and signs to lists in
`pipeline/tests/packages.rs`, and a package that is not on the list for its kind fails until a
reviewer writes it there:

- `BUNDLED_CONNECTORS` and `BUNDLED_AGENTS` (the agents they serve): a package that carries a
  connector table and is not on them fails
  `a_connector_names_one_protocol_and_identifies_its_application_exactly` and
  `every_bundled_adapter_is_present_and_declares_no_surface_of_its_own`. A third test, `every_bundled_adapter_declares_what_it_observes_and_controls_and_answers_only_a_native_request`,
  counts six bundled agents, so a connector for a seventh application also needs that count changed.
- `REVIEWED_INTEGRATIONS`: every command integration, word for word (the command, the flags, the
  variables, the package's grant statement and the list the host renders from the declaration). A
  package that declares one also has to state the SDK range `>=0.1.3, <0.2.0` exactly.
- `REVIEWED_GRANTS`: the installation grant of every native bridge, word for word.
- `PROFILE_PAIRS`: every pair of profiles of one bundled agent, declared, and both profiles of a pair
  published by one publisher.
- Connector protocol names: no two packages may claim one named protocol, so a vendor's connector
  for an application a listed package already reads fails the test whatever list it is added to.
- A package without a connector table requests only capabilities inside the default ceiling and
  declares no mutation. A listed connector's actions are `presentation`, `upstream_cancel`,
  `upstream_method` (a method its own table routes) or `decision_destination` (an answer through
  the table's own destination), never a `component` action, and none writes into the terminal.

The lists and rules record what this repository has reviewed and signs; they give a listed package
no power that a vendor's package lacks, and they restrict what this repository signs beyond the host
contract: an installation grant cannot overcome a failing test, so a package that breaks one is not
published from here.
