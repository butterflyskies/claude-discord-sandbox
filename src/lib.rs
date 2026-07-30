//! Entmoot MVP: a conversation-tree tracker that sits atop Dione.
//!
//! Scope note: this is the MVP slice of the full Entmoot design described in
//! `DESIGN.md` (enrichment pipeline, queryable conversation graph, Rolodex
//! integration, fail-closed cross-channel visibility, etc). None of that is
//! implemented here. This crate implements exactly the MVP surface:
//!
//! - One branch per Discord channel, auto-created on first message ("water").
//! - A branch tracks: channel id, channel name, earliest unpruned message,
//!   latest message, and a lifetime message count.
//! - Staleness is computed on demand from `now - last_message_timestamp`.
//!   There is no tick cron; nothing polls. `status()` and `tend()` both take
//!   an explicit `now` so the whole crate stays a pure function of its input,
//!   which is what makes it trivially testable.
//! - `tend()` surfaces branches idle longer than a threshold, returning the
//!   EARLIEST unpruned message (FIFO) — the point being deferred, not the
//!   most recent chatter.
//! - `prune` always resolves to one code path: `prune(channel, message_id,
//!   disposition)`. The convenience `prune_latest(channel, disposition)` is
//!   sugar that reads the current latest known id and forwards to it — there
//!   is no second implementation to drift out of sync.
//! - Pruning requires a disposition: `Done`, `Dropped`, or `Parked`. History
//!   of prune decisions is retained and queryable.
//! - No humans, no sibling constructs, no external crons are threaded through
//!   this crate. It is exercised entirely through synthetic `Message` values
//!   injected by the caller (or the CLI binary).

use std::collections::VecDeque;
use std::io::Write;
use std::path::Path;

use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tempfile::NamedTempFile;

/// A single conversational event fed into Entmoot. In production this is
/// derived from a Dione message event; in tests it's constructed directly —
/// Entmoot never talks to Discord or Dione itself.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Message {
    /// Discord snowflake (or any monotonically increasing id in tests).
    /// Ordering by `id` is what makes "earliest"/"latest" well-defined even
    /// if timestamps collide.
    pub id: u64,
    pub timestamp: DateTime<Utc>,
    pub author: String,
    pub content: String,
}

impl Message {
    pub fn new(id: u64, timestamp: DateTime<Utc>, author: impl Into<String>, content: impl Into<String>) -> Self {
        Self {
            id,
            timestamp,
            author: author.into(),
            content: content.into(),
        }
    }
}

/// Why a branch was pruned. Required on every prune — there is no
/// dispositionless prune path.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Disposition {
    /// The branch's open loops were resolved.
    Done,
    /// The branch was deliberately abandoned.
    Dropped,
    /// The branch is intentionally deferred — "I'll come back to that."
    Parked,
}

impl std::fmt::Display for Disposition {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Disposition::Done => write!(f, "done"),
            Disposition::Dropped => write!(f, "dropped"),
            Disposition::Parked => write!(f, "parked"),
        }
    }
}

/// A record of one prune decision, retained in history after the messages
/// themselves are cleared. This is what makes disposition retrievable after
/// the fact (test 13).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PruneRecord {
    pub channel_id: String,
    /// The cursor: every unpruned message with id <= this was cleared.
    pub message_id: u64,
    pub disposition: Disposition,
    pub pruned_at: DateTime<Utc>,
}

/// One branch: the conversational state for a single Discord channel.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Branch {
    pub channel_id: String,
    pub channel_name: String,
    /// Unpruned messages, kept in ascending id order. `front()` is the
    /// earliest unpruned message; `back()` is the latest.
    messages: VecDeque<Message>,
    /// Lifetime count of messages ever received on this branch, including
    /// ones since pruned. Distinct from `messages.len()`, which is only the
    /// unpruned tail.
    pub message_count: u64,
    /// Timestamp of the most recent message received, pruned or not. This is
    /// what staleness is computed from — pruning does not reset the clock,
    /// only new activity does.
    pub last_activity: DateTime<Utc>,
    /// The highest message id ever seen on this branch, pruned or not. This
    /// is the "latest known id" that `prune_latest` reads.
    latest_known_id: u64,
}

