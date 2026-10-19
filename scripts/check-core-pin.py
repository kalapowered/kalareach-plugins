#!/usr/bin/env python3
"""Holds this repository's use of the core repository to one released pin and no path dependency.

    python3 scripts/check-core-pin.py check [--source <url or path>] [--releases <url>]
    python3 scripts/check-core-pin.py self-test

The catalogue is built against the plugin SDK the core repository publishes, and a catalogue that
built against a checkout on somebody's disk, or against a revision no release names, would sign
packages for a contract nobody else can reproduce. So this reads every tracked `Cargo.toml`,
`Cargo.lock` and Cargo configuration file and refuses:

  * a path dependency, in any dependency table or `[patch]` entry, and a configuration file's
    `paths` override. A path names a directory of the machine that built it. `[replace]`, a
    `[patch]` of the core repository's own source, and a configuration file's `[source]` table or
    `include` are refused outright: each moves what Cargo builds away from the pinned revision
    while the manifests and the lock file still name it;
  * a core dependency (a crate named `kr-*`, or one fetched from the core repository) that is not a
    Git dependency on that repository at a full 40-digit revision. A branch, a tag, an abbreviated
    revision and a registry version can each move or name something else;
  * more than one revision, and a lock file that is not tracked, that locks any package from the
    core repository (or any crate named `kr-*`, whatever its source) to another revision, or that
    holds a package from no source that no manifest here declares;
  * a revision that no package release of the core repository was packed from. A release is a tag
    `packages/v<version>+<first 12 digits of the commit>` on that very commit, and its published
    `SHA512SUMS` says which commit it was packed from, so both are read.

It reads the files Git tracks in this repository. A Cargo configuration outside it (a parent
directory, `CARGO_HOME`) is not read, and a hosted runner has none.

Reading the tags and the release needs the network, and a source that cannot be read is a failure,
never a pass. `--source` and `--releases` name another place to read them from, which is how
`self-test` runs without one.

`self-test` builds a core repository with its releases and a consumer of it, and plants each fault
in the consumer: the check must refuse every one and pass the pin as it should be.
"""

import argparse
import os
import re
import shutil
import subprocess
import sys
import tempfile
import textwrap
import tomllib
import urllib.error
import urllib.parse
import urllib.request

# The row this check carries, written in front of each self-test case.
KEY = "KR-REQ-04.18"

SCRIPT = os.path.abspath(__file__)
CORE_URL = "https://github.com/kalapowered/kalareach"
DEFAULT_SOURCE = CORE_URL + ".git"
DEFAULT_RELEASES = CORE_URL + "/releases/download"
REVISION = re.compile(r"\A[0-9a-f]{40}\Z")
RELEASE_TAG = re.compile(r"\Apackages/v[0-9]+\.[0-9]+\.[0-9]+(?:-[0-9A-Za-z.-]+)?\+([0-9a-f]{12})\Z")
DEPENDENCY_TABLES = ("dependencies", "dev-dependencies", "build-dependencies",
                     "dev_dependencies", "build_dependencies")
FETCH_SECONDS = 30


def same_repository(url):
    """Whether a Git URL names the core repository, in its https, ssh or scp-like form."""
    text = url.lower().strip().rstrip("/").removesuffix(".git")
    text = re.sub(r"\A(?:git\+)?(?:https?|ssh|git)://", "", text)
    text = text.removeprefix("git@").replace("github.com:", "github.com/", 1)
    text = text.removeprefix("www.")
    return text == CORE_URL.removeprefix("https://")


def tracked(root):
    done = subprocess.run(["git", "ls-files", "-z"], cwd=root, capture_output=True, check=True)
    return [os.fsdecode(path) for path in done.stdout.split(b"\0") if path]


