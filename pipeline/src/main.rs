//! The KalaReach catalogue command.
//!
//! ```text
//! kalareach-catalogue validate            check every package, publisher record and build record
//! kalareach-catalogue root                write the trust root into the signing directory
//! kalareach-catalogue development-root    write the development lineage's version-1 root
//! kalareach-catalogue rotate-root         write the root that follows a root, signed under both
//! kalareach-catalogue build --out <dir>   build and sign one generation
//! kalareach-catalogue verify <dir>        verify a generation with the TUF client
//! kalareach-catalogue check-generation    accept a generation after its predecessor
//! kalareach-catalogue bench --entries N   measure a synthetic catalogue of N packages
//! ```
//!
//! Every command that signs first walks the working tree and refuses to run if a private key is
//! inside it.

use std::path::PathBuf;
use std::process::ExitCode;

use clap::{Args, Parser, Subcommand};
use jiff::{Span, Timestamp};
use kalareach_catalogue::development::DevelopmentSigning;
use kalareach_catalogue::keys::{SigningDirectory, SigningMaterial};
use kalareach_catalogue::packages::Repository;
use kalareach_catalogue::tuf::Expiries;
use kalareach_catalogue::{
    Error, Result, bench, fixtures, index, keys, packages, repository_root, tuf, write,
};
use kr_plugin_sdk::ids::RepositoryGeneration;
use kr_plugin_sdk::scalars::TimestampMs;

/// The KalaReach plugin catalogue.
#[derive(Debug, Parser)]
#[command(name = "kalareach-catalogue", version, about, long_about = None)]
struct Cli {
    /// The repository to work in. Defaults to the current directory or the first parent that
    /// holds both `plugins/` and `publishers/`.
    #[arg(long, global = true, value_name = "DIR")]
    repository: Option<PathBuf>,

    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Validate every package, publisher record and build record, and say which builds each
    /// release names.
    Validate,
    /// Write the trust root into the signing directory from the keys already there.
    Root(RootArgs),
    /// Write the development lineage's version-1 root, which anybody can derive.
    DevelopmentRoot(DevelopmentRootArgs),
    /// Write the root that follows a root, signed under both its keys and the next root's.
    RotateRoot(RotateArgs),
    /// Build and sign one catalogue generation.
    Build(BuildArgs),
    /// Verify a generation with the TUF client and read its index back.
    Verify(VerifyArgs),
    /// Accept a generation after the one it follows, or say why not.
    CheckGeneration(CheckArgs),
    /// Measure a synthetic catalogue.
    Bench(BenchArgs),
}

#[derive(Debug, Args)]
struct SigningArgs {
    /// The directory holding the signing keys. It must be outside the repository.
    #[arg(long, value_name = "DIR", env = "KALAREACH_SIGNING_DIR")]
    signing_dir: PathBuf,
}

#[derive(Debug, Args)]
struct RootArgs {
    #[command(flatten)]
    signing: SigningArgs,
    /// How many days the trust root is valid for.
    #[arg(long, default_value_t = 3650)]
    expires_in_days: i64,
}

#[derive(Debug, Args)]
struct DevelopmentRootArgs {
    /// The metadata directory to write `1.root.json` and `root.json` into.
    #[arg(long, value_name = "DIR")]
    out: PathBuf,
}

#[derive(Debug, Args)]
struct RotateArgs {
    /// Rotate the development lineage: the keys of the epoch after `--epoch` take over.
    #[arg(long, requires = "epoch", conflicts_with_all = ["signing_dir", "next_signing_dir"])]
    development: bool,
    /// The epoch whose keys sign the root being replaced (development only).
    #[arg(long)]
    epoch: Option<u32>,
    /// The directory holding the keys the root being replaced is signed with.
    #[arg(long, value_name = "DIR", requires = "next_signing_dir")]
    signing_dir: Option<PathBuf>,
    /// The directory holding the keys the new root names.
    #[arg(long, value_name = "DIR")]
    next_signing_dir: Option<PathBuf>,
    /// The directory holding the numbered roots; the new root is written into it.
    #[arg(long, value_name = "DIR")]
    roots_dir: PathBuf,
    /// When the new root expires, as an RFC 3339 time.
    #[arg(long, value_name = "TIME")]
    expires_at: Option<String>,
}

