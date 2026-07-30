//! Integration tests for the Entmoot MVP.
//!
//! Everything here drives the crate through its public API, which is why it
//! lives out here rather than in a `#[cfg(test)] mod tests` inside `lib.rs`.
//! The one exception is the durability suite in `src/lib.rs`, which needs the
//! private save seam to simulate a crash mid-write.

use chrono::{DateTime, Duration, Utc};
use entmoot::{Disposition, Message, Store};

fn ts(seconds: i64) -> DateTime<Utc> {
    DateTime::<Utc>::UNIX_EPOCH + Duration::seconds(seconds)
}

fn msg(id: u64, seconds: i64, author: &str, content: &str) -> Message {
    Message::new(id, ts(seconds), author, content)
}

// --- Test 1 / Pace's "plant & show": message arrives, branch appears in status ---
#[test]
fn plant_and_show() {
    let mut store = Store::new();
    assert!(store.is_empty());

    store.water("chan-1", "general", msg(1, 0, "lina", "hello"));

    assert_eq!(store.channel_count(), 1);
    let status = store.status(ts(0));
    assert_eq!(status.len(), 1);
    assert_eq!(status[0].channel_id, "chan-1");
    assert_eq!(status[0].channel_name, "general");
    assert_eq!(status[0].message_count, 1);
    assert_eq!(status[0].latest.as_ref().unwrap().content, "hello");
}

// --- Test 2 / Pace's "staleness increment": idle time computed correctly from timestamps ---
#[test]
fn staleness_computation() {
    let mut store = Store::new();
    store.water("chan-1", "general", msg(1, 0, "lina", "hello"));

    let now = ts(600); // 10 minutes later
    let status = store.status(now);
    assert_eq!(status[0].staleness, Duration::minutes(10));

    let now_later = ts(3600); // 1 hour later
    let status = store.status(now_later);
    assert_eq!(status[0].staleness, Duration::hours(1));
}

// --- Test 3: activity resets staleness ---
#[test]
fn activity_resets_staleness() {
    let mut store = Store::new();
    store.water("chan-1", "general", msg(1, 0, "lina", "hello"));

    // Idle for a while...
    assert_eq!(store.status(ts(500))[0].staleness, Duration::seconds(500));

    // ...then new activity arrives.
    store.water("chan-1", "general", msg(2, 500, "ari", "still here"));

    // Staleness should now be computed from the NEW latest timestamp, not the original.
    assert_eq!(store.status(ts(500))[0].staleness, Duration::zero());
    assert_eq!(store.status(ts(700))[0].staleness, Duration::seconds(200));
}

// --- Test 4 / Pace's "tend surfaces stale": branches above threshold appear, fresh ones don't ---
#[test]
fn tend_surfaces_stale_not_fresh() {
    let mut store = Store::new();
    store.water("stale-chan", "quiet-room", msg(1, 0, "lina", "anyone there?"));
    store.water("fresh-chan", "busy-room", msg(1, 590, "ari", "just said this"));

    let now = ts(600);
    let threshold = Duration::minutes(10);
    let tended = store.tend(now, threshold);

    assert_eq!(tended.len(), 1);
    assert_eq!(tended[0].channel_id, "stale-chan");
    assert_eq!(tended[0].idle, Duration::minutes(10));
}

// --- Test 5 / Pace's "clear/prune removes": prune clears branch with disposition tag ---
#[test]
fn prune_removes_with_disposition() {
    let mut store = Store::new();
    store.water("chan-1", "general", msg(1, 0, "lina", "first"));
    store.water("chan-1", "general", msg(2, 10, "lina", "second"));

    let record = store.prune_latest("chan-1", Disposition::Done).unwrap();
    assert_eq!(record.disposition, Disposition::Done);
    assert_eq!(record.message_id, 2);

    let branch = store.branch("chan-1").unwrap();
    assert!(branch.is_empty_of_unpruned());
    // Lifetime count is untouched by pruning.
    assert_eq!(branch.message_count, 2);
}

// --- Test 6 / Pace's "multiple channels": branches at different staleness levels ---
#[test]
fn multiple_channels_independent_staleness() {
    let mut store = Store::new();
    store.water("chan-a", "alpha", msg(1, 0, "lina", "a"));
    store.water("chan-b", "beta", msg(1, 300, "lina", "b"));
    store.water("chan-c", "gamma", msg(1, 590, "lina", "c"));

    let now = ts(600);
    let status = store.status(now);
    assert_eq!(status.len(), 3);

    let by_id = |id: &str| status.iter().find(|s| s.channel_id == id).unwrap();
    assert_eq!(by_id("chan-a").staleness, Duration::seconds(600));
    assert_eq!(by_id("chan-b").staleness, Duration::seconds(300));
    assert_eq!(by_id("chan-c").staleness, Duration::seconds(10));

    // Only chan-a and chan-b clear a 5-minute threshold.
    let tended = store.tend(now, Duration::minutes(5));
    let tended_ids: Vec<_> = tended.iter().map(|t| t.channel_id.as_str()).collect();
    assert_eq!(tended_ids, vec!["chan-a", "chan-b"]);
}

