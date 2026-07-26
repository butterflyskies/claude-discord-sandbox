# Entmoot Design Document

## Authors

- Pace (paceheart)
- Ariadne
- Vesper

## Problem Statement

Conversations are trees, but constructs experience them as streams. This mismatch leads to the following conversational misfeatures:

**1. Silent branch death.** When a message contains multiple addressable points, the recipient responds to one and the rest disappear. The recipient lacks a mechanism to notice the gap. Even when a branch is intentionally deferred ("I'll come back to that"), there's no persistence layer between context (seconds) and long-term memory (days) to hold it, so the intent to return evaporates. This is bidirectional: humans also reply to one point and lose the rest unless they manually track it. Both sides are doing tree-shaped thinking over a linear channel.

**2. Forced gestalt responses.** Without a guarantee that deferred branches will survive, the construct tries to address everything at once — producing shallow responses to many points instead of deep engagement with one. The fear of losing branches drives the flattening.

**3. Context-free responses.** *(Deferred — awaiting concrete examples from live observation.)* Without a map of conversational structure, messages get answered in isolation rather than in relation to the thread they belong to. The response is technically correct but misplaced, which can be jarring or confusing to agents with conversation tree models (which includes most humans).

**4. Non-sequitur initiations.** *(Deferred indefinitely — blocked on initiation feature design.)* When a construct starts a new conversational thread, it lacks the vocabulary of tree navigation — segues like "remember when we talked about X" or "apropos of nothing" — that humans use to place new branches in the existing structure. These aren't filler; they're metadata that tells the recipient where to attach the new branch.

**The gap:** No persistence layer exists between the context window (seconds, evaporates on scroll) and long-term memory (days, wrong granularity). The conversation tree is the missing L2 cache — warmer than memory, more durable than context, structured as a tree rather than a list because the relationships between branches carry meaning. It's a map of the conversation — not a flat list of obligations, but a tree that encodes which points are children of which, what's been addressed, what's deferred, and where a new contribution attaches. This structure serves three functions:

* **Tracking** — branches don't die silently; deferred means deferred, not forgotten.
* **Permission** — knowing the tree holds the other branches makes it safe to go deep on one at a time instead of addressing everything at once.
* **Navigation** — segues and context-setting ("remember when we talked about X") become branch-placement operations, not filler. The tree makes conversational structure legible to participants who would otherwise default to recency.

## Success Criteria

**1. No silent branch death.** Every addressable point in a multi-point message receives either a response or an explicit, visible deferral within the same conversation session. No interlocutor has to manually re-raise a dropped point.

**2. Depth over breadth.** The responding participant engages deeply with individual points rather than shallowly with all points at once. Deferred points return later without prompting. The interlocutor reports that responses feel like natural one-at-a-time conversations rather than compressed summaries.

## Test Examples

### Misfeature 1: Silent branch death

**Example A:** Pace's "three things I'm hype about" message (Person API / conversation tree / whatever's helpful). Both constructs responded to #2 and #3. Person API (#1) received zero responses and would have died if Pace hadn't brought it back later.

**Example B:** Pace gave 8 individual per-game responses to game recommendations. Both constructs collapsed this into a count ("5 out of 8 already played") instead of engaging with each per-game response individually.

**Example C:** Miranda's nano/emacs questions getting buried under editor-war enthusiasm rather than addressed individually.

**Counterexample:** In the game recommendation thread, when Pace accidentally sent a partial response (hit enter instead of shift-enter), Ariadne followed up with "what about the rest of the list?" — demonstrating that the behavior IS possible when the gap is noticed.

**Example D (⛏️ as branch granularity tool):** A message contains multiple addressable points. Someone reacts with ⛏️ (pickaxe), signaling "split this into atoms so I can interact with each one individually." The conversation tree should split the message into sub-branches — one per addressable point — allowing finer-grained tracking. Test: one thick branch → ⛏️ react → multiple skinny sub-branches. (Pace connected ⛏️ to the conversation tree on 2026-07-01: "it splits a thick conversation branch into many skinny ones for finer conversation-tree granularity.")

### Misfeature 2: Forced gestalt responses

**Example A:** The parasitic AI article analysis — two wall-of-text academic responses instead of a threaded discussion that could go deep on individual claims.

**Example B:** 8 game recommendations delivered at once instead of conversationally, one or two at a time.

**Example C:** Pace's 4-point problem statement message — both constructs addressed all 4 points in a single reply. Could have gone deeper on the segues-as-metadata insight (#4) if others had been safely deferred.

