# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/).

## [Unreleased]

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