impl Branch {
    fn new(channel_id: String, channel_name: String, first: Message) -> Self {
        let last_activity = first.timestamp;
        let latest_known_id = first.id;
        let mut messages = VecDeque::new();
        messages.push_back(first);
        Self {
            channel_id,
            channel_name,
            messages,
            message_count: 1,
            last_activity,
            latest_known_id,
        }
    }

    fn water(&mut self, msg: Message) {
        if msg.timestamp > self.last_activity {
            self.last_activity = msg.timestamp;
        }
        if msg.id > self.latest_known_id {
            self.latest_known_id = msg.id;
        }
        self.message_count += 1;
        self.messages.push_back(msg);
        // Keep ascending order even if messages arrive out of id order —
        // FIFO semantics for "earliest unpruned" depend on this.
        let slice = self.messages.make_contiguous();
        slice.sort_by_key(|m| m.id);
    }

    /// The earliest message that hasn't been pruned yet (FIFO head).
    pub fn earliest_unpruned(&self) -> Option<&Message> {
        self.messages.front()
    }

    /// The most recent message received on this branch.
    pub fn latest(&self) -> Option<&Message> {
        self.messages.back()
    }

    /// How long this branch has been idle, as of `now`.
    pub fn staleness(&self, now: DateTime<Utc>) -> Duration {
        now - self.last_activity
    }

    /// Clear every unpruned message with id <= `message_id`. Returns the
    /// number of messages cleared.
    fn prune_to(&mut self, message_id: u64) -> usize {
        let before = self.messages.len();
        self.messages.retain(|m| m.id > message_id);
        before - self.messages.len()
    }

    pub fn is_empty_of_unpruned(&self) -> bool {
        self.messages.is_empty()
    }
}

/// A point-in-time snapshot of a branch, as returned by `status()`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BranchStatus {
    pub channel_id: String,
    pub channel_name: String,
    pub message_count: u64,
    pub earliest_unpruned: Option<Message>,
    pub latest: Option<Message>,
    pub staleness: Duration,
}

/// One entry in `tend()`'s output: a branch that has gone idle longer than
/// the configured threshold, carrying the point that's waiting on it.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TendEntry {
    pub channel_id: String,
    pub channel_name: String,
    pub idle: Duration,
    /// The earliest unpruned message — the thing that's been waiting,
    /// FIFO, not the most recent chatter.
    pub earliest_unpruned: Message,
}

#[derive(Debug, thiserror::Error)]
pub enum EntmootError {
    #[error("no branch exists for channel {0}")]
    UnknownChannel(String),
    #[error("branch for channel {0} has no messages to prune")]
    NothingToPrune(String),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("serialization error: {0}")]
    Serde(#[from] serde_json::Error),
}

/// The points inside [`Store::save`] at which a crash would leave a different
/// mess behind. A real save runs straight through them; the durability tests
/// stop at one and assert the ledger on disk is still the old one, intact.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SavePhase {
    /// Part of the payload is in the temp file and the rest never arrives.
    PartiallyWritten,
    /// The temp file is complete and synced, but the swap hasn't happened.
    BeforeRename,
}

/// Default staleness threshold used by `tend()` when the caller doesn't
/// override it: 10 minutes.
pub const DEFAULT_STALE_THRESHOLD: Duration = Duration::minutes(10);

/// The Entmoot store: all branches, plus the retained prune history. This is
/// the entire persisted state of the MVP.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Store {
    branches: HashMap<String, Branch>,
    history: Vec<PruneRecord>,
}

impl Store {
    pub fn new() -> Self {
        Self::default()
    }

    /// Feed a message in. Auto-creates the branch on first message for a
    /// channel ("plant"); grows it otherwise ("water"). This is the only way
    /// branches come into existence — there's no separate explicit "create
    /// branch" call, matching "branches grow automatically on message
    /// receipt."
    pub fn water(&mut self, channel_id: impl Into<String>, channel_name: impl Into<String>, message: Message) {
        let channel_id = channel_id.into();
        match self.branches.get_mut(&channel_id) {
            Some(branch) => branch.water(message),
            None => {
                let branch = Branch::new(channel_id.clone(), channel_name.into(), message);
                self.branches.insert(channel_id, branch);
            }
        }
    }