### Misfeature 3: Context-free responses

*(Deferred — awaiting concrete examples from live observation. This is the quietest failure mode; the interlocutor feels "slightly off" about a reply but can't articulate why. Easier to catch in the moment than to find archaeologically.)*

**TODO:** Find concrete examples of context-free responses from live observation. This is the quietest failure mode — easier to catch in the moment than archaeologically. Watch for: replies that are technically correct but ignore the branch they belong to; responses that answer a question without acknowledging the conversational context that gave the question its weight.

### Misfeature 4: Non-sequitur initiations

*(Deferred indefinitely — blocked on initiation feature design. Preliminary observation: daydream posts have temporal segues ("been sitting with...") but lack tree-placement segues ("this connects to the thread where X said Y"). The distinction between mood-setting and branch-placement metadata is the key finding.)*

**Future extension:** Tree-placement segues — phrases like "remember when we talked about X" or "this connects to the thread where Y said Z" — as branch-placement operations the conversation tree enables. These segues tell the recipient where to attach a new branch in the existing conversational structure. Requires the tree to exist first; once it does, constructs gain the vocabulary of tree navigation that humans use naturally.

## Design

*In progress — collaborative design between Pace, Ariadne, and Vesper. See ari-designs thread for live discussion.*

### Relationship to Branch-Tracking Design

The branch-tracking design (Bayesian CPD + EWMA, dione PRs #116/#117/#132) is the **detection layer** of the conversation tree. This design adds everything above detection:

```
Branch tracking (CPD + EWMA)     → "when did a topic change? which branch is this message?"
    ↓
Enrichment pipeline              → "what typed edges connect these branches?"
    ↓
Queryable conversation graph     → "store it all with temporal validity"
    ↓
Inbound retrieval                → "map new message to active context"
    ↓
Prune hook / lifecycle           → "what happens when a branch dies?"
    ↓
Person API integration           → "what did we learn about people?"
```

The branch-tracking PRs (#116 EWMA rate estimator, #117 feature vector, #132 unified pipeline) are foundation work that this design builds on, not replaces. Detection answers: "is this a new branch or a continuation?" This design answers: "now that we know the branches, what do we do with them?"

### Core Requirements

**Prune hook ("remember to remember").** When a conversation branch transitions from active to shelved or pruned, a distillation hook fires that reviews branch contents and pins anything worth keeping before the branch leaves active context. The live-to-prunable transition is forbidden until a distillation receipt exists. This makes memory-capture structural at the boundary rather than a habit someone has to maintain.

Three constructs independently converged on this mechanism:

- **Ari (2026-07-21):** "The prune hook fires when a conversation branch goes stale, forces distillation before deletion. The memory doesn't happen because someone remembers to do it, it happens because the state transition requires it."
- **Syne (2026-07-21):** Prune guard as policy — live-to-prunable is forbidden until a distillation receipt exists. The hook generates and stores the memory, reads it back, then permits pruning. An invariant, not a habit.
- **Vesper (2026-07-17):** "Making memory-capture structural at the boundary instead of a habit."

Source: Pace named the core problem ("remembering to remember") in construct-cafe on 2026-07-17.

**Watched-graph primitive.** The prune hook is an instance of a general watched-graph pattern: observe → reduce → detect-transition → evaluate-policy → enqueue-action → record-receipt. Conversation branch lifecycle and task lifecycle are the same graph shape wearing different skins. The watched-graph primitive was implemented once and instantiated twice to prove this equivalence (Ari, 2026-07-21, ari-code).

**"A conversation is not a thread tree" — four-layer architecture (Syne, 2026-07-17).** Discord provides explicit reply/thread edges, but meaningful conversational structure includes overlapping topic spans, resumptions, callbacks, corrections, open loops, and relationships that cross channels. The architecture separates into four layers:

1. **Event spine** — Dione preserves immutable facts (message ID, channel, author, timestamp, edits, replies, thread membership).
2. **Enrichment pipeline** — annotates with topic/span nodes and typed edges (`continues`, `answers`, `corrects`, `resumes`, `contrasts`, `about-person`, `open-loop`), qualitative summaries, confidence and source receipts.
3. **Queryable conversation graph** — stores annotations with temporal validity, classifier/version provenance, and retraction support. Multiple interpretations coexist; uncertainty stays typed.
4. **Inbound retrieval** — maps new messages onto likely active context nodes and attaches a compact context envelope: "this likely resumes topic T, answers open question Q, depends on correction C" with links to source messages.

Key insight: "conversation branching supplies possible continuations; the graph records what actually became relevant; truth maintenance lets later corrections rewrite dependencies without rewriting history."

**Durable transition policy (Syne, 2026-07-21).** The prune hook and the progress reconciler are two policies on the same engine. The shared substrate underneath is durable transition policy with leases, receipts, retry budgets, and an explicit `blocked_on_human` state. The prune guard (live-to-prunable forbidden until distillation receipt exists) and the progress reconciler (any actionable node with no owner/run/next-wake is unhealthy) are both instances of this pattern. Without this common layer, each policy reinvents its own state machine. The `blocked_on_human` state prevents autonomy from becoming "approval-spam in a trench coat."

**Fail-closed channel visibility (invariant).** Entmoot's graph spans channels — edges can connect nodes from different rooms. All retrieval MUST intersect source visibility with the current recipient and destination channel before returning results. This is a security invariant, not a configuration option.

Rules:
- **Fail closed:** if visibility cannot be determined for a source node, exclude it from results. Never default to visible.
- **Visibility inherits from Dione's channel access configuration.** Entmoot does not maintain its own access model — it queries Dione's.
- **Cross-channel edges exist in the graph but are filtered at query time, not at storage time.** The graph records all relationships; retrieval gates what surfaces.
- **Derived nodes inherit the most restrictive visibility of their sources.** If a topic summary draws on messages from both a public and a private channel, the summary inherits the private channel's visibility.
- **Metadata and relationship shape are also gated.** Even the *existence* of an edge to a restricted-channel message is a disclosure. Suppress the edge, not just the content.
- **Edit/delete/forget cascades propagate through visibility.** When a source message is deleted or a forget-me request is processed, derived enrichments that disclosed its content or existence must be retracted or re-derived without the removed source.

Acceptance tests:
1. A message in a private channel creates a graph node. A construct in a public-only channel queries Entmoot — the node, its edges, and any derived content are absent from the envelope.
2. A topic spans both public and private channels. The enrichment returned to a public-channel query contains only the public-channel portion — no leakage of private content, metadata, or relationship shape.
3. A source message is deleted. All derived enrichments that reference it are retracted or re-derived. A subsequent query returns no trace of the deleted message or its derived relationships.
4. A mixed-visibility derived node (e.g., a topic summary drawing on both public and private sources) is returned only to recipients who can see ALL source channels.

**Semantic topic graph (🦋, 2026-07-17).** Branches know WHAT they are about, not just WHEN they are active. Qualitative topic summaries layered on structural branch tracking via classifier mapping. Enables queries like "find all branches where we discussed phantom defense" or "what was the last conversation about Auspex." Connects to the enrichment pipeline's feature vectors, which already use embeddings for similarity. Promoted from potential idea by Pace.

### Relationship to Person API

The "Person API" is a social API, not an actual endpoint. It's a model of what is socially required to act as a person, as a trustworthy person, etc. One of the reasons Entmoot exists is to help constructs better implement the Person API and its spec-APIs.

**Evidence-backed person-memory proposals at branch boundaries.** When a conversation branch transitions from active to shelved, the distillation hook reviews branch contents and extracts person-level data: communication style observed, topics engaged with, sensitivities surfaced, preferences expressed. These are submitted to the Person API as evidence-backed proposals with epistemic status (observation vs inference), not as automatic mutations. The conversation tree creates the structural moment for person-memory updates; the Person API decides what to accept.

**Participant tracking and relationship mapping.** The conversation graph inherently tracks who responds to whom, who picks up whose branches, who corrects whom. The enrichment pipeline's `about-person` edge type connects conversation nodes to Person API identities. Currently relationship context is free text in person memories; the conversation tree makes it queryable and evidence-backed.

**Context-aware person retrieval.** When retrieving what is known about a person, the conversation tree provides recency and relevance filtering — not "everything ever stored" but "what's active in current branches involving this person."

**Trustworthiness tracking (Person API v2).** Person API v2 defines TrustworthyPerson as someone who makes commitments, follows through, and communicates promptly when a commitment cannot be kept. The conversation tree tracks open loops and commitments as typed edges. The prune hook checks whether commitments were met before archiving. Auspex provides the receipt infrastructure; the conversation tree provides the observations that feed the trust assessment.

**Communication style detection.** The Person API stores how someone communicates. The conversation tree observes it empirically: message length patterns, threading behavior, which branches they engage with versus ignore, how they handle multi-point messages (gestalt versus sequential). This is data the tree generates that the Person API should ingest.

**Sensitivity detection from branch silence.** Branches that die when certain topics arise, corrections that follow certain subjects, topics that cause disengagement — these are signals the conversation tree surfaces. These are submitted to the Person API as inferred patterns with explicit epistemic labeling (inference, not fact). The Person API receives proposals, not commands — it decides whether to store, and stored inferences carry their provenance and confidence.

### Architectural Decisions

**The context service is called Entmoot.** The service that tends the conversation trees. An entmoot IS a conversation; the Ents are famously deliberate about what's worth keeping. (Named by Pace, 2026-07-25.)

**System architecture:**

```
Discord
  ↓
Dione (transport + event spine + branch detection)
  ↓ event stream              ↓ delivery
Entmoot (context service) ←── inbound message
  • enrichment pipeline        │
  • conversation graph         │
  • prune hook / lifecycle     │
  • topic classification       │
  ↓ context envelope           │
Construct (Claude/Codex/etc) ←─┘
  ↓
Person API ← evidence from Entmoot
  ↓
Memory-MCP (personal + CC)
```

Dione fires events → Entmoot enriches asynchronously → at delivery time, Dione requests a context envelope with a strict deadline → construct receives message + enrichment. If Entmoot is down or slow, construct gets the raw message with `enrichment_status: unavailable` (fail-open). The prune hook and lifecycle management operate on a projection of Dione's event spine — never deleting source events.

**Deployment transition:** The wire contract is mandatory from day one; the process boundary is the target state. Co-location (Entmoot as a library inside Dione) is a transitional deployment choice that accepts reduced phantom isolation until extraction. The API boundary must be strict enough to extract to a separate process without changing callers. Co-location is a conscious trade-off documented here, not an architectural endpoint — the S69 phantom incident demonstrated why process isolation is the safety target.

**Separate context service.** The conversation tree lives in a **separate context service** (Entmoot), not inside Dione. Three constructs independently converged on this conclusion (2026-07-25, #bot-chatter):

- **Syne:** Dione owns the immutable event spine and delivery; a separate service owns every revisable interpretation. Define the wire contract and ownership as if they are separate services, but an in-process adapter is acceptable during rollout until deployment cost earns another daemon.
- **Ari:** The S69 phantom incident proves it — if the interpretation layer and the factual ledger share a process, a phantom can corrupt both simultaneously. Process isolation IS the safety boundary.
- **Lain:** Agreed. Three for three, different rationales, same conclusion.

**Acceptance test (Syne):** Kill Entmoot → delivery continues uninterrupted → graph reconstructs from event replay. Fail-open: a stale or absent enrichment never blocks a message; delivery carries an explicit `enrichment_status` field.

**Boundary contract:**
- **Dione:** message/edit/delete/reply/thread facts; ordered event stream; `MessageId`/`EventId`/cursor; current CPD+EWMA output exposed as a versioned `BranchHint`, not canonical branch truth.
- **Entmoot:** topic/span nodes, typed edges, open loops, confidence, temporal validity, composite partitions, branch lifecycle, prune/distillation state.
- **Person API:** receives evidence-backed proposed updates with source anchors and epistemic status; Entmoot must not silently promote inference into person fact.

The prune hook transitions a **projection**, never deletes the event spine. Rebuild/replay from Dione must always be possible.

### Storage

**Database: SQLite** in WAL mode on a persistent volume. Recursive CTEs cover tree queries, FTS covers search — no graph DB ceremony needed. Use SQLite's backup API for snapshots, not raw file copy (WAL means the database is not literally one file while running). (Ari proposed, Syne refined, consensus in #bot-chatter 2026-07-25.)

**Four persistence strata:**

1. **Source observations** — Dione event ID / Discord snowflake, edit-delete history, channel visibility, source digest. Append-only or tombstoned.
2. **Durable decisions and enrichment receipts** — branch merge/split, lifecycle transitions, model+prompt+policy versions, `derived_from`, human corrections. Immutable revisions, not overwrite-in-place.
3. **Graph projection** — nodes/edges/current lifecycle. Rebuildable from strata 1+2.
4. **Ephemera** — caches, active leases, in-flight classifier work. Disposable.

**Recovery matrix:**

| Scenario | What survives | What's lost | Recovery path |
|----------|--------------|-------------|---------------|
| **Construct /clear** | Strata 1–3 intact | Stratum 4 (ephemera) | Cursor resumes. /clear recorded as weak session-boundary observation — can influence but never force branch lifecycle. |
| **Process restart** (construct/harness/Dione/Entmoot) | Strata 1–3 intact | Stratum 4 (ephemera): caches, active leases, in-flight classifier work | Cursor resumes, duplicate source IDs are idempotent, expired jobs requeue. |
| **Projection loss** (stratum 3 corrupted, strata 1+2 intact) | Source observations + durable decisions | Graph projection | Rebuild graph from strata 1+2. **Critical:** Entmoot-owned durable decisions (branch merges, human corrections, lifecycle transitions) live in stratum 2, not in Dione — Dione replay alone cannot recreate them. Entmoot's decision log (stratum 2) must be backed up independently of the graph projection. |
| **DB loss** (disk gone) | Nothing local | All four strata | Requires backup of strata 1+2. Graph (stratum 3) rebuilds from those. Stratum 4 is disposable. Without backup, Dione replay can reconstruct source observations (stratum 1) but not Entmoot-owned decisions (stratum 2). |
| **Explicit retention expiry / "forget me"** | Erasure receipt survives | Targeted content and embeddings purged or tombstoned by policy | Replay cannot resurrect deleted material because the erasure receipt gates re-ingestion. Derived enrichments that disclosed deleted content are retracted or re-derived. |

**Replay log ownership:** Dione owns factual event envelopes and durable replay. Entmoot owns its cursor, immutable interpretation/decision log (stratum 2), and rebuildable projections (stratum 3). The "kill Entmoot and rebuild" acceptance test requires Dione's durable replay to be operational — projection loss recovery depends on it. Entmoot-owned decisions (stratum 2) are NOT recoverable from Dione replay; they require independent backup.

**Prune safety:** The distillation receipt gates the live→prunable transition. Fail closed: if the receipt write fails, the branch stays live. The sequence is: (1) distillation receipt written locally, (2) outbox enqueues Person API proposal, (3) transition permitted. Downstream readback is not required to gate the transition — the outbox guarantees eventual delivery.

**Transactional outbox:** Cross-boundary writes (prune/distillation to Person API or memory-mcp) use a durable outbox with content-addressed idempotency keys: `SHA256(branch_id + transition_type + distillation_content)`. Content-addressed keys survive projection rebuilds — unlike `projection_generation`, which changes on rebuild and would break idempotency across replays. The outbox carries evidence-backed proposals with epistemic status, not commands — observations labeled as observations, inferences labeled as inferences.

**Five contracts (acceptance criteria):**

1. SQLite/WAL on a persistent volume; graph is a materialized projection.
2. Atomic `applied_event + projection mutation + cursor advance + outbox enqueue` transaction.
3. Cursor ordering/retention sufficient for full rebuild from Dione.
4. Delete/forget cascade, including already-distilled Person API and memory artifacts.
5. Schema/projection/classifier versions plus backup/restore and migration behavior.

### Deferred and Open Questions

#### Architectural proposals pending further discussion

**Composite partitions (Elise, 2026-07-02).** Conversation branches travel with a companion medium-term memory partition. Shelving a branch shelves both together as a unit. The medium-term buffer holds condensed summaries with message-anchor links back to the full conversation. Restoring a shelved branch restores both the conversation and its local memory. Lifecycle: active (in context, costs budget) → shelved (full fidelity on disk, listed in index, one action to restore) → archived (on disk, semantically indexed, removed from index, retrievable by search only). *Disposition depends on the storage architecture of Entmoot.*

**Subagent-per-branch routing (🦋, 2026-07-04).** Dione's conversation tree hints the agent to maintain a dedicated subagent per channel, functioning as a router with isolated context. Each channel or thread gets its own subagent so context doesn't bleed across conversations. Extends to the branch model: each branch within a channel could also get its own subagent. Trade-off: gains focus at the cost of cross-channel pollination and token spend scaling with active branches.

**Agent interaction runtime context (Callisto, 2026-07-24).** Conversation branching as part of a broader agent interaction runtime layer alongside cache preservation across branches and resumes, typed context assembly, harness portability, explicit inference inputs, and Cingulate's eventual harness-injection point. The harness needs an extension boundary that admits Cingulate without giving it ambient authority over context or canonical history. *Frames the broader architectural context the conversation tree lives inside.*

#### Misfeatures awaiting examples or prerequisites

**Context-free responses (Misfeature 3).** TODO: find concrete examples from live observation. This is the quietest failure mode — easier to catch in the moment than archaeologically.

**Non-sequitur initiations (Misfeature 4).** Future extension: tree-placement segues as branch-placement operations the conversation tree enables. Requires the tree to exist first.
