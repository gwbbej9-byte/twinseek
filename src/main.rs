//! twinseek CLI — scan a tree or open the two-pane TUI.

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use std::path::PathBuf;
use twinseek::scan::{print_human, scan_dir};
use twinseek::tui;

#[derive(Parser)]
#[command(
    name = "twinseek",
    version,
    about = "SHA-256 duplicate file finder with a ratatui TUI"
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Walk DIR; size → 4 KiB partial → SHA-256; print identical-content groups
    Scan {
        dir: PathBuf,
        /// Pretty JSON report (groups, hashes, wasted bytes)
        #[arg(long)]
        json: bool,
        /// Skip files smaller than N bytes (default 1 — empty files ignored)
        #[arg(long, default_value_t = 1)]
        min_bytes: u64,
    },
    /// Two-pane TUI. `d` marks; files are never deleted.
    Tui { dir: PathBuf },
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Commands::Scan {
            dir,
            json,
            min_bytes,
        } => {
            let report =
                scan_dir(&dir, min_bytes).with_context(|| format!("scan {}", dir.display()))?;
            if json {
                serde_json::to_writer_pretty(std::io::stdout(), &report)?;
                println!();
            } else {
                print_human(&report);
            }
        }
        Commands::Tui { dir } => {
            tui::run(&dir, 1).with_context(|| format!("tui {}", dir.display()))?
        }
    }
    Ok(())
}