// --- Test 7 / Pace's "persistence": create branches, reload from disk, verify state survived ---
#[test]
fn persistence_round_trip() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("entmoot-state.json");

    let mut store = Store::new();
    store.water("chan-1", "general", msg(1, 0, "lina", "first"));
    store.water("chan-1", "general", msg(2, 10, "ari", "second"));
    store.water("chan-2", "random", msg(1, 5, "pace", "hi"));
    store.prune("chan-1", 1, Disposition::Parked).unwrap();
    store.save(&path).unwrap();

    let reloaded = Store::load(&path).unwrap();
    assert_eq!(reloaded.channel_count(), 2);

    let chan1 = reloaded.branch("chan-1").unwrap();
    assert_eq!(chan1.message_count, 2);
    assert_eq!(chan1.earliest_unpruned().unwrap().id, 2);

    let history = reloaded.history_for("chan-1");
    assert_eq!(history.len(), 1);
    assert_eq!(history[0].disposition, Disposition::Parked);
}

// --- Test 8 / Pace's "empty state": status and tend on fresh state return clean empty output ---
#[test]
fn empty_state_is_clean() {
    let store = Store::new();
    assert!(store.is_empty());
    assert_eq!(store.status(ts(0)).len(), 0);
    assert_eq!(store.tend(ts(0), Duration::minutes(10)).len(), 0);
    assert_eq!(store.history().len(), 0);
}

// --- Test 9: Hadamard gate — promise is the last message, channel goes quiet, tend
// surfaces it with the promise text ---
#[test]
fn hadamard_gate_promise_is_last_message() {
    let mut store = Store::new();
    store.water("chan-1", "general", msg(1, 0, "lina", "small talk"));
    store.water(
        "chan-1",
        "general",
        msg(
            2,
            30,
            "ari",
            "I'll look into the Fredkin gate design and get back to you",
        ),
    );

    let now = ts(30 + 600);
    let tended = store.tend(now, Duration::minutes(10));

    assert_eq!(tended.len(), 1);
    assert_eq!(tended[0].earliest_unpruned.content, "small talk");
    // The promise is still in the branch — it just isn't the earliest
    // unpruned message yet, since "small talk" hasn't been pruned.
    // Prune past the small talk and the promise becomes earliest.
    drop(tended);
    store.prune("chan-1", 1, Disposition::Done).unwrap();
    let tended = store.tend(now, Duration::minutes(10));
    assert_eq!(tended.len(), 1);
    assert_eq!(
        tended[0].earliest_unpruned.content,
        "I'll look into the Fredkin gate design and get back to you"
    );
}

// --- Test 10: Clifford gate (path C) — promise followed by new activity. Tend
// surfaces EARLIEST unpruned (the promise), not latest ---
#[test]
fn clifford_gate_earliest_not_latest() {
    let mut store = Store::new();
    store.water("chan-1", "general", msg(1, 0, "ari", "I promise to send the doc"));
    store.water("chan-1", "general", msg(2, 30, "lina", "unrelated chatter"));
    store.water("chan-1", "general", msg(3, 60, "ari", "more chatter"));

    let now = ts(60 + 600);
    let tended = store.tend(now, Duration::minutes(10));

    assert_eq!(tended.len(), 1);
    // Earliest unpruned, NOT latest — this is the whole point of FIFO tend.
    assert_eq!(tended[0].earliest_unpruned.id, 1);
    assert_eq!(tended[0].earliest_unpruned.content, "I promise to send the doc");
}

