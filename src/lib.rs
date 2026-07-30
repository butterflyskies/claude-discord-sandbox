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

    /// The most recent message that hasn't been pruned yet. `None` once the
    /// branch is fully pruned — this is the unpruned tail, not a record of
    /// everything ever received. `last_activity` and `latest_known_id` are
    /// the fields that survive a prune.
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

/// The house default staleness threshold: 10 minutes. `tend()` takes its
/// threshold as a required argument and never reads this — it is here so
/// callers that want the default have one place to read it from rather than
/// each hardcoding their own 10.
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

    /// Status of every known branch, sorted by `channel_id`. The order is a
    /// guarantee, not an accident of the underlying map — callers may index
    /// into the result.
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
    ///
    /// Two limits on that guarantee, both deliberate:
    ///
    /// - Surviving power loss (as opposed to a process dying) additionally
    ///   requires the parent directory to be synced, which is done only on
    ///   unix. On other platforms the rename is atomic but its durability is
    ///   whatever the filesystem offers.
    /// - A process killed outright runs no destructors, so the temp file it
    ///   was writing is left behind. It is inert — a uniquely named sibling
    ///   the ledger never points at — but nothing reaps it.
    ///
    /// An `Err` returned after the rename has already committed is possible
    /// (only from the directory sync), so a failed `save` means "the new state
    /// may or may not be live", never "the old state was damaged".
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
        // A hard kill runs no destructors and does leave the temp behind; see
        // the note on `save`.
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

    /// Load a store from `path`. A missing file — and only a missing file —
    /// yields an empty store: a fresh Entmoot instance with nothing planted.
    ///
    /// Every other failure to read the ledger is an error, deliberately. The
    /// tempting shape here is `if !path.exists()`, but `exists()` reports
    /// `false` for any failed stat — an unreadable parent directory, a
    /// dangling symlink, a transient I/O error — so an unreachable ledger
    /// would load as "nothing planted yet" and the next [`Store::save`] would
    /// durably, atomically replace the real ledger with an empty one. Opening
    /// and matching on the error keeps that failure loud.
    pub fn load(path: impl AsRef<Path>) -> Result<Self, EntmootError> {
        match std::fs::read(path.as_ref()) {
            Ok(bytes) => Ok(serde_json::from_slice(&bytes)?),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Self::default()),
            Err(e) => Err(e.into()),
        }
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

    fn dir_entries(dir: &Path) -> Vec<std::path::PathBuf> {
        let mut paths: Vec<_> = std::fs::read_dir(dir).unwrap().map(|e| e.unwrap().path()).collect();
        paths.sort();
        paths
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

            // Watch the crash window from inside it: at the moment of the
            // fault there really is a half-written payload sitting next to the
            // ledger, and the ledger really is still the old one. Without this
            // the phases would be indistinguishable from each other and from a
            // save that never wrote anything at all.
            let saving = grown();
            let full_len = serde_json::to_vec_pretty(&saving).unwrap().len();
            let mut observed_mid_write = false;
            let mut crash = crash_at(phase);
            let err = saving
                .save_with_hook(&path, |reached| {
                    if reached == SavePhase::PartiallyWritten {
                        let siblings: Vec<_> = dir_entries(dir.path()).into_iter().filter(|p| *p != path).collect();
                        assert_eq!(siblings.len(), 1, "expected exactly one temp file mid-write");
                        let partial = std::fs::metadata(&siblings[0]).unwrap().len() as usize;
                        assert!(partial > 0, "temp file was empty mid-write");
                        assert!(
                            partial < full_len,
                            "temp file held the whole payload, so there was no mid-write window"
                        );
                        assert_eq!(std::fs::read(&path).unwrap(), before, "ledger touched mid-write");
                        observed_mid_write = true;
                    }
                    crash(reached)
                })
                .unwrap_err();
            assert!(observed_mid_write, "the mid-write checkpoint was never reached");
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

            // The error path cleans up its own temp file. Note the scope: this
            // holds because the fault unwinds and runs destructors. A process
            // killed outright would leave the temp behind — inert, but not
            // reaped — which is why `save` documents that rather than
            // pretending this assertion covers it.
            assert_eq!(
                dir_entries(dir.path()),
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

        assert_eq!(dir_entries(dir.path()), vec![path]);
    }

    /// Both arms of the permission handling. Replacing a ledger keeps the mode
    /// it already had; creating one keeps the temp file's restrictive default,
    /// which matters because the ledger holds message content and authors.
    #[cfg(unix)]
    #[test]
    fn save_sets_ledger_permissions_from_the_file_it_replaces() {
        use std::os::unix::fs::PermissionsExt;

        let dir = tempfile::tempdir().unwrap();

        let fresh = dir.path().join("fresh.json");
        planted().save(&fresh).unwrap();
        let mode = std::fs::metadata(&fresh).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o600, "a brand new ledger was not created private");

        let existing = dir.path().join("existing.json");
        planted().save(&existing).unwrap();
        std::fs::set_permissions(&existing, std::fs::Permissions::from_mode(0o640)).unwrap();
        grown().save(&existing).unwrap();
        let mode = std::fs::metadata(&existing).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o640, "save changed the ledger's permissions");
    }

    /// `load` must distinguish "no ledger yet" from "ledger unreadable". The
    /// first is a fresh start; the second must not present as one, or the next
    /// save durably replaces a real ledger with an empty one.
    #[test]
    fn load_returns_empty_only_for_a_missing_ledger() {
        let dir = tempfile::tempdir().unwrap();

        let missing = dir.path().join("not-written-yet.json");
        assert!(Store::load(&missing).unwrap().is_empty());

        // A directory where a ledger was expected reads as an I/O error, not
        // as an empty store.
        let not_a_file = dir.path().join("a-directory.json");
        std::fs::create_dir(&not_a_file).unwrap();
        assert!(matches!(Store::load(&not_a_file), Err(EntmootError::Io(_))));

        let corrupt = dir.path().join("corrupt.json");
        std::fs::write(&corrupt, b"{ this is not a ledger").unwrap();
        assert!(matches!(Store::load(&corrupt), Err(EntmootError::Serde(_))));
    }

    /// The case that makes the `exists()` shape dangerous rather than merely
    /// unidiomatic: a real ledger that cannot be stat'd. `Path::exists` answers
    /// `false` here, which would present a populated ledger as a fresh install
    /// and hand the next save a licence to overwrite it.
    #[cfg(unix)]
    #[test]
    fn an_unreadable_ledger_is_an_error_not_an_empty_store() {
        use std::os::unix::fs::PermissionsExt;

        let dir = tempfile::tempdir().unwrap();
        let vault = dir.path().join("vault");
        std::fs::create_dir(&vault).unwrap();
        let path = vault.join("entmoot-state.json");
        planted().save(&path).unwrap();

        // Revoke traversal on the parent, so stat and open both fail.
        std::fs::set_permissions(&vault, std::fs::Permissions::from_mode(0o000)).unwrap();
        let result = Store::load(&path);
        // Root ignores permission bits; record whether they actually bit
        // before restoring, so this test skips rather than lies when run as
        // root.
        let stat_was_blocked = !path.exists();
        std::fs::set_permissions(&vault, std::fs::Permissions::from_mode(0o700)).unwrap();

        if stat_was_blocked {
            match result {
                Err(EntmootError::Io(e)) => {
                    assert_eq!(e.kind(), std::io::ErrorKind::PermissionDenied)
                }
                other => panic!("unreadable ledger loaded as {other:?} instead of an error"),
            }
        }
    }
}
