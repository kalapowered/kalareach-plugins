# Catalogue generations

A generation is one immutable catalogue release: a trust root, the TUF metadata over it, and every
target the metadata pins.

```text
<generation>/
  root.json          the trust root a host ships or adopts out of band
  metadata/          root, timestamp, snapshot and targets metadata
  targets/           index.json and every package payload
```

Generations are build output. `kalareach-catalogue build --out <dir>` produces one, and the inputs
that produce it, the packages and the publisher records, are what this repository keeps under
version control.

## The development generation

`development/` is the exception. It is a complete, really signed generation committed as a fixture,
so the core repository's catalogue sync has something to verify against without a signing
environment.

Its keys are development keys, and nothing else about it should be read as a release:

- The root and the metadata are signed with Ed25519 keys derived from a published phrase, which
  `docs/pipeline.md` gives. Anybody can derive them. The signatures show that the pipeline works and
  say nothing about who produced the packages.
- Its metadata expires in 2046 rather than in thirty days, so the fixture keeps verifying. A real
  generation's timestamp metadata expires quickly on purpose: a short window is what limits a replay
  attack.
- Its index carries a fixed `produced_at`, and each signed role is written in a fixed order, so
  rebuilding it from the same packages produces the same bytes.

Rebuild it after changing a package. `--replace` is what writes over a generation that is already
there; without it a build refuses the destination, because a generation is written once.

```bash
cargo run -p kalareach-catalogue -- build \
  --development \
  --out snapshots/development \
  --generation 2 \
  --produced-at 1760000000000 \
  --expires-at 2046-10-01T00:00:00Z \
  --replace
```

A generation after another is checked against it with `check-generation`, which says whether its
roots continue the previous generation's.

The new generation is assembled beside its destination and moved into place once it verifies there,
so a build that fails leaves the previous one intact. The generation it replaced is moved beside it
under a dotted name and left there; the build prints where. Removing it is yours to decide, because
a directory the pipeline checked before a build is not necessarily the same directory afterwards.

A production generation is signed in the signing environment, with keys that were generated there
and never left it, and is published rather than committed.