    pub fn branch(&self, channel_id: &str) -> Option<&Branch> {
        self.branches.get(channel_id)
    }

    pub fn is_empty(&self) -> bool {
        self.branches.is_empty()
    }

    pub fn channel_count(&self) -> usize {
        self.branches.len()
    }

    /// Status of every known branch, in no particular guaranteed order
    /// (callers that need stable ordering should sort by `channel_id`).
    pub fn status(&self, now: DateTime<Utc>) -> Vec<BranchStatus> {
        let mut out: Vec<BranchStatus> = self
            .branches
            .values()
            .map(|b| BranchStatus {
                channel_id: b.channel_id.clone(),
                channel_name: b.channel_name.clone(),
                message_count: b.message_count,
                earliest_unpruned: b.earliest_unpruned().cloned(),
                latest: b.latest().cloned(),
                staleness: b.staleness(now),
            })
            .collect();
        out.sort_by(|a, b| a.channel_id.cmp(&b.channel_id));
        out
    }

    /// Branches idle longer than `threshold` as of `now`, each carrying its
    /// earliest unpruned message. Branches with no unpruned messages (fully
    /// pruned) are never surfaced — there's nothing left to tend.
    pub fn tend(&self, now: DateTime<Utc>, threshold: Duration) -> Vec<TendEntry> {
        let mut out: Vec<TendEntry> = self
            .branches
            .values()
            .filter(|b| b.staleness(now) >= threshold)
            .filter_map(|b| {
                b.earliest_unpruned().map(|m| TendEntry {
                    channel_id: b.channel_id.clone(),
                    channel_name: b.channel_name.clone(),
                    idle: b.staleness(now),
                    earliest_unpruned: m.clone(),
                })
            })
            .collect();
        // Most-idle first: the longest-waiting branch is the most urgent to tend.
        out.sort_by(|a, b| b.idle.cmp(&a.idle).then_with(|| a.channel_id.cmp(&b.channel_id)));
        out
    }

    /// The highest message id ever seen on a channel's branch, pruned or
    /// not. This is what `prune_latest` reads as its cursor.
    pub fn latest_known_id(&self, channel_id: &str) -> Option<u64> {
        self.branches.get(channel_id).map(|b| b.latest_known_id)
    }

    /// The one prune code path: clear every unpruned message on `channel_id`
    /// with id <= `message_id`, tag the operation with `disposition`, and
    /// retain a `PruneRecord` in history.
    ///
    /// Because the cursor (`message_id`) is an explicit argument rather than
    /// something this function re-reads at execution time, a message that
    /// arrives with a higher id between when the caller *decided* to prune
    /// and when this function actually *runs* is never touched — it has an
    /// id greater than the cursor, so `Branch::prune_to` leaves it alone.
    /// This is what makes prune race-safe: capture the cursor once, act on
    /// exactly that snapshot.
    pub fn prune(
        &mut self,
        channel_id: &str,
        message_id: u64,
        disposition: Disposition,
    ) -> Result<PruneRecord, EntmootError> {
        let branch = self
            .branches
            .get_mut(channel_id)
            .ok_or_else(|| EntmootError::UnknownChannel(channel_id.to_string()))?;
        branch.prune_to(message_id);
        let record = PruneRecord {
            channel_id: channel_id.to_string(),
            message_id,
            disposition,
            pruned_at: Utc::now(),
        };
        self.history.push(record.clone());
        Ok(record)
    }

    /// Convenience form: `prune(channel) = prune(channel, latest_known_id)`.
    /// Reads the branch's current latest known id and forwards to
    /// [`Store::prune`] — there is no separate implementation of the clearing
    /// logic here, only cursor resolution.
    pub fn prune_latest(&mut self, channel_id: &str, disposition: Disposition) -> Result<PruneRecord, EntmootError> {
        let latest = self
            .latest_known_id(channel_id)
            .ok_or_else(|| EntmootError::UnknownChannel(channel_id.to_string()))?;
        self.prune(channel_id, latest, disposition)
    }

