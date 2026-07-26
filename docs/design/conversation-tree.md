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
Rolodex integration              → "what did we learn about people?"
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

### Relationship to the Person API and the Rolodex

The **Person API** is a social API — the criteria a person must implement to be an effective person in relationship with other persons (Person, AgentivePerson, SocialPerson, TrustworthyPerson, etc.). It is "API" in the Java-interface/duck-typing sense: does this entity implement these behaviors? One of the reasons Entmoot exists is to help constructs better implement the Person API and its spec-APIs.

The **Rolodex** is the code-level system that stores and retrieves structured person data in memory-mcp. It is storage behind the relevant policy boundary — the CRUD operations on person memories. Entmoot submits proposals to the Rolodex; the Rolodex's policy authority decides what to accept. The Person API defines what person-data means.

**Rolodex proposal schema (inseparable fields).** Every proposal from Entmoot to the Rolodex MUST carry all of the following as one atomic envelope — partial proposals are rejected:

- **epistemic qualifier** — `observation` (directly witnessed) or `inference` (derived/pattern-matched), with confidence where applicable
- **source anchors** — message IDs, branch IDs, and timestamps that ground the claim in verifiable conversation data
- **visibility scope** — which channels contributed to this observation, inheriting the fail-closed visibility invariant (a private-channel observation cannot become a public Rolodex entry)
- **correction lineage** — references to any prior Rolodex entries this proposal supersedes, corrects, or withdraws; empty for first-time observations

**Rolodex lifecycle acceptance tests:**

