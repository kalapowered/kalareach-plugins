# Revocations

One file per withdrawn release, named after the release it withdraws:

```text
revocations/<publisher>-<plugin>-<version>.json
```

Each record names the publisher, the plugin, the version and the manifest digest of the release it
withdraws, plus the reason and a statement a person reads. The digest is what makes the record apply
to those exact bytes rather than to whatever is under that version later.

The builder reads this directory and writes each record into its index entry. Keeping the records
here, beside the packages, is what makes a rebuild produce the same index as the release it
rebuilds: a revocation that lived only inside a published generation would be lost the next time one
was built.

A revoked release stops new bindings. An active binding receives a warning and follows the
administrator's explicit disable policy.

`docs/publishing.md` has the file's shape.
