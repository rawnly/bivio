mod cli;
mod config;
mod git;
mod project;
mod shell;
mod storage;
mod utils;
mod version_check;
use std::io::{stdout, IsTerminal};

use anyhow::Result;
use clap::Parser;
use cli::{Cli, Command};
use project::Project;
use storage::Storage;

fn is_piped() -> bool {
    !stdout().is_terminal()
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    let check_process = tokio::spawn(async { version_check::is_update_available().await });

    if cli.debug {
        utils::enable_debug();
    }

    match cli.command.clone() {
        Command::Add { path, name, tags } => cli::commands::add(path, name, tags),
        Command::List { tags, json, limit } => cli::commands::list(tags, limit, json),
        Command::Pick {
            tags,
            query,
            show_broken,
        } => cli::commands::pick(cli::commands::PickOptions {
            query,
            show_broken,
            tags,
        }),
        Command::Remove { name, all, tags } => cli::commands::remove(name, tags, all),
        Command::Tag {
            project,
            tags,
            remove,
        } => cli::commands::tag(project, tags, remove),
        Command::Config { action } => cli::commands::config(action),
        Command::CheckUpdate => {
            if let Some(v) = version_check::is_update_available().await? {
                println!("A new version is available: {v}")
            } else {
                println!("Congrats! You're on the latest available version.")
            }

            Ok(())
        }
        Command::Init { shell } => cmd_init(shell),
    }?;

    if !is_piped() && matches!(cli.command, Command::Pick { .. }) {
        if let Some(latest) = check_process.await?? {
            println!();
            println!("A new update is available: {latest}");
            println!("Please update via: `brew update bivio`");
            println!();
        }
    }

    Ok(())
}

fn cmd_init(shell: Option<config::Shell>) -> Result<()> {
    let resolved_shell = shell
        .or_else(shell::detect_shell)
        .unwrap_or(config::Shell::Zsh);
    let hook = shell::generate_hook(resolved_shell);
    println!("{}", hook);
    Ok(())
}
