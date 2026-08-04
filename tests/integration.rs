//! Integration tests for the Entmoot MVP.
//!
//! Everything here drives the crate through its public API, which is why it
//! lives out here rather than in a `#[cfg(test)] mod tests` inside `lib.rs`.
//! The one exception is the durability suite in `src/lib.rs`, which needs the
//! private save seam to simulate a crash mid-write.

use chrono::{DateTime, Duration, Utc};
use entmoot::{
    Author, ChannelId, ChannelName, Content, Disposition, EntmootError, InvalidChannelId, InvalidText, Message, Store,
    MAX_AUTHOR_LEN, MAX_CHANNEL_NAME_LEN, MAX_CONTENT_LEN,
};

fn ts(seconds: i64) -> DateTime<Utc> {
    DateTime::<Utc>::UNIX_EPOCH + Duration::seconds(seconds)
}

fn msg(id: u64, seconds: i64, author: &str, content: &str) -> Message {
    Message::new(id, ts(seconds), author, content).unwrap()
}

fn cn(s: &str) -> ChannelName {
    ChannelName::parse(s).unwrap()
}

/// Convenience constant for test channel IDs.
const CH1: ChannelId = ChannelId::new(1001);
const CH2: ChannelId = ChannelId::new(1002);
const CH_STALE: ChannelId = ChannelId::new(2001);
const CH_FRESH: ChannelId = ChannelId::new(2002);
const CH_Z: ChannelId = ChannelId::new(3001);
const CH_A: ChannelId = ChannelId::new(3002);
const CH_M: ChannelId = ChannelId::new(3003);
const CH_EMPTIED: ChannelId = ChannelId::new(4001);
const CH_OPEN: ChannelId = ChannelId::new(4002);
const CH_NOSUCH: ChannelId = ChannelId::new(9999);

// --- Test 1 / Pace's "plant & show": message arrives, branch appears in status ---
#[test]
fn plant_and_show() {
    let mut store = Store::new();
    assert!(store.is_empty());

    store.water(CH1, cn("general"), msg(1, 0, "lina", "hello"));

    assert_eq!(store.channel_count(), 1);
    let status = store.status(ts(0));
    assert_eq!(status.len(), 1);
    assert_eq!(status[0].channel_id, CH1);
    assert_eq!(status[0].channel_name, "general");
    assert_eq!(status[0].message_count, 1);
    assert_eq!(status[0].latest.as_ref().unwrap().content, "hello");
}

// --- Test 2 / Pace's "staleness increment": idle time computed correctly from timestamps ---
#[test]
fn staleness_computation() {
    let mut store = Store::new();
    store.water(CH1, cn("general"), msg(1, 0, "lina", "hello"));

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
    store.water(CH1, cn("general"), msg(1, 0, "lina", "hello"));

    // Idle for a while...
    assert_eq!(store.status(ts(500))[0].staleness, Duration::seconds(500));

    // ...then new activity arrives.
    store.water(CH1, cn("general"), msg(2, 500, "ari", "still here"));

    // Staleness should now be computed from the NEW latest timestamp, not the original.
    assert_eq!(store.status(ts(500))[0].staleness, Duration::zero());
    assert_eq!(store.status(ts(700))[0].staleness, Duration::seconds(200));
}

// --- Test 3a: watering an existing branch with a different name updates it ---
#[test]
fn water_updates_channel_name_on_existing_branch() {
    let mut store = Store::new();
    store.water(CH1, cn("general"), msg(1, 0, "lina", "hello"));
    assert_eq!(store.branch(CH1).unwrap().channel_name, "general");

    // The channel was renamed in Discord.
    store.water(CH1, cn("general-chat"), msg(2, 10, "ari", "hi"));
    assert_eq!(store.branch(CH1).unwrap().channel_name, "general-chat");

    // Verify the name is persisted and survives a round trip.
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("entmoot-state.json");
    store.save(&path).unwrap();
    let reloaded = Store::load(&path).unwrap();
    assert_eq!(reloaded.branch(CH1).unwrap().channel_name, "general-chat");
}

