use std::path::PathBuf;

use anyhow::Result;
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "pulsar-patch-tool", about = "Binary patch tool for Pulsar self-updater")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Generate a bsdiff patch between two binaries, compressed with zstd
    GeneratePatch {
        #[arg(long)]
        old_binary: PathBuf,
        #[arg(long)]
        new_binary: PathBuf,
        #[arg(long)]
        output: PathBuf,
    },
    /// Apply a bsdiff patch to a binary
    ApplyPatch {
        #[arg(long)]
        binary: PathBuf,
        #[arg(long)]
        patch: PathBuf,
        #[arg(long)]
        output: PathBuf,
    },
    /// Compute SHA256 hash of a file
    Sha256 {
        #[arg(long)]
        file: PathBuf,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::GeneratePatch {
            old_binary,
            new_binary,
            output,
        } => {
            let hash = pulsar_patch_tool::generate_patch(&old_binary, &new_binary, &output)?;
            println!("Patch created: {}", output.display());
            println!("SHA256: {}", hash);
        }
        Commands::ApplyPatch {
            binary,
            patch,
            output,
        } => {
            pulsar_patch_tool::apply_patch(&binary, &patch, &output)?;
            println!("Patched binary written: {}", output.display());
        }
        Commands::Sha256 { file } => {
            let hash = pulsar_patch_tool::sha256_file(&file)?;
            println!("{}", hash);
        }
    }

    Ok(())
}
