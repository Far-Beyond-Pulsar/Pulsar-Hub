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
    /// Generate an ed25519 keypair for update-manifest signing.
    ///
    /// Prints the 32-byte seed (private key) as hex ONCE — store it in a
    /// secret store such as the UPDATE_SIGNING_KEY_SEED GitHub Actions secret —
    /// followed by the 32-byte public key hex to pin in the updater binary.
    GenKeypair,
    /// Sign an update manifest with an ed25519 private key (32-byte seed, hex).
    ///
    /// Writes a detached signature to <manifest>.sig, hex-encoded (64 raw
    /// bytes -> 128 hex chars). The updater verifies this against its pinned
    /// public key before trusting the manifest.
    SignManifest {
        #[arg(long)]
        manifest: PathBuf,
        #[arg(long)]
        key: String,
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
        Commands::GenKeypair => {
            let (seed, pubkey) = pulsar_patch_tool::generate_keypair()?;
            println!("Signing key seed (PRIVATE — store as UPDATE_SIGNING_KEY_SEED secret):");
            println!("{}", seed);
            println!("Public key (pin in updater binary):");
            println!("{}", pubkey);
        }
        Commands::SignManifest { manifest, key } => {
            let sig_path = pulsar_patch_tool::sign_file(&manifest, &key)?;
            println!("Signature written: {}", sig_path);
        }
    }

    Ok(())
}