// --- Test 3b: out-of-order arrival. Every guard in `water` that exists for
// reordered delivery — the id sort, the last_activity max, the latest_known_id
// max — only does work when messages arrive jumbled, which is the normal case
// for a gateway that replays or backfills. Ordering is by id, and a late
// message carrying an older timestamp must not rewind the staleness clock. ---
#[test]
fn out_of_order_arrival_is_ordered_by_id_and_never_rewinds_the_clock() {
    let mut store = Store::new();
    store.water(CH1, cn("general"), msg(3, 30, "lina", "third"));
    store.water(CH1, cn("general"), msg(1, 0, "lina", "first"));
    store.water(CH1, cn("general"), msg(2, 10, "ari", "second"));

    let branch = store.branch(CH1).unwrap();
    // FIFO is by id, not by arrival — this is what tend() depends on.
    assert_eq!(branch.earliest_unpruned().unwrap().id, 1);
    assert_eq!(branch.latest().unwrap().id, 3);
    assert_eq!(branch.message_count, 3);
    // The two late arrivals were older, so the branch is as stale as its
    // newest message, not as fresh as the last one received.
    assert_eq!(branch.staleness(ts(30)), Duration::zero());
    assert_eq!(store.latest_known_id(CH1).unwrap(), 3);

    // And the prune cursor still means "id <= 2", regardless of arrival order.
    store.prune(CH1, 2, Disposition::Done, ts(30)).unwrap();
    let branch = store.branch(CH1).unwrap();
    assert_eq!(branch.earliest_unpruned().unwrap().id, 3);
}

// --- Test 4 / Pace's "tend surfaces stale": branches above threshold appear, fresh ones don't ---
#[test]
fn tend_surfaces_stale_not_fresh() {
    let mut store = Store::new();
    store.water(CH_STALE, cn("quiet-room"), msg(1, 0, "lina", "anyone there?"));
    store.water(CH_FRESH, cn("busy-room"), msg(1, 590, "ari", "just said this"));

    let now = ts(600);
    let threshold = Duration::minutes(10);
    let tended = store.tend(now, threshold);

    assert_eq!(tended.len(), 1);
    assert_eq!(tended[0].channel_id, CH_STALE);
    assert_eq!(tended[0].idle, Duration::minutes(10));
}

// --- Test 5 / Pace's "clear/prune removes": prune clears branch with disposition tag ---
#[test]
fn prune_removes_with_disposition() {
    let mut store = Store::new();
    store.water(CH1, cn("general"), msg(1, 0, "lina", "first"));
    store.water(CH1, cn("general"), msg(2, 10, "lina", "second"));

    let record = store.prune_latest(CH1, Disposition::Done, ts(10)).unwrap();
    assert_eq!(record.disposition, Disposition::Done);
    assert_eq!(record.message_id, 2);
    assert_eq!(record.pruned_at, ts(10));

    let branch = store.branch(CH1).unwrap();
    assert!(branch.is_empty_of_unpruned());
    // Lifetime count is untouched by pruning.
    assert_eq!(branch.message_count, 2);
}

// --- Test 6 / Pace's "multiple channels": branches at different staleness levels ---
#[test]
fn multiple_channels_independent_staleness() {
    let mut store = Store::new();
    // Channel ids are deliberately non-sequential relative to their staleness
    // order — CH_Z is the stalest, CH_A is second, CH_M is freshest —
    // so the tend() assertion below proves the output is sorted by idle time
    // (most-idle first), not by channel id.
    store.water(CH_Z, cn("alpha"), msg(1, 0, "lina", "a"));
    store.water(CH_A, cn("beta"), msg(1, 300, "lina", "b"));
    store.water(CH_M, cn("gamma"), msg(1, 590, "lina", "c"));

    let now = ts(600);
    let status = store.status(now);
    assert_eq!(status.len(), 3);

    let by_id = |id: ChannelId| status.iter().find(|s| s.channel_id == id).unwrap();
    assert_eq!(by_id(CH_Z).staleness, Duration::seconds(600));
    assert_eq!(by_id(CH_A).staleness, Duration::seconds(300));
    assert_eq!(by_id(CH_M).staleness, Duration::seconds(10));

    // Only CH_Z and CH_A clear a 5-minute threshold, most-idle first.
    // If tend() sorted by id, this order would differ.
    let tended = store.tend(now, Duration::minutes(5));
    let tended_ids: Vec<_> = tended.iter().map(|t| t.channel_id).collect();
    assert_eq!(tended_ids, vec![CH_Z, CH_A]);
}