#[derive(Debug, Args)]
struct CheckArgs {
    /// The generation to accept.
    #[arg(value_name = "DIR")]
    generation: PathBuf,
    /// The generation it follows.
    #[arg(long, value_name = "DIR")]
    previous: PathBuf,
}

#[derive(Debug, Args)]
struct BuildArgs {
    /// The directory holding the signing keys. It must be outside the repository.
    #[arg(
        long,
        value_name = "DIR",
        env = "KALAREACH_SIGNING_DIR",
        required_unless_present = "development"
    )]
    signing_dir: Option<PathBuf>,
    /// Sign with the development keys, which anybody can derive, over the development lineage.
    #[arg(long, conflicts_with = "signing_dir")]
    development: bool,
    /// The epoch of the development keys that sign (with `--development`).
    #[arg(long, default_value_t = 0, requires = "development")]
    epoch: u32,
    /// The directory holding the development lineage's numbered roots, where it has rotated
    /// (with `--development`); the version-1 root is the lineage otherwise.
    #[arg(long, value_name = "DIR", requires = "development")]
    roots_dir: Option<PathBuf>,
    /// When the targets, snapshot and timestamp metadata expire, as an RFC 3339 time, so a build
    /// can be reproduced exactly. Overrides `--expires-in-days`.
    #[arg(long, value_name = "TIME")]
    expires_at: Option<String>,
    /// Where to write the generation.
    #[arg(long, value_name = "DIR")]
    out: PathBuf,
    /// The generation number this build is.
    #[arg(long, default_value_t = 1)]
    generation: u64,
    /// The index timestamp in milliseconds, so a build can be reproduced exactly.
    #[arg(long, value_name = "MS")]
    produced_at: Option<u64>,
    /// How many days the targets, snapshot and timestamp metadata are valid for.
    #[arg(long, default_value_t = 30)]
    expires_in_days: i64,
    /// Replace a generation that is already in the output directory.
    #[arg(long)]
    replace: bool,
}

#[derive(Debug, Args)]
struct VerifyArgs {
    /// The generation directory to verify.
    #[arg(value_name = "DIR")]
    generation: PathBuf,
    /// Read the generation even though its metadata has expired.
    #[arg(long)]
    ignore_expiry: bool,
}

#[derive(Debug, Args)]
struct BenchArgs {
    /// How many synthetic entries to build.
    #[arg(long, default_value_t = 10_000)]
    entries: usize,
    /// Where to write the measurement.
    #[arg(long, value_name = "FILE")]
    out: Option<PathBuf>,
}

#[tokio::main]
async fn main() -> ExitCode {
    let cli = Cli::parse();
    match run(cli).await {
        Ok(code) => code,
        Err(error) => {
            eprintln!("{error}");
            ExitCode::FAILURE
        }
    }
}

async fn run(cli: Cli) -> Result<ExitCode> {
    let start = cli.repository.clone().unwrap_or_else(|| PathBuf::from("."));
    let root = repository_root(&start)?;
    match cli.command {
        Command::Validate => validate(&root).map(|_| ExitCode::SUCCESS),
        Command::Root(args) => write_root(&root, &args).await.map(|()| ExitCode::SUCCESS),
        Command::DevelopmentRoot(args) => development_root(&args).await.map(|()| ExitCode::SUCCESS),
        Command::RotateRoot(args) => rotate_root(&root, &args).await.map(|()| ExitCode::SUCCESS),
        Command::Build(args) => build(&root, &args).await.map(|()| ExitCode::SUCCESS),
        Command::CheckGeneration(args) => check_generation(&args).await.map(|()| ExitCode::SUCCESS),
        Command::Verify(args) => verify(&args).await.map(|()| ExitCode::SUCCESS),
        Command::Bench(args) => bench(&root, &args).map(|()| ExitCode::SUCCESS),
    }
}

