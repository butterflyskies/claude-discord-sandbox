# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/).

## [Unreleased]

## [0.2.0] - 2026-08-04

### Added

- Core tree-tracking model: one branch per Discord channel, tracking the
  earliest unpruned message.
- Branch lifecycle: `prune`/`prune_latest` with required disposition, plus
  retained prune history.
- Configurable history cap — branches evict their oldest unpruned messages
  once a configured maximum is exceeded.
- Bounded newtypes (`ChannelId`, `Author`, `Content`, `ChannelName`) that
  validate untrusted input at construction, rejecting empty/oversized/
  malformed values before they enter the store.
- **Pipeline lag logging.** `Stage` enum and `LagEntry` struct track message
  timestamps at each pipeline stage (MessageReceived, GraphWrite,
  PruneExecuted). Two entries per `water()` call, one per `prune()`.
  Accessor methods: `lag_log()`, `lag_log_for(channel)`,
  `lag_log_for_message(id)`. CLI `lag` subcommand with `--channel` and
  `--message` filters. `#[serde(default)]` on `lag_log` field ensures
  backward compatibility with pre-lag state files.