def entries(document):
    """(table, name, specification) for every dependency a manifest or configuration file names,
    including the entries of `[patch]` and `[replace]`."""

    def tables(node, trail):
        for key, value in node.items():
            if not isinstance(value, dict):
                continue
            here = trail + [key]
            if key in DEPENDENCY_TABLES:
                yield ".".join(here), value
            elif key == "patch":
                for source, table in value.items():
                    if isinstance(table, dict):
                        yield f"patch.{source}", table
            elif key == "replace":
                continue
            else:
                yield from tables(value, here)

    for table, dependencies in tables(document, []):
        for name, specification in dependencies.items():
            yield table, name, specification


def read_pin(root, findings):
    """The revision every manifest pins the core repository at, or None."""
    files = tracked(root)
    manifests = [path for path in files if os.path.basename(path) == "Cargo.toml"]
    configs = [path for path in files if path.endswith((".cargo/config.toml", ".cargo/config"))]
    revisions = set()
    members = set()

    def load(path):
        try:
            with open(os.path.join(root, path), "rb") as handle:
                return tomllib.load(handle)
        except (OSError, tomllib.TOMLDecodeError) as problem:
            findings.append(f"{path} cannot be read: {problem}")
            return None

    for path in manifests + configs:
        document = load(path)
        if document is None:
            continue
        package = document.get("package")
        if isinstance(package, dict) and isinstance(package.get("name"), str):
            members.add(package["name"])
        if path in configs and "paths" in document:
            findings.append(f"{path} overrides dependencies with the directories in `paths`")
        if path in configs and "source" in document:
            findings.append(f"{path} replaces where Cargo fetches a source from, in `source`")
        if path in configs and "include" in document:
            findings.append(f"{path} includes other configuration files, which are not read")
        if "replace" in document:
            findings.append(f"{path} uses `[replace]`, which redirects a package away from its pin")
        patches = document.get("patch")
        if isinstance(patches, dict):
            for source in patches:
                if same_repository(source):
                    findings.append(f"{path} patches the core repository's own source")
        for table, name, specification in entries(document):
            where = f"{path}: {table}.{name}"
            if not isinstance(specification, dict):
                if name.startswith("kr-"):
                    findings.append(f"{where} is a registry version of a core crate, not a pinned revision")
                continue
            if "path" in specification:
                findings.append(f"{where} is a path dependency")
                continue
            crate = specification.get("package", name)
            git = specification.get("git")
            core = isinstance(git, str) and same_repository(git)
            if specification.get("workspace") is True:
                continue
            if not (core or (isinstance(crate, str) and crate.startswith("kr-"))):
                continue
            if not core:
                findings.append(f"{where} is a core crate that is not fetched from the core repository")
                continue
            if "branch" in specification or "tag" in specification:
                findings.append(f"{where} follows a branch or a tag; pin a revision")
                continue
            revision = specification.get("rev")
            if not isinstance(revision, str) or not REVISION.match(revision):
                findings.append(f"{where} does not name a full 40-digit revision")
                continue
            revisions.add(revision)

    if not revisions:
        findings.append("no manifest pins the core repository at a revision, so there is no pin to hold")
        return None, members, files
    if len(revisions) > 1:
        findings.append("the manifests pin more than one core revision: " + ", ".join(sorted(revisions)))
        return None, members, files
    return next(iter(revisions)), members, files


def check_lock(root, files, revision, members, findings):
    if "Cargo.lock" not in files:
        findings.append("Cargo.lock is not tracked, so the pin is not held for a build")
        return
    try:
        with open(os.path.join(root, "Cargo.lock"), "rb") as handle:
            lock = tomllib.load(handle)
    except (OSError, tomllib.TOMLDecodeError) as problem:
        findings.append(f"Cargo.lock cannot be read: {problem}")
        return
    from_core = 0
    for package in lock.get("package", []):
        name = package.get("name", "?")
        source = package.get("source")
        if source is None:
            if name not in members:
                findings.append(f"Cargo.lock holds {name} from no source, and no manifest here declares it")
            continue
        core = False
        if isinstance(source, str) and source.startswith("git+"):
            url, _, fragment = source[len("git+"):].partition("#")
            base, _, query = url.partition("?")
            core = same_repository(base)
        if core:
            from_core += 1
            if query != f"rev={revision}" or fragment != revision:
                findings.append(f"Cargo.lock locks {name} to {source}, not to the pinned revision")
        elif name.startswith("kr-"):
            findings.append(f"Cargo.lock locks the core crate {name} from {source}, not from the core repository")
    if from_core == 0:
        findings.append("Cargo.lock locks no package from the core repository")