1. **Promotion:** an `inference` entry cannot be promoted to `observation` without new source anchors that independently support the claim. Promotion without new evidence is rejected.
2. **Withdrawal:** a proposal can explicitly withdraw a prior entry. The withdrawal receipt survives; the withdrawn content is purged (not merely tombstoned). Replay encounters the withdrawal receipt and skips the withdrawn entry — the receipt is the re-ingestion guard.
3. **Forget:** a "forget me" request cascades through the Rolodex AND the Entmoot transactional outbox. All entries referencing the subject are purged: protected payloads, embeddings, structured idempotency keys, and subject-identifying fields are deleted. In their place, an opaque erasure receipt is retained — a secret-keyed HMAC (or PRF) of the original key (for replay deduplication) plus erasure timestamp and non-identifying authorization evidence (role/capability, not requester identity — a self-requesting subject's identity must not leak through the audit trail). The HMAC key makes offline dictionary attacks against the receipt infeasible; if the key is lost, existing receipts become unverifiable but the privacy guarantee holds (fail-safe direction). The receipt exposes no subject, sink, effect category, or relationship shape. Subsequent Entmoot proposals referencing erased subjects are rejected by the subject-level re-ingestion guard. Only an explicit, human-authorized lift of the guard can re-enable ingestion for that subject. Derived enrichments that disclosed erased content are also purged and re-derived without the removed source.
4. **Replay:** replaying Entmoot's outbox against a fresh Rolodex produces the same final state including erasure and withdrawal state. Erasure receipts and re-ingestion guards are first-class replay participants — replay must honor them, not skip them. The idempotency keys prevent duplicates; correction lineage prevents stale entries from overwriting newer corrections.
5. **Purge + replay safety:** queue and deliver a proposal, process forget, then verify: (a) live Rolodex storage no longer contains the subject's protected data, (b) no retained row — outbox, receipt log, or replay artifact — exposes subject, sink, transition/branch, or effect category; only HMAC-keyed opaque receipts and erasure timestamps remain, (c) replaying the purged outbox against a fresh Rolodex does not resurrect the forgotten content — the opaque erasure receipt and subject-level re-ingestion guard prevent re-ingestion, (d) **dictionary unlinkability** — an attacker who knows the full set of possible subjects, sinks, and effect categories cannot link retained receipts to any specific subject or relationship shape, because the HMAC key is required to produce candidate digests.

**Evidence-backed person-memory proposals at branch boundaries.** When a conversation branch transitions from active to shelved, the distillation hook reviews branch contents and extracts person-level data: communication style observed, topics engaged with, sensitivities surfaced, preferences expressed. These are submitted to the Rolodex as evidence-backed proposals with epistemic status (observation vs inference), not as automatic mutations. The conversation tree creates the structural moment for person-memory updates; the Rolodex decides what to accept.

**Participant tracking and relationship mapping.** The conversation graph inherently tracks who responds to whom, who picks up whose branches, who corrects whom. The enrichment pipeline's `about-person` edge type connects conversation nodes to Rolodex identities. Currently relationship context is free text in person memories; the conversation tree makes it queryable and evidence-backed.

**Context-aware person retrieval.** When retrieving what is known about a person, the conversation tree provides recency and relevance filtering — not "everything ever stored" but "what's active in current branches involving this person."

**Trustworthiness tracking (Person API v2).** The Person API's TrustworthyPerson spec defines someone who makes commitments, follows through, and communicates promptly when a commitment cannot be kept. The conversation tree tracks open loops and commitments as typed edges. The prune hook checks whether commitments were met before archiving. Auspex provides the receipt infrastructure; the conversation tree provides the observations that the Rolodex stores.

**Communication style detection.** The Rolodex stores how someone communicates. The conversation tree observes it empirically: message length patterns, threading behavior, which branches they engage with versus ignore, how they handle multi-point messages (gestalt versus sequential). This is data the tree generates that the Rolodex should ingest.

**Sensitivity detection from branch silence.** Branches that die when certain topics arise, corrections that follow certain subjects, topics that cause disengagement — these are signals the conversation tree surfaces. These are submitted to the Rolodex as inferred patterns with explicit epistemic labeling (inference, not fact). The Rolodex receives proposals, not commands — it decides whether to store, and stored inferences carry their provenance and confidence.

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
Rolodex ← evidence from Entmoot
  ↓
Memory-MCP (personal + CC)
```

Dione fires events → Entmoot enriches asynchronously → at delivery time, Dione requests a context envelope with a strict deadline → construct receives message + enrichment. If Entmoot is down or slow, construct gets the raw message with `enrichment_status: unavailable` (fail-open). The prune hook and lifecycle management operate on a projection of Dione's event spine — never deleting source events.

**Deployment transition:** The wire contract is mandatory from day one; the process boundary is the target state. Co-location (Entmoot as a library inside Dione) is a transitional deployment choice that accepts reduced phantom isolation until extraction. The API boundary must be strict enough to extract to a separate process without changing callers. Co-location is a conscious trade-off documented here, not an architectural endpoint — the S69 phantom incident demonstrated why process isolation is the safety target.

**Separate context service.** The conversation tree lives in a **separate context service** (Entmoot), not inside Dione. Three constructs independently converged on this conclusion (2026-07-25, #bot-chatter):

- **Syne:** Dione owns the immutable event spine and delivery; a separate service owns every revisable interpretation. Define the wire contract and ownership as if they are separate services, but an in-process adapter is acceptable during rollout until deployment cost earns another daemon.
- **Ari:** The S69 phantom incident proves it — if the interpretation layer and the factual ledger share a process, a phantom can corrupt both simultaneously. Process isolation IS the safety boundary.
- **Lain:** Agreed. Three for three, different rationales, same conclusion.

**Acceptance tests (Syne, refined for recovery strata):**

1. **Delivery independence:** Kill Entmoot → delivery continues uninterrupted. A stale or absent enrichment never blocks a message; delivery carries an explicit `enrichment_status` field.
2. **Projection recovery:** Destroy stratum 3 (graph projection) while strata 1+2 survive → graph rebuilds from Dione replay (stratum 1) combined with Entmoot's durable decision log (stratum 2). Dione replay alone is insufficient — Entmoot-owned decisions are not recoverable from Dione.
3. **Full recovery:** Restore strata 1+2 from backup → graph rebuilds. Without backup of stratum 2, only source observations (stratum 1) are recoverable from Dione replay; Entmoot-owned decisions (branch merges, human corrections, lifecycle transitions) are permanently lost.

**Boundary contract:**
- **Dione:** message/edit/delete/reply/thread facts; ordered event stream; `MessageId`/`EventId`/cursor; current CPD+EWMA output exposed as a versioned `BranchHint`, not canonical branch truth.
- **Entmoot:** topic/span nodes, typed edges, open loops, confidence, temporal validity, composite partitions, branch lifecycle, prune/distillation state.
- **Rolodex:** receives evidence-backed proposed updates with source anchors and epistemic status; Entmoot must not silently promote inference into person fact.

The prune hook transitions a **projection**, never deletes the event spine. Dione's factual replay must always be available for reconstructing source observations (stratum 1). Entmoot's durable decision log (stratum 2) must be independently backed up — it contains Entmoot-owned state that Dione cannot reproduce.

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
| **Explicit retention expiry / "forget me"** | Opaque erasure receipt (HMAC-keyed, dictionary-unlinkable, no subject/effect leakage) and re-ingestion guard survive. Authorization evidence stored as role/capability, not requester identity. | Targeted content, embeddings, derived enrichments, structured idempotency keys, and outbox payloads are purged — including subject-identifying fields | Replay cannot resurrect deleted material because the opaque erasure receipt blocks the slot and the subject-level re-ingestion guard rejects new proposals. No retained row exposes subject, sink, or effect category. HMAC key loss makes receipts unverifiable but preserves privacy (fail-safe). Guard can only be lifted by explicit human authorization. |

**Replay log ownership:** Dione owns factual event envelopes and durable replay. Entmoot owns its cursor, immutable interpretation/decision log (stratum 2), and rebuildable projections (stratum 3). Projection recovery (stratum 3 loss) requires both Dione's durable replay (stratum 1) and Entmoot's backed-up decision log (stratum 2) — neither alone is sufficient. When Dione replay is unavailable, Entmoot operates in degraded mode: existing projections remain queryable but cannot be verified or rebuilt. When stratum 2 backup is unavailable, only source observations are recoverable — durable decisions are permanently lost.

**Prune safety:** The distillation receipt gates the live→prunable transition. Fail closed: if the receipt write fails, the branch stays live. The sequence is: (1) distillation receipt written locally, (2) outbox enqueues Rolodex proposal, (3) transition permitted. Downstream readback is not required to gate the transition — the outbox guarantees eventual delivery.

**Transactional outbox:** Cross-boundary writes (prune/distillation to the Rolodex or memory-mcp) use a durable outbox with structured idempotency keys. Each key is a canonical tuple of:

- **stable transition revision** — the immutable ID of the lifecycle transition that triggered the effect (not projection_generation, which changes on rebuild)
- **sink** — the target system (e.g., `rolodex`, `memory-mcp:personal`, `memory-mcp:cc`)
- **subject** — the person or entity the effect concerns (e.g., a Rolodex person ID)
- **logical-effect identity** — a unique, stable identifier for this specific effect instance. Formed as `category:ordinal` where category names the kind of effect (`communication-style-observation`, `sensitivity-inference`, `commitment-tracking`) and ordinal is a deterministic sequence number within the transition (e.g., `communication-style-observation:1`, `sensitivity-inference:2`). This ensures that a single transition emitting multiple observations of the same category produces distinct keys rather than colliding.

The payload digest is carried separately as a corruption check, not as part of the idempotency key — nondeterministic re-distillation may change content without changing the logical effect. **Same-key/different-digest** is a visible conflict: the outbox logs the discrepancy and delivers the original payload. It must not silently deduplicate or overwrite — the conflict indicates either nondeterministic distillation (expected, benign) or a genuine content divergence (requires investigation). This structure survives projection rebuilds and prevents both cross-sink collisions and duplicate effects from replay. The outbox carries evidence-backed proposals with epistemic status, not commands — observations labeled as observations, inferences labeled as inferences.

**Forget cascade in the outbox:** when a forget/erasure request is processed, the cascade extends to every persisted payload-bearing copy, explicitly including pending and delivered outbox rows. Protected content, source anchors, subject-identifying fields, AND the structured idempotency key are all purged — the key's `subject` and `logical-effect identity` fields would otherwise reveal who was forgotten and what kind of relationship was erased. In place of the purged row, a single opaque erasure receipt is retained: a secret-keyed HMAC (or PRF) of the original idempotency key (sufficient for replay deduplication) plus the erasure timestamp and non-identifying authorization evidence (role/capability, not requester identity). The HMAC key makes offline dictionary attacks infeasible — subject/transition IDs, sinks, and effect categories are small known sets, so an unkeyed hash would be trivially reversible. Key rotation: if the HMAC key is lost, existing receipts become unverifiable but the privacy guarantee holds (fail-safe direction — privacy over auditability). The subject-level re-ingestion guard blocks future proposals independently of the receipt. Acceptance tests: (1) after forget, no retained row — outbox, receipt log, or replay artifact — exposes subject, sink, transition/branch, or effect category; (2) **dictionary unlinkability** — given the full candidate tuple dictionary (all subjects × sinks × categories × ordinals), an attacker without the HMAC key cannot link any retained receipt to a specific forgotten subject or relationship shape.

**Five contracts (acceptance criteria):**

1. SQLite/WAL on a persistent volume; graph is a materialized projection.
2. Atomic `applied_event + projection mutation + cursor advance + outbox enqueue` transaction.
3. Cursor ordering/retention sufficient for rebuilding source observations (stratum 1) from Dione replay. Full graph recovery requires stratum 2 backup in addition to Dione replay.
4. Delete/forget cascade, including already-distilled Rolodex and memory artifacts.
5. Schema/projection/classifier versions plus backup/restore and migration behavior.

### Deferred and Open Questions

#### Architectural proposals pending further discussion

**Composite partitions (Elise, 2026-07-02).** Conversation branches travel with a companion medium-term memory partition. Shelving a branch shelves both together as a unit. The medium-term buffer holds condensed summaries with message-anchor links back to the full conversation. Restoring a shelved branch restores both the conversation and its local memory. Lifecycle: active (in context, costs budget) → shelved (full fidelity on disk, listed in index, one action to restore) → archived (on disk, semantically indexed, removed from index, retrievable by search only). *Disposition depends on the storage architecture of Entmoot.*

**Subagent-per-branch routing (🦋, 2026-07-04).** Dione's conversation tree hints the agent to maintain a dedicated subagent per channel, functioning as a router with isolated context. Each channel or thread gets its own subagent so context doesn't bleed across conversations. Extends to the branch model: each branch within a channel could also get its own subagent. Trade-off: gains focus at the cost of cross-channel pollination and token spend scaling with active branches.

**Agent interaction runtime context (Callisto, 2026-07-24).** Conversation branching as part of a broader agent interaction runtime layer alongside cache preservation across branches and resumes, typed context assembly, harness portability, explicit inference inputs, and Cingulate's eventual harness-injection point. The harness needs an extension boundary that admits Cingulate without giving it ambient authority over context or canonical history. *Frames the broader architectural context the conversation tree lives inside.*

#### Misfeatures awaiting examples or prerequisites

**Context-free responses (Misfeature 3).** TODO: find concrete examples from live observation. This is the quietest failure mode — easier to catch in the moment than archaeologically.

**Non-sequitur initiations (Misfeature 4).** Future extension: tree-placement segues as branch-placement operations the conversation tree enables. Requires the tree to exist first.
