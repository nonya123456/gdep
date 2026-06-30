use anyhow::Result;
use clap::Parser;
use gdep::commands;

#[derive(Parser)]
#[command(name = "gdep", version, about = "Godot addon dependency manager")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(clap::Subcommand)]
enum Command {
    /// Install all addons from gdep.toml
    Install,
    /// Delete the local repo cache at ~/.cache/gdep/
    Clean,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Command::Install => commands::install(),
        Command::Clean => commands::clean(),
    }
}