    /// Full prune history, oldest first, across all channels.
    pub fn history(&self) -> &[PruneRecord] {
        &self.history
    }

    /// Prune history for one channel, oldest first.
    pub fn history_for(&self, channel_id: &str) -> Vec<&PruneRecord> {
        self.history.iter().filter(|r| r.channel_id == channel_id).collect()
    }

    /// Persist the whole store to `path` as JSON, atomically.
    ///
    /// A ledger that loses its own state is worse than no ledger, so this
    /// never writes into the live file. It writes a complete copy to a temp
    /// file beside it, flushes that copy to disk, and renames it over the
    /// target. A crash at any point leaves either the previous ledger or the
    /// new one on disk — never a truncated or half-written mixture.
    pub fn save(&self, path: impl AsRef<Path>) -> Result<(), EntmootError> {
        self.save_with_hook(path.as_ref(), |_| Ok(()))
    }

    /// The real save, with a hook run at each durability checkpoint.
    ///
    /// `save` passes a hook that does nothing. Tests pass one that fails at a
    /// chosen [`SavePhase`], which is how the crash windows this function
    /// exists to close are exercised without actually killing the process.
    fn save_with_hook(
        &self,
        path: &Path,
        mut hook: impl FnMut(SavePhase) -> Result<(), EntmootError>,
    ) -> Result<(), EntmootError> {
        let json = serde_json::to_vec_pretty(self)?;

        // The temp file must live in the target's own directory: `rename` is
        // only atomic within a filesystem, and a sibling path is the only
        // placement guaranteed to be on the same one.
        let dir = match path.parent() {
            Some(parent) if !parent.as_os_str().is_empty() => parent,
            _ => Path::new("."),
        };

        // NamedTempFile unlinks itself on drop, so every `?` below cleans up
        // its own temp file on the way out — including the hook's failures.
        let mut tmp = NamedTempFile::new_in(dir)?;

        // Written in two chunks so there is a real window in which the temp
        // file holds a partial payload — that is the state a crash would
        // leave behind, and the hook lets a test stop exactly there.
        let (head, tail) = json.split_at(json.len() / 2);
        tmp.write_all(head)?;
        hook(SavePhase::PartiallyWritten)?;
        tmp.write_all(tail)?;

        // Push the bytes out of the page cache before the rename. Without
        // this, the rename can be durable while the contents it points at are
        // not, which turns a crash into a zero-length ledger.
        tmp.as_file().sync_all()?;

        // Replacing an existing ledger keeps that ledger's permissions; a
        // brand new one keeps the temp file's restrictive default.
        match std::fs::metadata(path) {
            Ok(meta) => tmp.as_file().set_permissions(meta.permissions())?,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(e.into()),
        }

        hook(SavePhase::BeforeRename)?;

        // Atomic on POSIX: a concurrent reader sees the old ledger or the new
        // one, never an intermediate.
        tmp.persist(path).map_err(|e| EntmootError::Io(e.error))?;

        // The rename is a change to the *directory*, and that change is only
        // durable once the directory itself is synced. Skipping this can
        // resurrect the pre-save ledger after a power loss even though the
        // rename returned successfully.
        #[cfg(unix)]
        std::fs::File::open(dir)?.sync_all()?;

        Ok(())
    }

    /// Load a store from `path`. Returns an empty store if the file doesn't
    /// exist yet — a fresh Entmoot instance with nothing planted.
    pub fn load(path: impl AsRef<Path>) -> Result<Self, EntmootError> {
        let path = path.as_ref();
        if !path.exists() {
            return Ok(Self::default());
        }
        let bytes = std::fs::read(path)?;
        let store: Store = serde_json::from_slice(&bytes)?;
        Ok(store)
    }
}

/// Durability tests for [`Store::save`].
///
/// These live in-crate rather than in `tests/integration.rs` because they
/// drive `save_with_hook`, the private seam that stands in for a crash. The
/// alternative — exposing fault injection on the public API — would put a
/// test-only escape hatch in front of every consumer of the crate.
#[cfg(test)]
mod durability {
    use super::*;