def read_release(root, revision, source, releases, findings):
    """Holds the revision to a package release: a tag on that commit, and the release's own sums."""
    try:
        done = subprocess.run(
            ["git", "ls-remote", "--tags", source, "refs/tags/packages/v*"],
            cwd=root,
            capture_output=True,
            text=True,
            env={**os.environ, "GIT_TERMINAL_PROMPT": "0"},
            timeout=FETCH_SECONDS * 4,
        )
    except subprocess.TimeoutExpired:
        findings.append(f"the tags of {source} could not be read in time")
        return None
    if done.returncode != 0:
        findings.append(f"the tags of {source} could not be read: {done.stderr.strip()}")
        return None
    commits = {}
    for line in done.stdout.splitlines():
        sha, _, ref = line.partition("\t")
        name = ref.removeprefix("refs/tags/")
        peeled = name.endswith("^{}")
        name = name.removesuffix("^{}")
        if peeled or name not in commits:
            commits[name] = sha
    named = [
        name
        for name, sha in commits.items()
        if sha == revision and (match := RELEASE_TAG.match(name)) and match.group(1) == revision[:12]
    ]
    if not named:
        findings.append(f"{revision} is not the commit of any package release tag of the core repository")
        return None
    tag = sorted(named)[0]
    address = releases.rstrip("/") + "/" + "/".join(urllib.parse.quote(part, safe="") for part in tag.split("/"))
    address += "/SHA512SUMS"
    try:
        with urllib.request.urlopen(address, timeout=FETCH_SECONDS) as answer:
            sums = answer.read().decode("utf-8", "replace")
    except (OSError, urllib.error.URLError) as problem:
        findings.append(f"the release {tag} publishes no readable SHA512SUMS at {address}: {problem}")
        return None
    if f"packed from commit {revision}" not in sums or not re.search(
        rf"^# Release tag {re.escape(tag)}\.?$", sums, re.MULTILINE
    ):
        findings.append(f"the SHA512SUMS of {tag} does not say it was packed from {revision}")
        return None
    return tag


def run_check(source, releases):
    root = subprocess.run(
        ["git", "rev-parse", "--show-toplevel"], capture_output=True, text=True, check=True
    ).stdout.strip()
    print("core pin")
    findings = []
    revision, members, files = read_pin(root, findings)
    print(f"  pin        {revision or 'none'}")
    if revision is not None:
        check_lock(root, files, revision, members, findings)
        tag = read_release(root, revision, source, releases, findings)
        print(f"  release    {tag or 'none'}")
    if not findings:
        print("\nThe core repository is used at one released revision, through no path.")
        return 0
    print("\nThe pin is not held:")
    for finding in findings:
        print(f"  - {finding}")
    return 1


# ---------------------------------------------------------------------------------------------
# The self-test.

GIT_ENVIRONMENT = {
    "GIT_AUTHOR_NAME": "Check",
    "GIT_AUTHOR_EMAIL": "check@example.invalid",
    "GIT_COMMITTER_NAME": "Check",
    "GIT_COMMITTER_EMAIL": "check@example.invalid",
    "GIT_CONFIG_GLOBAL": os.devnull,
    "GIT_CONFIG_NOSYSTEM": "1",
}


def git(directory, *arguments):
    done = subprocess.run(
        ["git", "-c", "commit.gpgsign=false", "-c", "tag.gpgsign=false", *arguments],
        cwd=directory,
        env={**os.environ, **GIT_ENVIRONMENT},
        capture_output=True,
    )
    if done.returncode != 0:
        raise RuntimeError(f"git {' '.join(arguments)} failed: {done.stderr.decode().strip()}")
    return done.stdout.decode().strip()


