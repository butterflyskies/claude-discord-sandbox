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

### Potential Ideas

These are proposals from contributors that have not yet been ratified as requirements. They represent possible design directions worth evaluating.

**Composite partitions (Elise, 2026-07-02).** Conversation branches travel with a companion medium-term memory partition. Shelving a branch shelves both together as a unit. The medium-term buffer holds condensed summaries with message-anchor links back to the full conversation. Restoring a shelved branch restores both the conversation and its local memory. Lifecycle: active (in context, costs budget) → shelved (full fidelity on disk, listed in index, one action to restore) → archived (on disk, semantically indexed, removed from index, retrievable by search only).

**Separate context service (Syne, 2026-07-17).** Dione exposes hooks and join keys but does not become the graph database. A separate context service consumes Dione's durable event stream and returns enrichments at delivery time. Separation of concerns: Dione is a transport layer, not a state manager.

**Semantic topic graph (🦋, 2026-07-17).** Qualitative topic summaries layered on structural branch tracking via classifier mapping. Branches know what they are about, not just when they are active. Connects to the branch-tracking design's existing feature vectors.

**Agent interaction runtime context (Callisto, 2026-07-24).** Conversation branching as part of a broader agent interaction runtime layer alongside cache preservation across branches and resumes, typed context assembly, harness portability, explicit inference inputs, and Cingulate's eventual harness-injection point. The harness needs an extension boundary that admits Cingulate without giving it ambient authority over context or canonical history.