fn validate(root: &std::path::Path) -> Result<Repository> {
    let loaded = packages::load(root)?;
    for rejected in &loaded.rejected {
        eprintln!("{}:", rejected.relative);
        for finding in &rejected.report.findings {
            eprintln!("  {finding}");
        }
    }
    if !loaded.rejected.is_empty() {
        return Err(Error::InvalidPackages {
            count: loaded.rejected.len(),
        });
    }

    let mut cases = 0usize;
    for loaded_package in &loaded.repository.packages {
        cases += fixtures::check(
            &loaded_package.relative,
            &loaded_package.directory,
            &loaded_package.package,
        )?;
    }
    println!(
        "{} publisher(s), {} package(s), {cases} fixture case(s): valid",
        loaded.repository.publishers.len(),
        loaded.repository.packages.len()
    );
    let builds = &loaded.repository.builds;
    let named: Vec<_> = builds.named().collect();
    println!(
        "{} build(s) named, {} pinned build(s) left out",
        named.len(),
        builds.omitted().len()
    );
    for (plugin_id, release, build) in named {
        println!(
            "  named for {plugin_id} {release}: {} {} from {} on {} {}, executable {}",
            build.application,
            build.version,
            build.distribution,
            build.os.as_str(),
            build.architecture.as_str(),
            build.executable_digest
        );
    }
    for omitted in builds.omitted() {
        println!("  left out: {omitted}");
    }
    Ok(loaded.repository)
}

async fn write_root(root: &std::path::Path, args: &RootArgs) -> Result<()> {
    keys::refuse_keys_in_tree(root)?;
    let signing = SigningDirectory::open(&args.signing.signing_dir, root)?;
    let expires = expires_in(args.expires_in_days)?;
    let signed = keys::build_root(&signing, expires).await?;
    write(&signing.root_path(), signed.buffer())?;
    println!(
        "wrote {} (expires {expires})",
        signing.root_path().display()
    );
    Ok(())
}

async fn build(root: &std::path::Path, args: &BuildArgs) -> Result<()> {
    keys::refuse_keys_in_tree(root)?;
    let repository = validate(root)?;
    let signing: Box<dyn SigningMaterial> = if args.development {
        Box::new(match &args.roots_dir {
            Some(directory) => DevelopmentSigning::from_directory(args.epoch, directory)?,
            None => DevelopmentSigning::first().await?,
        })
    } else {
        let directory = args.signing_dir.as_ref().ok_or_else(|| Error::Signing {
            detail: "--signing-dir or --development names what signs".to_owned(),
        })?;
        Box::new(SigningDirectory::open(directory, root)?)
    };

    let produced_at = TimestampMs::new(args.produced_at.unwrap_or_else(now_ms));
    let generation = RepositoryGeneration::new(args.generation);
    let catalogue = index::build(&repository, generation, produced_at);

    let expires = match &args.expires_at {
        Some(time) => parse_time(time)?,
        None => expires_in(args.expires_in_days)?,
    };
    let outcome = tuf::build(
        &repository,
        &catalogue,
        signing.as_ref(),
        Expiries {
            targets: expires,
            snapshot: expires,
            timestamp: expires,
        },
        &args.out,
        args.replace,
    )
    .await?;

    println!(
        "generation {} -> {}: {} entries, {} targets, metadata expires {expires}",
        args.generation,
        args.out.display(),
        catalogue.entries.len(),
        outcome.target_count
    );
    if let Some(retired) = &outcome.retired {
        println!(
            "the generation it replaced is at {}; remove it when you no longer want it",
            retired.display()
        );
    }
    Ok(())
}

async fn development_root(args: &DevelopmentRootArgs) -> Result<()> {
    let bytes = kalareach_catalogue::development::version_one_root_bytes().await?;
    write(&args.out.join("1.root.json"), &bytes)?;
    write(&args.out.join("root.json"), &bytes)?;
    println!("wrote {}", args.out.join("1.root.json").display());
    Ok(())
}

