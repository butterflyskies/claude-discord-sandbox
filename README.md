# Entmoot

The conversation tree context service. Tends the trees so constructs can see the forest.

See [DESIGN.md](DESIGN.md) for the full architecture.

## MVP implementation

`src/` contains a first-slice Rust implementation — a much smaller thing than the full
design above. It does **not** implement the enrichment pipeline, the queryable
conversation graph, Rolodex integration, fail-closed cross-channel visibility, SQLite
persistence, or any of the other architecture described in DESIGN.md. Consider it
scaffolding to validate the core branch/staleness/prune lifecycle before the fuller
design gets built out.

MVP scope:

- One branch per Discord channel, auto-created on first message ("water").
- A branch tracks: channel id, channel name, earliest unpruned message, latest message,
  and a lifetime message count.
- Staleness is `now - last_message_timestamp`, computed on demand — no tick cron.
- `tend()` surfaces branches idle past a configurable threshold (default 10 minutes),
  returning the branch's **earliest unpruned** message (FIFO), not the latest.
- `prune(channel, message_id, disposition)` is the one prune code path; clears every
  unpruned message with id <= `message_id` and requires a disposition tag (`done`,
  `dropped`, `parked`). `prune_latest(channel, disposition)` is sugar that reads the
  branch's current latest known id and forwards to the same function.
- Persists to a JSON file on disk, atomically — a save that dies partway through
  leaves the previous ledger intact rather than a truncated one.
- Fully self-contained: no live Discord, no Dione dependency, no humans or sibling
  constructs in the loop. Tests inject synthetic `Message` values directly.

Explicitly **out of scope** for the MVP: sub-branch segmentation within a single
channel branch (what Pace calls "grunspreke") — prune is the MVP-level stand-in for
that granularity.

### Usage

```bash
# Lifecycle tests live in tests/integration.rs — plant/water, staleness, tend,
# prune, persistence, multi-channel, out-of-order arrival, the promise-gate
# scenarios (Hadamard/Clifford/Fredkin), the prune race condition, and
# disposition history. The atomic-save durability tests live in src/lib.rs,
# where they can reach the private crash seam.
cargo nextest run --workspace

cargo run -- --state ./entmoot-state.json water <channel_id> <channel_name> <message_id> <author> <content>
cargo run -- --state ./entmoot-state.json status
cargo run -- --state ./entmoot-state.json tend --threshold-minutes 10
cargo run -- --state ./entmoot-state.json prune-latest <channel_id> <done|dropped|parked>
cargo run -- --state ./entmoot-state.json history
```