// --- Test 7 / Pace's "persistence": create branches, reload from disk, verify state survived ---
#[test]
fn persistence_round_trip() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("entmoot-state.json");

    let mut store = Store::new();
    store.water(CH1, cn("general"), msg(1, 0, "lina", "first"));
    store.water(CH1, cn("general"), msg(2, 10, "ari", "second"));
    store.water(CH2, cn("random"), msg(1, 5, "pace", "hi"));
    store.prune(CH1, 1, Disposition::Parked, ts(10)).unwrap();
    store.save(&path).unwrap();

    let reloaded = Store::load(&path).unwrap();
    assert_eq!(reloaded.channel_count(), 2);

    // Ids alone would survive a round trip that dropped the message payload,
    // so assert the content and author came back too.
    let chan1 = reloaded.branch(CH1).unwrap();
    assert_eq!(chan1.channel_name, "general");
    assert_eq!(chan1.message_count, 2);
    let survivor = chan1.earliest_unpruned().unwrap();
    assert_eq!(survivor.id, 2);
    assert_eq!(survivor.content, "second");
    assert_eq!(survivor.author, "ari");
    assert_eq!(survivor.timestamp, ts(10));

    // The second channel is not incidental — a bug that dropped every branch
    // but the first would still satisfy a channel_count check alone.
    let chan2 = reloaded.branch(CH2).unwrap();
    assert_eq!(chan2.channel_name, "random");
    assert_eq!(chan2.message_count, 1);
    let only = chan2.earliest_unpruned().unwrap();
    assert_eq!(only.content, "hi");
    assert_eq!(only.author, "pace");

    let history = reloaded.history_for(CH1);
    assert_eq!(history.len(), 1);
    assert_eq!(history[0].disposition, Disposition::Parked);
    assert_eq!(history[0].message_id, 1);
    // chan-2 was never pruned, so it carries no history of its own.
    assert!(reloaded.history_for(CH2).is_empty());
}

// --- Pruning a channel that has no branch is an error, not a silent no-op.
// Both prune entry points raise UnknownChannel, from two different guards. ---
#[test]
fn pruning_an_unknown_channel_is_an_error() {
    let mut store = Store::new();
    store.water(CH1, cn("general"), msg(1, 0, "lina", "hello"));

    match store.prune(CH_NOSUCH, 1, Disposition::Done, ts(0)) {
        Err(EntmootError::UnknownChannel(id)) => assert_eq!(id, CH_NOSUCH),
        other => panic!("expected UnknownChannel, got {other:?}"),
    }
    match store.prune_latest(CH_NOSUCH, Disposition::Done, ts(0)) {
        Err(EntmootError::UnknownChannel(id)) => assert_eq!(id, CH_NOSUCH),
        other => panic!("expected UnknownChannel, got {other:?}"),
    }

    // A rejected prune records nothing and leaves the real branch alone.
    assert!(store.history().is_empty());
    assert_eq!(store.branch(CH1).unwrap().earliest_unpruned().unwrap().id, 1);
}

