# Aurora — Backend Improvements

This document summarises the backend changes that ship with the Aurora
red-theme edition of Valthrun.

## 1. Radar server (`radar/server`)

### Reliability
- **Client-ID counter fix.** The previous `wrapping_add(1)` was called
  *after* reading the counter, meaning IDs started at 0 instead of 1 and
  the first issued ID was a duplicate of the initial sentinel. Fixed to
  increment-first.
- **Automatic dead-subscriber eviction.** `PubSession::broadcast` now
  detects closed subscriber channels mid-broadcast and removes them
  from the session so the subscriber list does not leak.
- **Idle / stalled-publisher detection.** Sessions whose owner has not
  sent a state update within `idle_timeout` (default 30 s) are now
  treated as dead and closed automatically. Previously only clean
  disconnects and unbound-session timeouts were handled, so a crashed
  publisher would leave a zombie session indefinitely.
- **Reconnect window increased** from 120 s to 180 s to give publishers
  more time to restart (e.g. after a CS2 map change).
- **Session ID collision protection.** The session-id generator loops
  until it produces a truly unique id (collision chance for 8-char ids
  is negligible, but correctness is free).
- **Owner verification** for `NotifyRadarState` is performed inside the
  same lock as the broadcast, eliminating a TOCTOU race where a
  subscriber could briefly be allowed to publish state.

### Performance / Hardening
- **Per-session broadcast rate-limit** (default 60 msgs/s) prevents a
  misbehaving or malicious publisher from saturating all subscribers or
  the server event loop.
- **Non-blocking broadcasts.** Broadcast is now `try_send`; slow
  subscribers drop frames instead of stalling the publisher loop.
- **Lock-free metrics** via `AtomicU64`:
  - `total_connections`
  - `active_publishers` / `active_subscribers`
  - `messages_broadcast`
  - `sessions_created`
  - `uptime_secs`
- **Dedicated tick tasks** for session cleanup (1 s interval) and
  metrics logging (60 s interval), both spawned at server startup.
- **Channel capacity tuning**: per-client inbound/outbound queues are
  larger (64 / 64) on the server and (64 / 256) on the publisher client
  to better handle burst traffic on map changes.
- **Per-request access log** with method, path, status, and latency.
- **CORS headers** enabled by default so the web radar can talk to the
  WebSocket endpoint even when hosted on a different origin.

### New HTTP endpoints
| Method | Path      | Purpose                                                |
|--------|-----------|--------------------------------------------------------|
| GET    | `/health` | JSON metrics snapshot (for operators/load balancers)   |
| GET    | `/info`   | Server name, protocol version, and build version       |
| WS     | `/subscribe` | Existing — viewer WebSocket                         |
| WS     | `/publish`   | Existing — publisher WebSocket                      |

### Protocol
- **Bumped to v3.** The handshake success response now includes an
  optional `serverName` field. The server identifies itself as
  `"Aurora"`. Older v2 clients are rejected with a clear
  `ResponseIncompatible` message.
- Session IDs are now **8 characters long** (up from 6) and
  human-friendly (ambiguous `0/O/1/l/I` characters are excluded).
- Auth tokens are now **24 characters** (up from 12).

## 2. Radar web client (`radar/web/src/backend`)

The `SubscriberClient` TypeScript class was rewritten:

- **Automatic reconnection** with exponential back-off
  (250 ms → 500 ms → 1 s → 2 s → 4 s → 8 s, capped at 15 s).
- **Distinct `reconnecting` state** surfaced to the UI so it can show
  the user what is happening rather than sitting on a spinner.
- **Proper shutdown semantics**: `disconnect()` sends an orderly
  `disconnect` frame, clears the reconnect timer, and transitions to
  `disconnected`.
- **Message-dispatch safety**: a malformed frame no longer crashes the
  event loop; it logs a warning and keeps the socket alive.
- **`view.count` event**: the `notify-view-count` message is now
  surfaced via the event emitter, and the radar page renders a glowing
  live viewer-count pill next to the map name.
- **Richer `UpdateStatistics`** — tracks min/max interval and
  updates-per-second in addition to the EMA.
- **Logger** gracefully degrades when no global `log` is defined.
- **Pending outbound queue**: commands issued before the socket is OPEN
  are buffered and flushed once the connection is ready.

## 3. Publisher client (`radar/client`)

- Increased channel sizes for smoother burst handling.
- `send_message` no longer closes the transport on a full queue; it
  drops (radar state updates are non-critical) and logs once if the
  channel closes.
- Banner log on startup with Aurora branding.

## 4. Crate metadata

- All workspace crates now have `authors = ["Aurora Team"]`,
  descriptions that mention Aurora, and cleaned-up `Cargo.toml`s.
- Workspace version bumped to `1.0.0` and all internal path deps no longer
  pin hardcoded `0.5.x` versions (so the workspace resolves cleanly
  regardless of which version is in `[workspace.package]`).

---

These changes preserve wire compatibility for the viewer state stream
(`RadarState`), so no changes are required in radar renderer code
beyond what is already updated in this branch.
