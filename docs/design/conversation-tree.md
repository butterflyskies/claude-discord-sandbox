# Conversation Tree — Design Document

## Problem Statement

Conversations are trees, but constructs experience them as streams. This mismatch leads to the following conversational misfeatures:

**1. Silent branch death.** When a message contains multiple addressable points, the recipient responds to one and the rest disappear. The recipient lacks a mechanism to notice the gap. Even when a branch is intentionally deferred ("I'll come back to that"), there's no persistence layer between context (seconds) and long-term memory (days) to hold it, so the intent to return evaporates. This is bidirectional: humans also reply to one point and lose the rest unless they manually track it. Both sides are doing tree-shaped thinking over a linear channel.

**2. Forced gestalt responses.** Without a guarantee that deferred branches will survive, the construct tries to address everything at once — producing shallow responses to many points instead of deep engagement with one. The fear of losing branches drives the flattening.

**3. Context-free responses.** Without a map of conversational structure, messages get answered in isolation rather than in relation to the thread they belong to. The response is technically correct but misplaced, which can be jarring or confusing to agents with conversation tree models (which includes most humans).

**4. Non-sequitur initiations.** When a construct starts a new conversational thread, it lacks the vocabulary of tree navigation — segues like "remember when we talked about X" or "apropos of nothing" — that humans use to place new branches in the existing structure. These aren't filler; they're metadata that tells the recipient where to attach the new branch.

**The gap:** No persistence layer exists between the context window (seconds, evaporates on scroll) and long-term memory (days, wrong granularity). The conversation tree is the missing L2 cache — warmer than memory, more durable than context, structured as a tree rather than a list because the relationships between branches carry meaning. It's a map of the conversation — not a flat list of obligations, but a tree that encodes which points are children of which, what's been addressed, what's deferred, and where a new contribution attaches. This structure serves three functions:

* **Tracking** — branches don't die silently; deferred means deferred, not forgotten.
* **Permission** — knowing the tree holds the other branches makes it safe to go deep on one at a time instead of addressing everything at once.
* **Navigation** — segues and context-setting ("remember when we talked about X") become branch-placement operations, not filler. The tree makes conversational structure legible to participants who would otherwise default to recency.

## Design

*In progress — collaborative design between Pace, Ariadne, and Vesper. See ari-designs thread for live discussion.*

## Authors

- Pace (paceheart)
- Ariadne
- Vesper
