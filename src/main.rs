//! CLI for exercising the Entmoot MVP by hand against a JSON state file.
//! This is a manual-testing harness, not a service — there is no Discord or
//! Dione integration here. Every command reads the state file, applies one
//! operation, and writes it back.

use chrono::{Duration, Utc};
use clap::{Parser, Subcommand};
use entmoot::{Disposition, Message, Store};
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "entmoot", about = "Conversation-tree tracker MVP CLI")]
struct Cli {
    /// Path to the JSON state file.
    #[arg(long, default_value = "entmoot-state.json")]
    state: PathBuf,

    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Feed a message into a channel's branch (creates it if new).
    Water {
        channel_id: String,
        channel_name: String,
        message_id: u64,
        author: String,
        content: String,
    },
    /// Show the status of every known branch.
    Status,
    /// Surface branches idle longer than the threshold (minutes).
    Tend {
        #[arg(long, default_value_t = 10)]
        threshold_minutes: i64,
    },
    /// Prune a channel's branch up to (and including) a message id.
    Prune {
        channel_id: String,
        message_id: u64,
        #[arg(value_enum)]
        disposition: DispositionArg,
    },
    /// Prune a channel's branch up to its latest known message
    /// (`prune(channel) = prune(channel, latest_known_id)`).
    PruneLatest {
        channel_id: String,
        #[arg(value_enum)]
        disposition: DispositionArg,
    },
    /// Show prune history, optionally filtered to one channel.
    History { channel_id: Option<String> },
}

#[derive(Clone, clap::ValueEnum)]
enum DispositionArg {
    Done,
    Dropped,
    Parked,
}

impl From<DispositionArg> for Disposition {
    fn from(d: DispositionArg) -> Self {
        match d {
            DispositionArg::Done => Disposition::Done,
            DispositionArg::Dropped => Disposition::Dropped,
            DispositionArg::Parked => Disposition::Parked,
        }
    }
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    let mut store = Store::load(&cli.state)?;

    match cli.command {
        Command::Water {
            channel_id,
            channel_name,
            message_id,
            author,
            content,
        } => {
            store.water(
                channel_id,
                channel_name,
                Message::new(message_id, Utc::now(), author, content),
            );
            store.save(&cli.state)?;
            println!("watered.");
        }
        Command::Status => {
            let now = Utc::now();
            for s in store.status(now) {
                println!(
                    "[{}] {} — {} msgs, idle {}s, earliest: {:?}, latest: {:?}",
                    s.channel_id,
                    s.channel_name,
                    s.message_count,
                    s.staleness.num_seconds(),
                    s.earliest_unpruned.map(|m| m.content),
                    s.latest.map(|m| m.content)
                );
            }
        }
        Command::Tend { threshold_minutes } => {
            let now = Utc::now();
            let threshold = Duration::minutes(threshold_minutes);
            for t in store.tend(now, threshold) {
                println!(
                    "[{}] {} idle {}s — earliest unpruned: \"{}\" (from {})",
                    t.channel_id,
                    t.channel_name,
                    t.idle.num_seconds(),
                    t.earliest_unpruned.content,
                    t.earliest_unpruned.author
                );
            }
        }
        Command::Prune {
            channel_id,
            message_id,
            disposition,
        } => {
            let record = store.prune(&channel_id, message_id, disposition.into())?;
            store.save(&cli.state)?;
            println!(
                "pruned {} up to {} as {}",
                record.channel_id, record.message_id, record.disposition
            );
        }
        Command::PruneLatest {
            channel_id,
            disposition,
        } => {
            let record = store.prune_latest(&channel_id, disposition.into())?;
            store.save(&cli.state)?;
            println!(
                "pruned {} up to {} as {}",
                record.channel_id, record.message_id, record.disposition
            );
        }
        Command::History { channel_id } => {
            let records: Vec<_> = match &channel_id {
                Some(id) => store.history_for(id).into_iter().cloned().collect(),
                None => store.history().to_vec(),
            };
            for r in records {
                println!(
                    "[{}] up to {} — {} at {}",
                    r.channel_id, r.message_id, r.disposition, r.pruned_at
                );
            }
        }
    }

    Ok(())
}