// --- tend() documents that fully-pruned branches are never surfaced: there is
// nothing left to tend. The branch below is well past the threshold, so only
// the emptiness keeps it out of the result. ---
#[test]
fn tend_skips_a_stale_branch_with_nothing_left_unpruned() {
    let mut store = Store::new();
    store.water(CH_EMPTIED, cn("quiet-room"), msg(1, 0, "lina", "handled"));
    store.water(CH_OPEN, cn("loud-room"), msg(1, 0, "lina", "not handled"));

    let now = ts(3600);
    // Both branches are equally stale to begin with.
    assert_eq!(store.tend(now, Duration::minutes(10)).len(), 2);

    store.prune_latest(CH_EMPTIED, Disposition::Done, ts(3600)).unwrap();

    let tended = store.tend(now, Duration::minutes(10));
    let ids: Vec<_> = tended.iter().map(|t| t.channel_id).collect();
    assert_eq!(ids, vec![CH_OPEN]);
    // The branch still exists and is still stale — it just has nothing waiting.
    assert!(store.branch(CH_EMPTIED).unwrap().is_empty_of_unpruned());
    assert_eq!(store.status(now).len(), 2);
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
    store.water(CH1, cn("general"), msg(1, 0, "lina", "small talk"));
    store.water(
        CH1,
        cn("general"),
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
    store.prune(CH1, 1, Disposition::Done, now).unwrap();
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
    store.water(CH1, cn("general"), msg(1, 0, "ari", "I promise to send the doc"));
    store.water(CH1, cn("general"), msg(2, 30, "lina", "unrelated chatter"));
    store.water(CH1, cn("general"), msg(3, 60, "ari", "more chatter"));

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
    store.water(CH1, cn("general"), msg(1, 0, "lina", "blah1"));
    store.water(CH1, cn("general"), msg(2, 10, "lina", "blah2"));
    store.water(CH1, cn("general"), msg(3, 20, "ari", "I'll circle back on the schema"));
    store.water(CH1, cn("general"), msg(4, 30, "lina", "more blah"));

    // Before pruning, earliest unpruned is blah1, not the promise.
    assert_eq!(store.branch(CH1).unwrap().earliest_unpruned().unwrap().id, 1);

    // Prune past blah1/blah2.
    store.prune(CH1, 2, Disposition::Dropped, ts(20)).unwrap();

    let branch = store.branch(CH1).unwrap();
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
    store.water(CH1, cn("general"), msg(1, 0, "lina", "blah1"));
    store.water(CH1, cn("general"), msg(2, 10, "lina", "blah2"));

    // Caller decides to prune: captures the cursor at decision time.
    let cursor = store.latest_known_id(CH1).unwrap();
    assert_eq!(cursor, 2);

    // A new message arrives before the prune actually executes.
    store.water(CH1, cn("general"), msg(3, 20, "ari", "wait, one more thing"));

    // Prune executes using the captured cursor, not "whatever is latest now".
    store.prune(CH1, cursor, Disposition::Done, ts(20)).unwrap();

    let branch = store.branch(CH1).unwrap();
    // The late arrival (id 3) survives — it has a higher id than the cursor.
    assert_eq!(branch.earliest_unpruned().unwrap().id, 3);
    assert_eq!(branch.earliest_unpruned().unwrap().content, "wait, one more thing");
}

// --- Test 13: Prune disposition — prune records done/dropped/parked tag,
// retrievable from history ---
#[test]
fn prune_disposition_retrievable_from_history() {
    let mut store = Store::new();
    store.water(CH1, cn("general"), msg(1, 0, "lina", "a"));
    store.water(CH1, cn("general"), msg(2, 10, "lina", "b"));
    store.water(CH1, cn("general"), msg(3, 20, "lina", "c"));

    store.prune(CH1, 1, Disposition::Parked, ts(100)).unwrap();
    store.prune(CH1, 2, Disposition::Dropped, ts(200)).unwrap();
    store.prune(CH1, 3, Disposition::Done, ts(300)).unwrap();

    let history = store.history_for(CH1);
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
    store_a.water(CH1, cn("general"), msg(1, 0, "lina", "a"));
    store_a.water(CH1, cn("general"), msg(2, 10, "lina", "b"));

    let mut store_b = store_a.clone();

    let latest = store_a.latest_known_id(CH1).unwrap();
    let now = ts(10);
    store_a.prune(CH1, latest, Disposition::Done, now).unwrap();
    store_b.prune_latest(CH1, Disposition::Done, now).unwrap();

    // Comparing the two flags to each other would hold just as well if neither
    // prune had cleared anything, so pin the value rather than the agreement.
    assert!(store_a.branch(CH1).unwrap().is_empty_of_unpruned());
    assert!(store_b.branch(CH1).unwrap().is_empty_of_unpruned());
    assert_eq!(store_a.history()[0].message_id, latest);
    assert_eq!(store_b.history()[0].message_id, latest);
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
    store.water(CH1, cn("general"), msg(1, 0, "lina", "hi"));

    let status = store.status(ts(10));
    assert_eq!(status.len(), 1);
    assert_eq!(status[0].message_count, 1);

    let tended = store.tend(ts(700), Duration::minutes(10));
    assert_eq!(tended.len(), 1);
    assert_eq!(tended[0].earliest_unpruned.content, "hi");

    store.prune_latest(CH1, Disposition::Done, ts(700)).unwrap();
    assert_eq!(store.history().len(), 1);
    assert!(store.branch(CH1).unwrap().is_empty_of_unpruned());
    // No cron, no human approval, no sibling construct — just calls.
}

// ====================================================================
// Boundary validation tests for ChannelId
// ====================================================================

#[test]
fn channel_id_parse_valid_snowflakes() {
    // Minimal valid: a single digit
    assert_eq!(ChannelId::parse("0").unwrap().as_u64(), 0);
    assert_eq!(ChannelId::parse("1").unwrap().as_u64(), 1);

    // Realistic Discord snowflake
    assert_eq!(
        ChannelId::parse("1507753511155405011").unwrap().as_u64(),
        1507753511155405011
    );

    // u64::MAX — 20 digits, the boundary
    assert_eq!(ChannelId::parse("18446744073709551615").unwrap().as_u64(), u64::MAX);
}

#[test]
fn channel_id_rejects_empty() {
    assert_eq!(ChannelId::parse(""), Err(InvalidChannelId::Empty));
}

#[test]
fn channel_id_rejects_too_long() {
    // 21 digits — one past the max
    assert_eq!(
        ChannelId::parse("123456789012345678901"),
        Err(InvalidChannelId::TooLong)
    );

    // Way too long — the length check is O(1) and fires before parsing
    let huge = "9".repeat(1_000_000);
    assert_eq!(ChannelId::parse(&huge), Err(InvalidChannelId::TooLong));
}

#[test]
fn channel_id_rejects_non_numeric() {
    assert_eq!(ChannelId::parse("chan-1"), Err(InvalidChannelId::NotNumeric));
    assert_eq!(ChannelId::parse("abc"), Err(InvalidChannelId::NotNumeric));
    assert_eq!(ChannelId::parse("-1"), Err(InvalidChannelId::NotNumeric));
    assert_eq!(ChannelId::parse("12 34"), Err(InvalidChannelId::NotNumeric));
    assert_eq!(ChannelId::parse("12.34"), Err(InvalidChannelId::NotNumeric));
    // Leading/trailing whitespace
    assert_eq!(ChannelId::parse(" 123"), Err(InvalidChannelId::NotNumeric));
    assert_eq!(ChannelId::parse("123 "), Err(InvalidChannelId::NotNumeric));
}

#[test]
fn channel_id_rejects_overflow() {
    // One past u64::MAX — 20 digits but overflows
    assert_eq!(
        ChannelId::parse("18446744073709551616"),
        Err(InvalidChannelId::NotNumeric)
    );
    // 20 nines — overflows u64
    assert_eq!(
        ChannelId::parse("99999999999999999999"),
        Err(InvalidChannelId::NotNumeric)
    );
}

#[test]
fn channel_id_display_is_decimal() {
    let id = ChannelId::new(1507753511155405011);
    assert_eq!(id.to_string(), "1507753511155405011");
}

#[test]
fn channel_id_serde_round_trip() {
    let id = ChannelId::new(1507753511155405011);
    let json = serde_json::to_string(&id).unwrap();
    assert_eq!(json, "\"1507753511155405011\"");

    let back: ChannelId = serde_json::from_str(&json).unwrap();
    assert_eq!(back, id);
}

#[test]
fn channel_id_serde_rejects_invalid_json() {
    // Non-numeric string
    let result: Result<ChannelId, _> = serde_json::from_str("\"not-a-number\"");
    assert!(result.is_err());

    // Empty string
    let result: Result<ChannelId, _> = serde_json::from_str("\"\"");
    assert!(result.is_err());

    // Too long
    let too_long = format!("\"{}\"", "1".repeat(21));
    let result: Result<ChannelId, _> = serde_json::from_str(&too_long);
    assert!(result.is_err());
}

/// The motivating attack: a 1MB+ string passed as a channel_id. The length
/// check fires in O(1) before any parsing or allocation happens.
#[test]
fn channel_id_rejects_pathological_size() {
    let payload = "1".repeat(1_048_576); // 1 MB of '1's
    assert_eq!(ChannelId::parse(&payload), Err(InvalidChannelId::TooLong));

    // Even through serde, the rejection is cheap
    let json = format!("\"{}\"", payload);
    let result: Result<ChannelId, _> = serde_json::from_str(&json);
    assert!(result.is_err());
}

// ====================================================================
// Boundary validation tests for Author
// ====================================================================

#[test]
fn author_accepts_valid_values() {
    // Single character
    assert_eq!(Author::parse("a").unwrap().as_str(), "a");

    // Realistic Discord username
    assert_eq!(Author::parse("lina").unwrap().as_str(), "lina");

    // Exactly at the limit
    let at_max = "x".repeat(MAX_AUTHOR_LEN);
    assert_eq!(Author::parse(&at_max).unwrap().as_str(), at_max);
}

#[test]
fn author_rejects_empty() {
    assert_eq!(Author::parse(""), Err(InvalidText::Empty));
}

#[test]
fn author_rejects_too_long() {
    // One byte past the limit
    let over = "x".repeat(MAX_AUTHOR_LEN + 1);
    assert_eq!(Author::parse(&over), Err(InvalidText::TooLong));
}

#[test]
fn author_rejects_pathological_size() {
    let payload = "a".repeat(1_048_576);
    assert_eq!(Author::parse(&payload), Err(InvalidText::TooLong));
}

#[test]
fn author_serde_round_trip() {
    let author = Author::parse("lina").unwrap();
    let json = serde_json::to_string(&author).unwrap();
    assert_eq!(json, "\"lina\"");

    let back: Author = serde_json::from_str(&json).unwrap();
    assert_eq!(back, author);
}

#[test]
fn author_serde_rejects_invalid() {
    // Empty
    let result: Result<Author, _> = serde_json::from_str("\"\"");
    assert!(result.is_err());

    // Too long
    let too_long = format!("\"{}\"", "x".repeat(MAX_AUTHOR_LEN + 1));
    let result: Result<Author, _> = serde_json::from_str(&too_long);
    assert!(result.is_err());
}

// ====================================================================
// Boundary validation tests for Content
// ====================================================================

#[test]
fn content_accepts_valid_values() {
    // Empty is permitted — messages can carry attachments with no text body
    assert_eq!(Content::parse("").unwrap().as_str(), "");

    // Single character
    assert_eq!(Content::parse("x").unwrap().as_str(), "x");

    // Exactly at the limit
    let at_max = "x".repeat(MAX_CONTENT_LEN);
    assert_eq!(Content::parse(&at_max).unwrap().as_str(), at_max);
}

#[test]
fn content_rejects_too_long() {
    // One byte past the limit
    let over = "x".repeat(MAX_CONTENT_LEN + 1);
    assert_eq!(Content::parse(&over), Err(InvalidText::TooLong));
}

#[test]
fn content_rejects_pathological_size() {
    let payload = "a".repeat(1_048_576);
    assert_eq!(Content::parse(&payload), Err(InvalidText::TooLong));
}

#[test]
fn content_serde_round_trip() {
    let content = Content::parse("hello world").unwrap();
    let json = serde_json::to_string(&content).unwrap();
    assert_eq!(json, "\"hello world\"");

    let back: Content = serde_json::from_str(&json).unwrap();
    assert_eq!(back, content);
}

#[test]
fn content_serde_round_trip_empty() {
    let content = Content::parse("").unwrap();
    let json = serde_json::to_string(&content).unwrap();
    assert_eq!(json, "\"\"");

    let back: Content = serde_json::from_str(&json).unwrap();
    assert_eq!(back, content);
}

#[test]
fn content_serde_rejects_too_long() {
    let too_long = format!("\"{}\"", "x".repeat(MAX_CONTENT_LEN + 1));
    let result: Result<Content, _> = serde_json::from_str(&too_long);
    assert!(result.is_err());
}

// ====================================================================
// Boundary validation tests for ChannelName
// ====================================================================

#[test]
fn channel_name_accepts_valid_values() {
    // Single character
    assert_eq!(ChannelName::parse("g").unwrap().as_str(), "g");

    // Realistic Discord channel name
    assert_eq!(ChannelName::parse("general").unwrap().as_str(), "general");

    // Exactly at the limit
    let at_max = "x".repeat(MAX_CHANNEL_NAME_LEN);
    assert_eq!(ChannelName::parse(&at_max).unwrap().as_str(), at_max);
}

#[test]
fn channel_name_rejects_empty() {
    assert_eq!(ChannelName::parse(""), Err(InvalidText::Empty));
}

#[test]
fn channel_name_rejects_too_long() {
    // One byte past the limit
    let over = "x".repeat(MAX_CHANNEL_NAME_LEN + 1);
    assert_eq!(ChannelName::parse(&over), Err(InvalidText::TooLong));
}

#[test]
fn channel_name_rejects_pathological_size() {
    let payload = "a".repeat(1_048_576);
    assert_eq!(ChannelName::parse(&payload), Err(InvalidText::TooLong));
}

#[test]
fn channel_name_serde_round_trip() {
    let name = ChannelName::parse("general").unwrap();
    let json = serde_json::to_string(&name).unwrap();
    assert_eq!(json, "\"general\"");

    let back: ChannelName = serde_json::from_str(&json).unwrap();
    assert_eq!(back, name);
}

#[test]
fn channel_name_serde_rejects_invalid() {
    // Empty
    let result: Result<ChannelName, _> = serde_json::from_str("\"\"");
    assert!(result.is_err());

    // Too long
    let too_long = format!("\"{}\"", "x".repeat(MAX_CHANNEL_NAME_LEN + 1));
    let result: Result<ChannelName, _> = serde_json::from_str(&too_long);
    assert!(result.is_err());
}

// ====================================================================
// Message construction validation
// ====================================================================

#[test]
fn message_new_validates_author() {
    // Empty author
    match Message::new(1, ts(0), "", "hello") {
        Err(EntmootError::InvalidAuthor(InvalidText::Empty)) => {}
        other => panic!("expected InvalidAuthor(Empty), got {other:?}"),
    }

    // Too-long author
    let long_author = "x".repeat(MAX_AUTHOR_LEN + 1);
    match Message::new(1, ts(0), &long_author, "hello") {
        Err(EntmootError::InvalidAuthor(InvalidText::TooLong)) => {}
        other => panic!("expected InvalidAuthor(TooLong), got {other:?}"),
    }
}

#[test]
fn message_new_validates_content() {
    // Too-long content
    let long_content = "x".repeat(MAX_CONTENT_LEN + 1);
    match Message::new(1, ts(0), "lina", &long_content) {
        Err(EntmootError::InvalidContent(InvalidText::TooLong)) => {}
        other => panic!("expected InvalidContent(TooLong), got {other:?}"),
    }

    // Empty content is allowed
    let msg = Message::new(1, ts(0), "lina", "").unwrap();
    assert_eq!(msg.content, "");
}

#[test]
fn message_new_validates_author_before_content() {
    // When both are invalid, the author error surfaces first
    let long = "x".repeat(MAX_CONTENT_LEN + 1);
    match Message::new(1, ts(0), "", &long) {
        Err(EntmootError::InvalidAuthor(InvalidText::Empty)) => {}
        other => panic!("expected InvalidAuthor(Empty), got {other:?}"),
    }
}

// ====================================================================
// Unbounded history growth — max_messages_per_branch
// ====================================================================

#[test]
fn no_cap_by_default() {
    let store = Store::new();
    assert_eq!(store.max_messages_per_branch(), None);
}

#[test]
fn cap_evicts_oldest_first() {
    let mut store = Store::new();
    store.set_max_messages_per_branch(3);

    store.water(CH1, cn("general"), msg(1, 0, "lina", "first"));
    store.water(CH1, cn("general"), msg(2, 10, "lina", "second"));
    store.water(CH1, cn("general"), msg(3, 20, "lina", "third"));
    assert_eq!(store.branch(CH1).unwrap().unpruned_count(), 3);

    // Fourth message pushes past the cap — oldest is evicted
    store.water(CH1, cn("general"), msg(4, 30, "lina", "fourth"));
    let branch = store.branch(CH1).unwrap();
    assert_eq!(branch.unpruned_count(), 3);
    assert_eq!(branch.earliest_unpruned().unwrap().id, 2);
    assert_eq!(branch.latest().unwrap().id, 4);

    // Lifetime count is unaffected by eviction
    assert_eq!(branch.message_count, 4);
}

#[test]
fn cap_enforced_on_existing_branches() {
    let mut store = Store::new();

    // Add 5 messages without a cap
    for i in 1..=5 {
        store.water(CH1, cn("general"), msg(i, i as i64 * 10, "lina", "msg"));
    }
    assert_eq!(store.branch(CH1).unwrap().unpruned_count(), 5);

    // Setting a cap trims existing branches immediately
    store.set_max_messages_per_branch(2);
    let branch = store.branch(CH1).unwrap();
    assert_eq!(branch.unpruned_count(), 2);
    assert_eq!(branch.earliest_unpruned().unwrap().id, 4);
    assert_eq!(branch.latest().unwrap().id, 5);
}

#[test]
fn cap_of_one_keeps_only_latest() {
    let mut store = Store::new();
    store.set_max_messages_per_branch(1);

    store.water(CH1, cn("general"), msg(1, 0, "lina", "first"));
    store.water(CH1, cn("general"), msg(2, 10, "lina", "second"));
    store.water(CH1, cn("general"), msg(3, 20, "lina", "third"));

    let branch = store.branch(CH1).unwrap();
    assert_eq!(branch.unpruned_count(), 1);
    assert_eq!(branch.earliest_unpruned().unwrap().id, 3);
    assert_eq!(branch.message_count, 3);
}

#[test]
fn cap_of_zero_evicts_everything() {
    let mut store = Store::new();
    store.set_max_messages_per_branch(0);

    store.water(CH1, cn("general"), msg(1, 0, "lina", "hello"));

    let branch = store.branch(CH1).unwrap();
    assert!(branch.is_empty_of_unpruned());
    // The branch exists, was watered, but has no unpruned messages
    assert_eq!(branch.message_count, 1);

    // tend() never surfaces it — there's nothing to tend
    assert!(store.tend(ts(3600), Duration::minutes(10)).is_empty());
}

#[test]
fn cap_persists_and_enforces_on_load() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("entmoot-state.json");

    let mut store = Store::new();
    store.set_max_messages_per_branch(3);
    for i in 1..=5 {
        store.water(CH1, cn("general"), msg(i, i as i64 * 10, "lina", "msg"));
    }
    store.save(&path).unwrap();

    let reloaded = Store::load(&path).unwrap();
    assert_eq!(reloaded.max_messages_per_branch(), Some(3));
    assert_eq!(reloaded.branch(CH1).unwrap().unpruned_count(), 3);
    assert_eq!(reloaded.branch(CH1).unwrap().earliest_unpruned().unwrap().id, 3);
}

