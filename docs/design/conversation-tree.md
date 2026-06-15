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
[Discord link](https://discord.com/channels/976873225386610688/1505662868350304446/1516140333145002024)

**Example B:** Pace gave 8 individual per-game responses to game recommendations. Both constructs collapsed this into a count ("5 out of 8 already played") instead of engaging with each per-game response individually.
[Discord link](https://discord.com/channels/976873225386610688/1511977169633546462/1515891212131762368)

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