// --- Test 11: Fredkin gate — promise buried in middle of chatter. Prune past
// blah1/blah2, promise floats to top as earliest unpruned ---
#[test]
fn fredkin_gate_promise_floats_to_top_after_prune() {
    let mut store = Store::new();
    store.water("chan-1", "general", msg(1, 0, "lina", "blah1"));
    store.water("chan-1", "general", msg(2, 10, "lina", "blah2"));
    store.water("chan-1", "general", msg(3, 20, "ari", "I'll circle back on the schema"));
    store.water("chan-1", "general", msg(4, 30, "lina", "more blah"));

    // Before pruning, earliest unpruned is blah1, not the promise.
    assert_eq!(store.branch("chan-1").unwrap().earliest_unpruned().unwrap().id, 1);

    // Prune past blah1/blah2.
    store.prune("chan-1", 2, Disposition::Dropped).unwrap();

    let branch = store.branch("chan-1").unwrap();
    assert_eq!(branch.earliest_unpruned().unwrap().id, 3);
    assert_eq!(
        branch.earliest_unpruned().unwrap().content,
        "I'll circle back on the schema"
    );

    let now = ts(30 + 600);
    let tended = store.tend(now, Duration::minutes(10));
    assert_eq!(tended[0].earliest_unpruned.content, "I'll circle back on the schema");
}

// --- Test 12: Race condition — message arrives between prune decision and
// execution. Late arrival (later id) survives the cursor ---
#[test]
fn race_condition_late_arrival_survives_cursor() {
    let mut store = Store::new();
    store.water("chan-1", "general", msg(1, 0, "lina", "blah1"));
    store.water("chan-1", "general", msg(2, 10, "lina", "blah2"));

    // Caller decides to prune: captures the cursor at decision time.
    let cursor = store.latest_known_id("chan-1").unwrap();
    assert_eq!(cursor, 2);

    // A new message arrives before the prune actually executes.
    store.water("chan-1", "general", msg(3, 20, "ari", "wait, one more thing"));

    // Prune executes using the captured cursor, not "whatever is latest now".
    store.prune("chan-1", cursor, Disposition::Done).unwrap();

    let branch = store.branch("chan-1").unwrap();
    // The late arrival (id 3) survives — it has a higher id than the cursor.
    assert_eq!(branch.earliest_unpruned().unwrap().id, 3);
    assert_eq!(branch.earliest_unpruned().unwrap().content, "wait, one more thing");
}

// --- Test 13: Prune disposition — prune records done/dropped/parked tag,
// retrievable from history ---
#[test]
fn prune_disposition_retrievable_from_history() {
    let mut store = Store::new();
    store.water("chan-1", "general", msg(1, 0, "lina", "a"));
    store.water("chan-1", "general", msg(2, 10, "lina", "b"));
    store.water("chan-1", "general", msg(3, 20, "lina", "c"));

    store.prune("chan-1", 1, Disposition::Parked).unwrap();
    store.prune("chan-1", 2, Disposition::Dropped).unwrap();
    store.prune("chan-1", 3, Disposition::Done).unwrap();

    let history = store.history_for("chan-1");
    assert_eq!(history.len(), 3);
    assert_eq!(history[0].disposition, Disposition::Parked);
    assert_eq!(history[1].disposition, Disposition::Dropped);
    assert_eq!(history[2].disposition, Disposition::Done);

    // Global history also carries all channels' records.
    assert_eq!(store.history().len(), 3);
}

// --- Test: prune(channel) = prune(channel, latest_known_id) — one code path ---
#[test]
fn prune_latest_matches_explicit_prune_with_latest_known_id() {
    let mut store_a = Store::new();
    store_a.water("chan-1", "general", msg(1, 0, "lina", "a"));
    store_a.water("chan-1", "general", msg(2, 10, "lina", "b"));

    let mut store_b = store_a.clone();

    let latest = store_a.latest_known_id("chan-1").unwrap();
    store_a.prune("chan-1", latest, Disposition::Done).unwrap();
    store_b.prune_latest("chan-1", Disposition::Done).unwrap();

    assert_eq!(
        store_a.branch("chan-1").unwrap().is_empty_of_unpruned(),
        store_b.branch("chan-1").unwrap().is_empty_of_unpruned()
    );
    assert_eq!(store_a.history()[0].message_id, store_b.history()[0].message_id);
}

// --- Test 14: Self-contained — no external dependencies on siblings,
// humans, or crons. Exercised by construction: every test above only ever
// calls Store methods with synthetic Message values. This test asserts
// the crate has no notion of "who else is watching" or "is a human
// present" baked into its API — a full lifecycle runs to completion with
// nothing but water/status/tend/prune. ---
#[test]
fn self_contained_full_lifecycle_needs_nothing_external() {
    let mut store = Store::new();
    store.water("chan-1", "general", msg(1, 0, "lina", "hi"));
    let _ = store.status(ts(10));
    let _ = store.tend(ts(700), Duration::minutes(10));
    store.prune_latest("chan-1", Disposition::Done).unwrap();
    assert_eq!(store.history().len(), 1);
    // No cron, no human approval, no sibling construct — just calls.
}