def write(directory, path, content):
    full = os.path.join(directory, path)
    os.makedirs(os.path.dirname(full), exist_ok=True)
    with open(full, "w", encoding="utf-8") as handle:
        handle.write(content)


class World:
    """A core repository with two commits and a release of the first, and a place to read the
    release from."""

    def __init__(self, scratch):
        self.core = os.path.join(scratch, "core")
        os.makedirs(self.core)
        git(self.core, "init", "--quiet", "--initial-branch=main")
        write(self.core, "README.md", "core\n")
        git(self.core, "add", "-A")
        git(self.core, "commit", "--quiet", "-m", "One")
        self.released = git(self.core, "rev-parse", "HEAD")
        write(self.core, "README.md", "core, later\n")
        git(self.core, "add", "-A")
        git(self.core, "commit", "--quiet", "-m", "Two")
        self.unreleased = git(self.core, "rev-parse", "HEAD")
        self.tag = f"packages/v0.64.0+{self.released[:12]}"
        git(self.core, "tag", self.tag, self.released)
        self.releases = os.path.join(scratch, "releases")
        self.publish(self.tag, self.released)

    def publish(self, tag, commit):
        write(
            self.releases,
            os.path.join(*tag.split("/"), "SHA512SUMS"),
            f"# The generated packages at version 0.64.0, packed from commit {commit}.\n"
            f"# Release tag {tag}.\n#\n",
        )

    def releases_url(self):
        return "file://" + self.releases


MANIFEST = """[workspace]
members = ["pipeline"]

[workspace.dependencies]
kr-plugin-sdk = {{ git = "{core}.git", rev = "{revision}" }}
serde = "=1.0.0"
"""

MEMBER = """[package]
name = "pipeline"
version = "0.1.0"
edition = "2024"

[dependencies]
kr-plugin-sdk.workspace = true
serde.workspace = true
"""

LOCK = """version = 4

[[package]]
name = "pipeline"
version = "0.1.0"
dependencies = ["kr-plugin-sdk"]

[[package]]
name = "kr-plugin-sdk"
version = "0.1.0"
source = "git+{core}.git?rev={revision}#{revision}"
"""


class Consumer:
    made = 0

    def __init__(self, scratch, world, revision=None):
        Consumer.made += 1
        self.world = world
        self.revision = revision or world.released
        self.directory = os.path.join(scratch, f"consumer-{Consumer.made}")
        os.makedirs(self.directory)
        git(self.directory, "init", "--quiet", "--initial-branch=main")
        self.put("Cargo.toml", MANIFEST.format(core=CORE_URL, revision=self.revision))
        self.put("pipeline/Cargo.toml", MEMBER)
        self.put("Cargo.lock", LOCK.format(core=CORE_URL, revision=self.revision))

    def put(self, path, content):
        write(self.directory, path, content)

    def edit(self, path, old, new):
        full = os.path.join(self.directory, path)
        with open(full, encoding="utf-8") as handle:
            text = handle.read()
        assert old in text, (path, old)
        self.put(path, text.replace(old, new))

    def run(self, source=None):
        git(self.directory, "add", "-A")
        done = subprocess.run(
            [
                sys.executable, SCRIPT, "check",
                "--source", source or self.world.core,
                "--releases", self.world.releases_url(),
            ],
            cwd=self.directory,
            capture_output=True,
            text=True,
        )
        return done.returncode, done.stdout + done.stderr


def self_test():
    scratch = tempfile.mkdtemp(prefix="kalareach-pin-")
    results = []

    def expect(name, condition, detail=""):
        print(("ok    " if condition else "FAIL  ") + f"{KEY}: {name}")
        if not condition and detail:
            print(textwrap.indent(detail.rstrip(), "      "))
        results.append(bool(condition))

    try:
        cases(scratch, expect)
    finally:
        # Only a directory this run made, under the directory it made it in.
        if scratch.startswith(os.path.join(tempfile.gettempdir(), "kalareach-pin-")):
            shutil.rmtree(scratch, ignore_errors=True)
    failed = results.count(False)
    print(f"{len(results) - failed} of {len(results)} cases hold")
    return 1 if failed else 0


