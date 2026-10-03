# Publishing a package

A package is a directory under `plugins/<publisher>/<plugin>/`. Its contents, its manifest fields
and the rules a host checks are the package contract, documented in the core repository at
`docs/plugins/README.md`. This covers what happens around that: how a package gets into this
catalogue, what a review pins, and how a package is withdrawn.

## What you need first

You need a publisher record: one JSON file at `publishers/<publisher>.json`, named after the
identifier it declares:

```json
{
  "record_version": 1,
  "id": "example-vendor",
  "display_name": "Example Vendor",
  "homepage": "https://example.test",
  "first_party": false,
  "contact": "https://example.test/contact"
}
```

The identifier is immutable. It becomes a directory name, a path segment in every target this
catalogue signs, and half of every plugin identifier you publish. Choosing a new one later means
publishing new packages, not renaming existing ones.

## Free and commercial packages

The catalogue lists free and commercial packages the same way, and KalaReach runs no payment
marketplace. A publisher record, a package manifest and a presentation document are closed
documents with no price, payment, licence-key, purchase or entitlement field, and `kr-plugin-sdk`'s
capability vocabulary has no capability that takes a payment, so a package cannot ask the host for
one and the catalogue cannot carry one. A
vendor may charge for its own hosted service or its support, and a plugin that checks an entitlement
of its own does so as its own business: that check is not a KalaReach revenue boundary, and nothing
in this catalogue or in the host enforces it.

## Packages that read terminal text

A package that needs terminal text instead of its application's own protocol has to ask for
`terminal.stream`, `terminal.transcript_tail` or `process.observe`, each of which sits outside a
repository's default ceiling and needs an explicit grant. No package in this repository asks for any
of them: each first-party package reads its application's native protocol through
`broker.semantic_events`, and only the Claude Code package asks to decode and answer that
application's approvals.

## Adding a package

Create `plugins/<publisher>/<plugin>/` with `plugin.json`, `presentation.json`, and whatever else
the manifest declares. Declare every file: a file the manifest does not name is a defect, because a
package whose contents differ from its manifest is a package whose hash does not mean what it says.

Run the following to validate the package before submitting a change:

```bash
cargo run -p kalareach-catalogue -- validate
```

The same check runs on every change here, and a host runs it again before it trusts the package.

### The source pin

Ensure that `plugin.json` contains the repository and revision from which the release was built:

```json
"source": {
  "repository": "https://github.com/example-vendor/example-agent-plugin",
  "revision": "8f0c6c2a1d4e5b7a9c3f2e1d0b8a7c6d5e4f3a2b"
}
```

The revision is a 40-character commit identifier, and it names a commit that holds the package's
source. A branch names whatever it points at now, and a tag can be moved to point somewhere else, so
neither is a pin. Work with whatever reference suits you and resolve it to a commit before
packaging; the pipeline refuses anything else.

Keep your source wherever you like. A reviewed catalogue entry pins the publisher, that revision and
the package digest, and a host installs the release by digest. It never runs a vendor repository's
current branch.

### Fixtures

A package with controls should carry a fixture file. It lists cases, each naming what the host knows
and which controls that case should show and enable:

```json
{
  "fixture_version": 1,
  "visibility": [
    {
      "name": "bound and idle",
      "context": {
        "rights": ["session.view"],
        "binding_state": "bound",
        "flags": [],
        "capabilities": [],
        "present_nodes": ["controls"]
      },
      "visible_controls": ["refresh"],
      "enabled_controls": ["refresh"]
    }
  ]
}
```

The pipeline evaluates your own predicates against each context and compares the result. A predicate
edit that changes what a person sees fails the build instead of shipping.

`plugins/kalareach/example-declarative/` is a complete working example.

## What a review looks at

A review starts with the manifest, because the manifest is what the host enforces. It does not stop
there. A connector table is a claim about how an upstream protocol behaves, and a native bridge is
code that runs under the application's own permissions: neither can be checked by reading the
document that declares it. A package that carries one is reviewed with its source and its
qualification fixtures beside the manifest.

**Capabilities and the reasons beside them.** Every capability outside the default ceiling needs a
reason a person will read while deciding whether to grant it. "Required for functionality" is not a
reason. Ask for what you use and nothing more: a capability you requested and never exercise is a
capability a reviewer has to take on trust.

**Effect classes.** Every action declares one, and the broker enforces it. An action labelled
`observe` that needs to send something upstream will fail at dispatch rather than silently working,
so declare the class the action actually has.

**Match rules.** An `exact` rule identifies the application by something that cannot be coincidence:
a bundle identifier, a package name in a registry. An `inferred` rule recognises an executable by
name and is presented to the user as a guess. Claiming `exact` for a name match is the one thing in
a manifest that is straightforwardly dishonest.

**Connector tables.** A `connector.json` is a semantic trust claim, not a configuration file. It
says: these methods observe, these mutate, these carry credentials, and this one is outside what I
can proxy safely. Each classification carries the evidence you qualified it against, and the table
is pinned to the protocol version you tested. A method you leave out is treated as a mutation, which
is the safe answer rather than a gap. Record the qualification itself in
`docs/qualification-notes.md`: what each identity and version was pinned to, which vendor schema
or document it came from with its URL and the date you read it, and whether you checked it
against a live install or left it unverified.

**Native bridges.** A bridge runs under the application's own permissions, outside the sandbox. The
recipe lists exact files, configuration edits, hashes, version requirements and the operations that
remove them again, and the installation grant states what it is. Prefer a small registration file
that forwards to the core hook over anything larger.

## Versions and revocation

A version is immutable. Publishing a fix means publishing a new version, because hosts pin packages
by hash and an active binding stays on the hash it bound to.

To withdraw a release, add a file under `revocations/`:

```json
{
  "record_version": 1,
  "publisher_id": "example-vendor",
  "plugin_name": "example-agent",
  "version": "1.4.0",
  "manifest_digest": "2f0b...",
  "record": {
    "reason": "vulnerable",
    "revoked_at": "1760000000000",
    "statement": "Replaced by 1.4.1, which fixes the path handling."
  }
}
```

The reason is `withdrawn`, `vulnerable`, `key_compromise` or `superseded`. The manifest digest names
the exact bytes being withdrawn, so the record applies to that release and not to whatever is under
that version later. The builder reads `revocations/` and writes the record into the index entry, so
a rebuild produces the same index as the release it rebuilds.

A revoked release stops new bindings. An active binding receives a warning and follows the
administrator's explicit disable policy; it does not change under a live request, because changing a
binding mid-request is how a person ends up approving something other than what they read.

The executable builds an index entry names are not part of the package either. They come from the
build list and the qualification records in `fixtures/agents`, and a build is named only when its
record shows every part of the qualification cases passed. A build is added by committing a record
that qualifies it, and withdrawn by removing its pin or by a newly measured record that no longer
qualifies; the next generation names it or leaves it out, and the release keeps its version and
digest.

## Delegation

A host verifies delegated targets roles beneath a repository's root, scoped by publisher path, with
snapshot pinning, terminating-role semantics, bounded depth and revocation. This repository's
pipeline does not produce them: it signs every package with the top-level targets role and refuses a
role that pins any other, so a vendor's packages are signed here, and the review above is what
stands behind them.

## What a signature does not mean

A signature says which publisher released these exact bytes. It does not say the package is safe,
that this host has permission to run it, or that a publisher's classification of an upstream method
is correct. An installed connector is a semantic trust boundary: sandboxing a decoder does not make
it truthful, which is why the publisher and the methods are recorded, and why granting a decoder is
a decision a person makes rather than a consequence of installation.