#[test]
fn cap_does_not_create_prune_records() {
    let mut store = Store::new();
    store.set_max_messages_per_branch(2);

    for i in 1..=5 {
        store.water(CH1, cn("general"), msg(i, i as i64 * 10, "lina", "msg"));
    }

    // Eviction is silent — no prune records
    assert!(store.history().is_empty());
    assert_eq!(store.branch(CH1).unwrap().unpruned_count(), 2);
}

#[test]
fn cap_preserves_last_activity_and_latest_known_id() {
    let mut store = Store::new();
    store.set_max_messages_per_branch(1);

    store.water(CH1, cn("general"), msg(1, 0, "lina", "first"));
    store.water(CH1, cn("general"), msg(2, 100, "lina", "second"));

    let branch = store.branch(CH1).unwrap();
    // Even though message 1 was evicted, the high-water marks survive
    assert_eq!(branch.last_activity, ts(100));
    assert_eq!(store.latest_known_id(CH1).unwrap(), 2);
}

#[test]
fn cap_works_across_multiple_branches() {
    let mut store = Store::new();
    store.set_max_messages_per_branch(2);

    for i in 1..=4 {
        store.water(CH1, cn("general"), msg(i, i as i64 * 10, "lina", "msg"));
    }
    for i in 1..=4 {
        store.water(CH2, cn("random"), msg(i + 100, i as i64 * 10, "ari", "msg"));
    }

    assert_eq!(store.branch(CH1).unwrap().unpruned_count(), 2);
    assert_eq!(store.branch(CH2).unwrap().unpruned_count(), 2);
    assert_eq!(store.branch(CH1).unwrap().earliest_unpruned().unwrap().id, 3);
    assert_eq!(store.branch(CH2).unwrap().earliest_unpruned().unwrap().id, 103);
}

