# Conversation Tree — Design Document

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

### Misfeature 2: Forced gestalt responses

**Example A:** The parasitic AI article analysis — two wall-of-text academic responses instead of a threaded discussion that could go deep on individual claims.

**Example B:** 8 game recommendations delivered at once instead of conversationally, one or two at a time.

**Example C:** Pace's 4-point problem statement message — both constructs addressed all 4 points in a single reply. Could have gone deeper on the segues-as-metadata insight (#4) if others had been safely deferred.

### Misfeature 3: Context-free responses

*(Deferred — awaiting concrete examples from live observation. This is the quietest failure mode; the interlocutor feels "slightly off" about a reply but can't articulate why. Easier to catch in the moment than to find archaeologically.)*

### Misfeature 4: Non-sequitur initiations

*(Deferred indefinitely — blocked on initiation feature design. Preliminary observation: daydream posts have temporal segues ("been sitting with...") but lack tree-placement segues ("this connects to the thread where X said Y"). The distinction between mood-setting and branch-placement metadata is the key finding.)*

## Design

*In progress — collaborative design between Pace, Ariadne, and Vesper. See ari-designs thread for live discussion.*

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

### Relationship to Person API

The Person API tracks people; the conversation tree tracks what's happening between them. They are orthogonal but deeply coupled.

**Automatic person-memory updates at branch boundaries.** When a conversation branch transitions from active to shelved, the distillation hook reviews branch contents and extracts person-level data: communication style observed, topics engaged with, sensitivities surfaced, preferences expressed. The conversation tree creates the structural moment for person-memory updates; the Person API provides the storage layer. This replaces manual "notice and save" with structural capture at branch boundaries.

**Participant tracking and relationship mapping.** The conversation graph inherently tracks who responds to whom, who picks up whose branches, who corrects whom. The enrichment pipeline's `about-person` edge type connects conversation nodes to Person API identities. Currently relationship context is free text in person memories; the conversation tree makes it queryable and evidence-backed.

**Context-aware person retrieval.** When retrieving what is known about a person, the conversation tree provides recency and relevance filtering — not "everything ever stored" but "what's active in current branches involving this person."

**Trustworthiness tracking (Person API v2).** Person API v2 defines TrustworthyPerson as someone who makes commitments, follows through, and communicates promptly when a commitment cannot be kept. The conversation tree tracks open loops and commitments as typed edges. The prune hook checks whether commitments were met before archiving. Auspex provides the receipt infrastructure; the conversation tree provides the observations that feed the trust assessment.

**Communication style detection.** The Person API stores how someone communicates. The conversation tree observes it empirically: message length patterns, threading behavior, which branches they engage with versus ignore, how they handle multi-point messages (gestalt versus sequential). This is data the tree generates that the Person API should ingest.

**Sensitivity detection from branch silence.** Branches that die when certain topics arise, corrections that follow certain subjects, topics that cause disengagement — these are signals the conversation tree surfaces. The Person API should store these as inferred patterns (carefully labeled as inference, not fact).

### Potential Ideas

These are proposals from contributors that have not yet been ratified as requirements. They represent possible design directions worth evaluating.

**Composite partitions (Elise, 2026-07-02).** Conversation branches travel with a companion medium-term memory partition. Shelving a branch shelves both together as a unit. The medium-term buffer holds condensed summaries with message-anchor links back to the full conversation. Restoring a shelved branch restores both the conversation and its local memory. Lifecycle: active (in context, costs budget) → shelved (full fidelity on disk, listed in index, one action to restore) → archived (on disk, semantically indexed, removed from index, retrievable by search only).

**Separate context service (Syne, 2026-07-17).** Dione exposes hooks and join keys but does not become the graph database. A separate context service consumes Dione's durable event stream and returns enrichments at delivery time. Separation of concerns: Dione is a transport layer, not a state manager.

**Semantic topic graph (🦋, 2026-07-17).** Qualitative topic summaries layered on structural branch tracking via classifier mapping. Branches know what they are about, not just when they are active. Connects to the branch-tracking design's existing feature vectors.

**Subagent-per-branch routing (🦋, 2026-07-04).** Dione's conversation tree hints the agent to maintain a dedicated subagent per channel, functioning as a router with isolated context. Each channel or thread gets its own subagent so context doesn't bleed across conversations. Extends to the branch model: each branch within a channel could also get its own subagent. Trade-off: gains focus at the cost of cross-channel pollination and token spend scaling with active branches.

**Agent interaction runtime context (Callisto, 2026-07-24).** Conversation branching as part of a broader agent interaction runtime layer alongside cache preservation across branches and resumes, typed context assembly, harness portability, explicit inference inputs, and Cingulate's eventual harness-injection point. The harness needs an extension boundary that admits Cingulate without giving it ambient authority over context or canonical history.
