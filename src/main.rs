//! CLI for exercising the Entmoot MVP by hand against a JSON state file.
//! This is a manual-testing harness, not a service — there is no Discord or
//! Dione integration here. Every command reads the state file, applies one
//! operation, and writes it back.

use chrono::{Duration, Utc};
use clap::{Parser, Subcommand};
use entmoot::{Author, ChannelId, ChannelName, Content, Disposition, Message, Store, DEFAULT_STALE_THRESHOLD};
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
        channel_id: ChannelId,
        channel_name: ChannelName,
        message_id: u64,
        author: Author,
        content: Content,
    },
    /// Show the status of every known branch.
    Status,
    /// Surface branches idle longer than the threshold (minutes).
    Tend {
        #[arg(long, default_value_t = DEFAULT_STALE_THRESHOLD.num_minutes())]
        threshold_minutes: i64,
    },
    /// Prune a channel's branch up to (and including) a message id.
    Prune {
        channel_id: ChannelId,
        message_id: u64,
        #[arg(value_enum)]
        disposition: DispositionArg,
    },
    /// Prune a channel's branch up to its latest known message
    /// (`prune(channel) = prune(channel, latest_known_id)`).
    PruneLatest {
        channel_id: ChannelId,
        #[arg(value_enum)]
        disposition: DispositionArg,
    },
    /// Show prune history, optionally filtered to one channel.
    History { channel_id: Option<ChannelId> },
    /// Show lag log — pipeline stage timestamps for post-hoc analysis.
    /// Optionally filtered to one channel or one message.
    Lag {
        /// Filter to a specific channel.
        #[arg(long)]
        channel: Option<ChannelId>,
        /// Filter to a specific message id.
        #[arg(long)]
        message: Option<u64>,
    },
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
            let message = Message::new(message_id, Utc::now(), author.as_str(), content.as_str())?;
            store.water(channel_id, channel_name, message);
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
                    s.earliest_unpruned.map(|m| m.content.to_string()),
                    s.latest.map(|m| m.content.to_string())
                );
            }
        }
        Command::Tend { threshold_minutes } => {
            let now = Utc::now();
            let threshold = Duration::try_minutes(threshold_minutes)
                .ok_or_else(|| anyhow::anyhow!("threshold-minutes value overflows"))?;
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
            let record = store.prune(channel_id, message_id, disposition.into(), Utc::now())?;
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
            let record = store.prune_latest(channel_id, disposition.into(), Utc::now())?;
            store.save(&cli.state)?;
            println!(
                "pruned {} up to {} as {}",
                record.channel_id, record.message_id, record.disposition
            );
        }
        Command::History { channel_id } => {
            let records: Vec<_> = match channel_id {
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
        Command::Lag { channel, message } => {
            let entries: Vec<_> = match (&channel, message) {
                (_, Some(msg_id)) => store.lag_log_for_message(msg_id).into_iter().cloned().collect(),
                (Some(ch), None) => store.lag_log_for(&ch.to_string()).into_iter().cloned().collect(),
                (None, None) => store.lag_log().to_vec(),
            };
            if entries.is_empty() {
                println!("no lag entries.");
            } else {
                for e in &entries {
                    println!(
                        "[{}] msg {} — {} at {}",
                        e.channel_id, e.message_id, e.stage, e.timestamp
                    );
                }
            }
        }
    }

    Ok(())
}
