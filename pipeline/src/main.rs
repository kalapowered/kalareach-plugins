//! The KalaReach catalogue command.
//!
//! ```text
//! kalareach-catalogue validate            check every package and publisher record
//! kalareach-catalogue root                write the trust root into the signing directory
//! kalareach-catalogue build --out <dir>   build and sign one generation
//! kalareach-catalogue verify <dir>        verify a generation with the TUF client
//! kalareach-catalogue bench --entries N   measure a synthetic catalogue of N packages
//! ```
//!
//! Every command that signs first walks the working tree and refuses to run if a private key is
//! inside it.

use std::path::PathBuf;
use std::process::ExitCode;

use clap::{Args, Parser, Subcommand};
use jiff::{Span, Timestamp};
use kalareach_catalogue::keys::SigningDirectory;
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
    /// Validate every package and publisher record.
    Validate,
    /// Write the trust root into the signing directory from the keys already there.
    Root(RootArgs),
    /// Build and sign one catalogue generation.
    Build(BuildArgs),
    /// Verify a generation with the TUF client and read its index back.
    Verify(VerifyArgs),
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
struct BuildArgs {
    #[command(flatten)]
    signing: SigningArgs,
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
        Command::Build(args) => build(&root, &args).await.map(|()| ExitCode::SUCCESS),
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
    let signing = SigningDirectory::open(&args.signing.signing_dir, root)?;

    let produced_at = TimestampMs::new(args.produced_at.unwrap_or_else(now_ms));
    let generation = RepositoryGeneration::new(args.generation);
    let catalogue = index::build(&repository, generation, produced_at);

    let expires = expires_in(args.expires_in_days)?;
    let outcome = tuf::build(
        &repository,
        &catalogue,
        &signing,
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