async fn rotate_root(root: &std::path::Path, args: &RotateArgs) -> Result<()> {
    let roots = kalareach_catalogue::lineage::numbered_roots(&args.roots_dir)?;
    let (&highest, bytes) = roots.iter().next_back().ok_or_else(|| Error::Layout {
        detail: format!("{} holds no numbered root", args.roots_dir.display()),
    })?;
    let previous: tough::schema::Signed<tough::schema::Root> = serde_json::from_slice(bytes)
        .map_err(|source| Error::Json {
            path: format!("{highest}.root.json").into(),
            source,
        })?;
    let expires = match &args.expires_at {
        Some(time) => parse_time(time)?,
        None => parse_time(kalareach_catalogue::development::ROOT_EXPIRES)?,
    };
    let (previous_sources, next) = if args.development {
        let epoch = args.epoch.ok_or_else(|| Error::Signing {
            detail: "--epoch names the keys of the root being replaced".to_owned(),
        })?;
        (
            kalareach_catalogue::development::key_sources(epoch),
            kalareach_catalogue::development::role_sources(epoch + 1),
        )
    } else {
        keys::refuse_keys_in_tree(root)?;
        let current = args.signing_dir.as_ref().ok_or_else(|| Error::Signing {
            detail: "--signing-dir or --development names what signs".to_owned(),
        })?;
        let next = args
            .next_signing_dir
            .as_ref()
            .ok_or_else(|| Error::Signing {
                detail: "--next-signing-dir names the keys the new root names".to_owned(),
            })?;
        (
            SigningDirectory::open(current, root)?.key_sources()?,
            SigningDirectory::open(next, root)?.role_sources(),
        )
    };
    let signed = keys::rotate_root(&previous, &previous_sources, next, expires).await?;
    let version = signed.signed.version;
    let rendered = kalareach_catalogue::canonical::canonical(&signed)?;
    write(
        &args.roots_dir.join(format!("{version}.root.json")),
        &rendered,
    )?;
    write(&args.roots_dir.join("root.json"), &rendered)?;
    println!(
        "wrote {}",
        args.roots_dir
            .join(format!("{version}.root.json"))
            .display()
    );
    Ok(())
}

async fn check_generation(args: &CheckArgs) -> Result<()> {
    let lineage =
        kalareach_catalogue::lineage::check_generation(&args.generation, &args.previous).await?;
    println!(
        "{} follows {}: {}",
        args.generation.display(),
        args.previous.display(),
        match lineage {
            kalareach_catalogue::lineage::Lineage::Same => "the same root".to_owned(),
            kalareach_catalogue::lineage::Lineage::Chain { from, to } =>
                format!("the roots go on from version {from} to {to}"),
            kalareach_catalogue::lineage::Lineage::Break =>
                "the development lineage's one break".to_owned(),
        }
    );
    Ok(())
}

fn parse_time(time: &str) -> Result<Timestamp> {
    time.parse().map_err(|error| Error::Layout {
        detail: format!("{time} is not an RFC 3339 time: {error}"),
    })
}

async fn verify(args: &VerifyArgs) -> Result<()> {
    let verified = tuf::verify(&args.generation, !args.ignore_expiry).await?;
    println!(
        "{}: verified, {} targets, index generation {} with {} entries",
        args.generation.display(),
        verified.target_count,
        verified.index.generation,
        verified.index.entries.len()
    );
    Ok(())
}

fn bench(root: &std::path::Path, args: &BenchArgs) -> Result<()> {
    let loaded = packages::load(root)?;
    let template = loaded
        .repository
        .packages
        .iter()
        .find(|package| package.package.manifest.plugin_name.as_str() == "example-declarative")
        .or_else(|| loaded.repository.packages.first())
        .ok_or_else(|| Error::Layout {
            detail: "the repository holds no package to build synthetic entries from".to_owned(),
        })?;
    let measurement = bench::measure(&template.package.manifest, args.entries);
    let report = measurement.report();
    print!("{report}");
    if let Some(path) = &args.out {
        write(path, report.as_bytes())?;
        println!("wrote {}", path.display());
    }
    Ok(())
}

fn expires_in(days: i64) -> Result<Timestamp> {
    // A timestamp carries no calendar, so an expiry is expressed in hours.
    Timestamp::now()
        .checked_add(Span::new().hours(days.saturating_mul(24)))
        .map_err(|error| Error::Layout {
            detail: format!("{days} days is not a usable expiry: {error}"),
        })
}

fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|elapsed| elapsed.as_millis() as u64)
        .unwrap_or_default()
}