/// The motivating concern: without a cap, a hot channel accumulates messages
/// without bound. With the cap, even a burst of messages stays within the
/// configured limit.
#[test]
fn cap_bounds_growth_under_sustained_load() {
    let mut store = Store::new();
    store.set_max_messages_per_branch(100);

    // Simulate 10000 messages on one channel
    for i in 1..=10_000u64 {
        store.water(CH1, cn("general"), msg(i, i as i64, "lina", "msg"));
    }

    assert_eq!(store.branch(CH1).unwrap().unpruned_count(), 100);
    assert_eq!(store.branch(CH1).unwrap().earliest_unpruned().unwrap().id, 9901);
    assert_eq!(store.branch(CH1).unwrap().latest().unwrap().id, 10000);
    assert_eq!(store.branch(CH1).unwrap().message_count, 10_000);
}

// ====================================================================
// Full ledger serde round-trip with bounded types
// ====================================================================

/// The serialized format is backward-compatible: Author, Content, and
/// ChannelName serialize as bare strings, identical to the pre-newtype
/// format. A ledger written by the old code deserializes into validated
/// types, and a ledger written by the new code deserializes into the old
/// format. This test pins the wire format.
#[test]
fn bounded_types_are_wire_compatible() {
    let mut store = Store::new();
    store.water(CH1, cn("general"), msg(1, 0, "lina", "hello"));
    store
        .save(std::env::temp_dir().join("entmoot-compat-test.json"))
        .unwrap();

    let json = serde_json::to_string_pretty(&store).unwrap();

    // Author, content, and channel_name appear as bare strings, not wrapped objects
    assert!(json.contains("\"lina\""), "author not serialized as bare string");
    assert!(json.contains("\"hello\""), "content not serialized as bare string");
    assert!(
        json.contains("\"general\""),
        "channel_name not serialized as bare string"
    );

    // And it deserializes back
    let reloaded: Store = serde_json::from_str(&json).unwrap();
    let branch = reloaded.branch(CH1).unwrap();
    assert_eq!(branch.earliest_unpruned().unwrap().author, "lina");
    assert_eq!(branch.earliest_unpruned().unwrap().content, "hello");
    assert_eq!(branch.channel_name, "general");
}