def cases(scratch, expect):
    world = World(scratch)
    pinned = world.released

    consumer = Consumer(scratch, world)
    status, output = consumer.run()
    expect("a pin on the commit a package release was packed from passes", status == 0, output)

    # An annotated tag names the tag object; its commit is read through it.
    other = World(os.path.join(scratch, "annotated"))
    git(other.core, "tag", "-d", other.tag)
    git(other.core, "tag", "-a", other.tag, other.released, "-m", "Release")
    consumer = Consumer(scratch, other)
    status, output = consumer.run()
    expect("an annotated release tag is read through to its commit", status == 0, output)

    for name, path, old, new in [
        ("in the workspace dependencies", "Cargo.toml",
         f'kr-plugin-sdk = {{ git = "{CORE_URL}.git", rev = "{pinned}" }}',
         'kr-plugin-sdk = { path = "../kalareach/crates/kr-plugin-sdk" }'),
        ("in a member's dependencies", "pipeline/Cargo.toml",
         "serde.workspace = true", 'serde = { path = "../vendor/serde" }'),
        ("in a member's dev-dependencies", "pipeline/Cargo.toml",
         "serde.workspace = true", 'serde.workspace = true\n\n[dev-dependencies]\nhelper = { path = "../helper" }'),
        ("in a platform's dependencies", "pipeline/Cargo.toml",
         "serde.workspace = true",
         'serde.workspace = true\n\n[target.\'cfg(unix)\'.dependencies]\nhelper = { path = "../helper" }'),
        ("in a patch", "Cargo.toml",
         'serde = "=1.0.0"\n', f'serde = "=1.0.0"\n\n[patch.crates-io]\nserde = {{ path = "../serde" }}\n'),
    ]:
        consumer = Consumer(scratch, world)
        consumer.edit(path, old, new)
        status, output = consumer.run()
        expect(
            f"a path dependency {name} is refused",
            status == 1 and "is a path dependency" in output,
            output,
        )

    consumer = Consumer(scratch, world)
    consumer.put(".cargo/config.toml", '[patch."https://github.com/kalapowered/kalareach.git"]\nkr-plugin-sdk = { path = "../kalareach/crates/kr-plugin-sdk" }\n')
    status, output = consumer.run()
    expect("a path patch in the Cargo configuration is refused", status == 1 and "is a path dependency" in output, output)

    consumer = Consumer(scratch, world)
    consumer.put(
        ".cargo/config.toml",
        '[source.core]\ngit = "https://github.com/kalapowered/kalareach.git"\n'
        f'rev = "{pinned}"\nreplace-with = "elsewhere"\n\n'
        '[source.elsewhere]\ngit = "https://example.invalid/fork.git"\nbranch = "main"\n',
    )
    status, output = consumer.run()
    expect("a source replacement in the Cargo configuration is refused", status == 1 and "replaces where Cargo fetches" in output, output)

    consumer = Consumer(scratch, world)
    consumer.put(".cargo/config.toml", 'include = "elsewhere.toml"\n')
    status, output = consumer.run()
    expect("an included Cargo configuration file, which is not read, is refused", status == 1 and "includes other configuration" in output, output)

    consumer = Consumer(scratch, world)
    consumer.edit("Cargo.toml", 'serde = "=1.0.0"\n', 'serde = "=1.0.0"\n\n[replace]\n"serde:1.0.0" = { git = "https://example.invalid/serde.git", branch = "main" }\n')
    status, output = consumer.run()
    expect("a [replace] entry is refused", status == 1 and "uses `[replace]`" in output, output)

    consumer = Consumer(scratch, world)
    consumer.edit(
        "Cargo.toml",
        'serde = "=1.0.0"\n',
        'serde = "=1.0.0"\n\n[patch."ssh://git@github.com/kalapowered/kalareach.git"]\n'
        'companion-platform = { git = "https://example.invalid/fork.git", branch = "main" }\n',
    )
    status, output = consumer.run()
    expect("a patch of the core repository's own source is refused, in any form of its URL", status == 1 and "patches the core repository's own source" in output, output)

    consumer = Consumer(scratch, world)
    consumer.edit("pipeline/Cargo.toml", "serde.workspace = true", 'serde.workspace = true\nsdk = { package = "kr-plugin-sdk", version = "0.1.0" }')
    status, output = consumer.run()
    expect("a core crate renamed on import is still a core crate", status == 1 and "not fetched from the core repository" in output, output)

    consumer = Consumer(scratch, world)
    consumer.put(".cargo/config.toml", 'paths = ["../kalareach"]\n')
    status, output = consumer.run()
    expect("a paths override in the Cargo configuration is refused", status == 1 and "`paths`" in output, output)

    for name, old, new, said in [
        ("a branch", f'rev = "{pinned}"', 'branch = "main"', "follows a branch or a tag"),
        ("a tag", f'rev = "{pinned}"', 'tag = "packages/v0.64.0"', "follows a branch or a tag"),
        ("an abbreviated revision", f'rev = "{pinned}"', f'rev = "{pinned[:12]}"', "full 40-digit"),
        ("no revision", f', rev = "{pinned}"', "", "full 40-digit"),
    ]:
        consumer = Consumer(scratch, world)
        consumer.edit("Cargo.toml", old, new)
        status, output = consumer.run()
        expect(
            f"{name} is not a pin",
            status == 1 and "kr-plugin-sdk" in output and said in output,
            output,
        )

    consumer = Consumer(scratch, world)
    consumer.edit("Cargo.toml", f'{{ git = "{CORE_URL}.git", rev = "{pinned}" }}', '"0.1.0"')
    status, output = consumer.run()
    expect("a core crate from a registry is refused", status == 1 and "registry version" in output, output)

    consumer = Consumer(scratch, world)
    consumer.edit("Cargo.toml", f"{CORE_URL}.git", "https://example.invalid/kalareach.git")
    status, output = consumer.run()
    expect("a core crate from another repository is refused", status == 1 and "not fetched from the core repository" in output, output)

    consumer = Consumer(scratch, world, revision=world.unreleased)
    status, output = consumer.run()
    expect(
        "a revision no package release was packed from is refused",
        status == 1 and "not the commit of any package release tag" in output,
        output,
    )

    # A tag that carries the first digits of a commit but sits on another one is no release of it.
    liar = World(os.path.join(scratch, "liar"))
    liar_tag = f"packages/v9.9.9+{liar.unreleased[:12]}"
    git(liar.core, "tag", liar_tag, liar.released)
    liar.publish(liar_tag, liar.unreleased)
    consumer = Consumer(scratch, liar, revision=liar.unreleased)
    status, output = consumer.run()
    expect(
        "a release tag on another commit does not release the revision, whatever its sums say",
        status == 1 and "not the commit of any package release tag" in output,
        output,
    )

    # A release-shaped tag whose digits are not the first of the commit it sits on.
    digits = World(os.path.join(scratch, "digits"))
    git(digits.core, "tag", "-d", digits.tag)
    git(digits.core, "tag", f"packages/v0.64.0+{'0' * 12}", digits.released)
    consumer = Consumer(scratch, digits)
    status, output = consumer.run()
    expect(
        "a release tag that carries other digits than its commit's is no release of it",
        status == 1 and "not the commit of any package release tag" in output,
        output,
    )

    unpublished = World(os.path.join(scratch, "unpublished"))
    shutil.rmtree(unpublished.releases)
    consumer = Consumer(scratch, unpublished)
    status, output = consumer.run()
    expect(
        "a tag whose release publishes no sums is refused",
        status == 1 and "publishes no readable SHA512SUMS" in output,
        output,
    )

    mislabelled = World(os.path.join(scratch, "mislabelled"))
    mislabelled.publish(mislabelled.tag, mislabelled.unreleased)
    consumer = Consumer(scratch, mislabelled)
    status, output = consumer.run()
    expect(
        "sums that say another commit was packed are refused",
        status == 1 and "does not say it was packed from" in output,
        output,
    )

    renamed = World(os.path.join(scratch, "renamed"))
    renamed.publish(renamed.tag, renamed.released)
    sums = os.path.join(renamed.releases, *renamed.tag.split("/"), "SHA512SUMS")
    with open(sums, "w", encoding="utf-8") as handle:
        handle.write(f"# The generated packages, packed from commit {renamed.released}.\n# Release tag packages/v9.9.9+{renamed.released[:12]}.\n")
    consumer = Consumer(scratch, renamed)
    status, output = consumer.run()
    expect(
        "sums that name the commit but another tag are refused",
        status == 1 and "does not say it was packed from" in output,
        output,
    )

    consumer = Consumer(scratch, world)
    consumer.edit("Cargo.lock", f"rev={pinned}#{pinned}", f"rev={world.unreleased}#{world.unreleased}")
    status, output = consumer.run()
    expect("a lock file that locks another revision is refused", status == 1 and "not to the pinned revision" in output, output)

    consumer = Consumer(scratch, world)
    consumer.edit("Cargo.lock", "[[package]]\nname = \"kr-plugin-sdk\"", "[[package]]\nname = \"helper\"\nversion = \"0.1.0\"\n\n[[package]]\nname = \"kr-plugin-sdk\"")
    status, output = consumer.run()
    expect("a lock file package from no source that no manifest declares is refused", status == 1 and "from no source" in output, output)

    consumer = Consumer(scratch, world)
    os.remove(os.path.join(consumer.directory, "Cargo.lock"))
    status, output = consumer.run()
    expect("a pin without a tracked lock file is refused", status == 1 and "Cargo.lock is not tracked" in output, output)

    for name, source in [
        ("a registry", "registry+https://github.com/rust-lang/crates.io-index"),
        ("a fork", "git+https://example.invalid/fork.git?branch=main#" + "0" * 40),
    ]:
        consumer = Consumer(scratch, world)
        consumer.edit(
            "Cargo.lock",
            '[[package]]\nname = "kr-plugin-sdk"',
            f'[[package]]\nname = "kr-transitive"\nversion = "0.1.0"\nsource = "{source}"\n\n[[package]]\nname = "kr-plugin-sdk"',
        )
        status, output = consumer.run()
        expect(
            f"a core crate that the lock file takes from {name} is refused",
            status == 1 and "locks the core crate kr-transitive" in output,
            output,
        )

    consumer = Consumer(scratch, world)
    consumer.edit("pipeline/Cargo.toml", "serde.workspace = true", f'serde.workspace = true\nkr-extra = {{ git = "{CORE_URL}.git", rev = "{world.unreleased}" }}')
    status, output = consumer.run()
    expect("two core revisions are refused", status == 1 and "more than one core revision" in output, output)

    consumer = Consumer(scratch, world)
    consumer.edit("Cargo.toml", f'kr-plugin-sdk = {{ git = "{CORE_URL}.git", rev = "{pinned}" }}\n', "")
    consumer.edit("pipeline/Cargo.toml", "kr-plugin-sdk.workspace = true\n", "")
    status, output = consumer.run()
    expect("a repository with no core dependency has no pin and is refused", status == 1 and "no pin to hold" in output, output)

    consumer = Consumer(scratch, world)
    status, output = consumer.run(source=os.path.join(scratch, "nowhere"))
    expect("tags that cannot be read are a failure and not a pass", status == 1 and "could not be read" in output, output)


# ---------------------------------------------------------------------------------------------


def main(argv):
    parser = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    commands = parser.add_subparsers(dest="command", required=True)
    check = commands.add_parser("check", help="hold the core dependency to a released pin")
    check.add_argument("--source", default=DEFAULT_SOURCE, help="where to read the core tags from")
    check.add_argument("--releases", default=DEFAULT_RELEASES, help="where to read a release from")
    commands.add_parser("self-test", help="prove each refusal on a planted fault")
    arguments = parser.parse_args(argv)
    if arguments.command == "self-test":
        return self_test()
    return run_check(arguments.source, arguments.releases)


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