    fn at(seconds: i64) -> DateTime<Utc> {
        DateTime::<Utc>::UNIX_EPOCH + Duration::seconds(seconds)
    }

    /// A ledger that is already on disk and already trusted.
    fn planted() -> Store {
        let mut store = Store::new();
        store.water("chan-1", "general", Message::new(1, at(0), "lina", "first"));
        store
    }

    /// A later state of that same ledger, mid-save.
    fn grown() -> Store {
        let mut store = planted();
        store.water("chan-1", "general", Message::new(2, at(10), "ari", "second"));
        store.prune("chan-1", 1, Disposition::Done).unwrap();
        store
    }

    fn crash_at(phase: SavePhase) -> impl FnMut(SavePhase) -> Result<(), EntmootError> {
        move |reached| {
            if reached == phase {
                Err(EntmootError::Io(std::io::Error::other("simulated crash")))
            } else {
                Ok(())
            }
        }
    }

    /// The invariant the whole atomic-save exercise exists for: a save that
    /// dies partway through must not damage the ledger that was already
    /// there. Checked at both crash windows — payload half-written, and
    /// payload complete but not yet swapped in.
    #[test]
    fn interrupted_save_leaves_the_previous_ledger_byte_identical() {
        for phase in [SavePhase::PartiallyWritten, SavePhase::BeforeRename] {
            let dir = tempfile::tempdir().unwrap();
            let path = dir.path().join("entmoot-state.json");

            planted().save(&path).unwrap();
            let before = std::fs::read(&path).unwrap();

            let err = grown().save_with_hook(&path, crash_at(phase)).unwrap_err();
            assert!(
                matches!(err, EntmootError::Io(_)),
                "unexpected error at {phase:?}: {err}"
            );

            // Not truncated, not partially overwritten, not zero-length.
            assert_eq!(
                std::fs::read(&path).unwrap(),
                before,
                "ledger on disk was damaged by a save that failed at {phase:?}"
            );

            // And it still loads as the pre-save state, not something that
            // merely happens to be valid JSON.
            let reloaded = Store::load(&path).unwrap();
            assert_eq!(reloaded.channel_count(), 1);
            assert_eq!(reloaded.branch("chan-1").unwrap().message_count, 1);
            assert_eq!(reloaded.branch("chan-1").unwrap().earliest_unpruned().unwrap().id, 1);
            assert!(reloaded.history().is_empty());

            // The failed save cleaned up after itself.
            let leftovers: Vec<_> = std::fs::read_dir(dir.path())
                .unwrap()
                .map(|e| e.unwrap().path())
                .collect();
            assert_eq!(
                leftovers,
                vec![path.clone()],
                "save that failed at {phase:?} left files behind"
            );
        }
    }

    /// The other half of the invariant: when the save does not crash, it
    /// really does replace the ledger — the test above is not passing merely
    /// because `save` never writes anything.
    #[test]
    fn completed_save_replaces_the_ledger_and_leaves_no_temp_files() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("entmoot-state.json");

        planted().save(&path).unwrap();
        grown().save(&path).unwrap();

        let reloaded = Store::load(&path).unwrap();
        assert_eq!(reloaded.branch("chan-1").unwrap().message_count, 2);
        assert_eq!(reloaded.branch("chan-1").unwrap().earliest_unpruned().unwrap().id, 2);
        assert_eq!(reloaded.history().len(), 1);

        let entries: Vec<_> = std::fs::read_dir(dir.path())
            .unwrap()
            .map(|e| e.unwrap().path())
            .collect();
        assert_eq!(entries, vec![path]);
    }

    /// Saving through a temp file must not quietly re-permission the ledger.
    #[cfg(unix)]
    #[test]
    fn save_preserves_the_permissions_of_an_existing_ledger() {
        use std::os::unix::fs::PermissionsExt;

        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("entmoot-state.json");

        planted().save(&path).unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o640)).unwrap();

        grown().save(&path).unwrap();

        let mode = std::fs::metadata(&path).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o640, "save changed the ledger's permissions");
    }
}
