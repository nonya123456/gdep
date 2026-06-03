use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(
    name = "gdep",
    version,
    about = "Godot dependency manager — Cargo-style addon management for Godot projects"
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Create gdep.toml in the current directory
    Init,

    /// Add an addon entry to gdep.toml
    Add {
        /// Git repository URL
        url: String,
        /// Name override (defaults to last path segment of URL)
        #[arg(long, short)]
        name: Option<String>,
        /// Pin to a specific tag
        #[arg(long)]
        tag: Option<String>,
        /// Track a branch (updated via `gdep update`)
        #[arg(long)]
        branch: Option<String>,
        /// Pin to an exact commit SHA
        #[arg(long)]
        commit: Option<String>,
        /// Subdirectory within the repo containing the addon
        #[arg(long)]
        subdir: Option<String>,
    },

    /// Install all addons (resolves manifest → lockfile if needed)
    Install,

    /// Re-fetch and update branch-pinned addons
    Update {
        /// Update only this addon (default: all branch-pinned addons)
        name: Option<String>,
    },

    /// Remove an addon from the manifest and delete its files
    Remove {
        /// Addon name as it appears in gdep.toml
        name: String,
    },

    /// List installed addons and their pinned commits
    List,
}
