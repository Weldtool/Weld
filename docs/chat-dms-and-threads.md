# Chat: Add DMs, Remove Threads — Design Notes

> Status: **design only, not yet implemented.** Pre-release, **no real users and
> no production data**, so destructive schema changes (dropping the thread column)
> are safe.

## Decisions

1. **Add Direct Messages (DMs).** A DM is a 1:1 conversation that requires *no*
   channel ceremony — no naming, no manual member adds. Pick a person →
   find-or-create the conversation → start talking. Surfaced as a dedicated
   **"Direct Messages"** section in the sidebar, listing people, not `#channels`.
2. **Remove threads entirely.** Threads add a parallel mechanism for "focused
   discussion about a thing" that Weld already solves better via **task linking**
   (`#WD-123` refs, GitHub activity cards, the `task context` join). Since nothing
   has shipped to users, we rip the thread logic out rather than leave it dormant.
3. **Channels stay as-is.**

The chat model becomes: **Channels + DMs.** That's it.

---

## Part A — Remove Threads

Threads are modeled as `messages.thread_parent_id` (self-referential FK) plus a
`get_thread` service and a `ThreadPanel`. Touchpoints found in the codebase:

### Database
- Migration `crates/weld-server/migrations/20260503193329_create_chat_schema.sql`
  defines `thread_parent_id UUID REFERENCES messages(id) ON DELETE CASCADE` and
  `messages_thread_parent_idx`.
- **Approach:** add a **new** migration (e.g. `..._drop_message_threads.sql`) that
  `DROP INDEX messages_thread_parent_idx;` and
  `ALTER TABLE messages DROP COLUMN thread_parent_id;`. Prefer a new migration over
  editing the old one so migration history stays linear and the committed `.sqlx`
  cache regenerates cleanly. (Per project rule: always migrate, never manual SQL.)

### Backend (`crates/weld-server`)
- **Delete** `src/services/chat/get_thread.rs`.
- `src/services/chat/mod.rs` — remove `mod get_thread;` and the
  `pub use get_thread::{ThreadView, get_thread};`.
- `src/services/chat/post_message.rs` — remove the `thread_parent_id` field on
  `PostMessageInput`, the parent-validation block ("Thread parent is not in this
  channel" / "Cannot reply to a thread reply"), and the column from the INSERT +
  RETURNING.
- `src/services/chat/list_messages.rs`, `edit_message.rs`, `delete_message.rs`,
  `helpers.rs` — drop `thread_parent_id` from SELECT/RETURNING column lists and
  any struct mapping.
- `src/routes/chat.rs` — remove `ThreadView`/`ThreadResponse`, the `get_thread`
  handler, `thread_parent_id` fields on request/response structs, and the
  `thread_parent_id` key in the delete payload.
- `src/routes/mod.rs` — remove the thread route registration.
- `src/routes/ws.rs` — drop `thread_parent_id` from the message SELECT (~line 278).
- **SELECT lists elsewhere that pull message columns** also reference the field
  and must be updated: `services/github/helpers.rs`,
  `services/github/activity_feed.rs`, `services/study/agent/tools.rs`,
  `services/study/telemetry.rs`, `services/study/access_code.rs`.

### Shared model
- `crates/weld-core/src/models/message.rs` — remove the `thread_parent_id` field.

### Web (`apps/web`)
- **Delete** `src/lib/components/chat/ThreadPanel.svelte`.
- Update `src/routes/chat/[channelId]/+page.svelte` (thread panel open/close state),
  `src/lib/components/chat/MessageList.svelte`, `Message.svelte` ("reply in thread"
  affordance + reply counts), `Composer.svelte` (thread-reply mode).
- `src/lib/chat.svelte.ts` — remove thread state/actions.
- `src/lib/api/types.ts` + `src/lib/api/index.ts` — remove `thread_parent_id` from
  message types and the `getThread` client call.
- `src/lib/chat.test.ts` — drop thread test cases.
- `src/lib/study-snapshot.ts` — remove `thread_parent_id` from any message shape.

### After removal
- Run `make pr` (regenerates the committed `.sqlx` offline cache via
  `cargo sqlx prepare`, runs fmt/clippy/tests). Compile-time-checked queries will
  fail loudly if a `thread_parent_id` reference is missed — use that as the
  completeness check.

---

## Part B — Add Direct Messages

### What already exists
- `channels.is_direct BOOLEAN` is in the schema and on the `Channel` model.
- `create_channel` already accepts `is_direct` — **but** it requires a non-empty,
  name-validated channel name (letters/numbers/`-`/`_`). DMs shouldn't need a name.
- `mark_channel_read` exists, so DM read-state reuses channel read tracking.
- `ChannelSidebar` / `ChannelSwitcher` already branch on `is_direct`.

### What to build

1. **Duplicate-proof find-or-create.** A DM between A and B must resolve to the
   *same* channel every time. Add a deterministic uniqueness key:
   - Add `channels.direct_key TEXT` (nullable; set only for DMs) with a partial
     unique index `WHERE is_direct`. Key = sorted participant UUIDs joined, e.g.
     `min(a,b):max(a,b)`.
   - New service `services/chat/get_or_create_dm.rs`: given the other user id,
     compute `direct_key`, `SELECT` existing → else create channel
     (`is_direct = true`, no name, both memberships) in one transaction.
   - Make `channels.name` **nullable** (or system-derived) so DMs don't carry a
     fake name. The DM's display label is the *other participant's* display name,
     resolved client-side.

2. **Route.** `POST /api/v1/teams/:teamId/dms { user_id }` → returns the channel
   (find-or-create). Team-scoped so DM access maps to shared team membership.

3. **Scope.** You can DM **people you share a team with** (natural boundary given
   personal teams + memberships). Enforce in the service.

4. **Frontend.**
   - **"Direct Messages" sidebar section**, separate from channels, listing DMs by
     the other participant's name + avatar, with unread badges (reuse read-state).
   - **"New message" people-picker** (members of your shared teams) that calls the
     find-or-create route and navigates to the conversation.
   - DM view reuses the existing message list/composer; just no channel header
     ceremony.

### Open questions
- **Group DMs** (you + 2+ others, no name)? Model already supports N members; the
  `direct_key` would extend to a sorted set of all participant ids. **Defer** —
  ship 1:1 first, revisit.
- **DM scope** confirmation — strictly shared-team, or global directory?
- **Read state** — confirm `mark_channel_read` covers per-user unread counts well
  enough for the DM/sidebar badges, or whether that needs hardening.

---

## Build order

1. Remove threads (Part A) — get to a clean Channels-only chat, `make pr` green.
2. DM schema (`direct_key`, nullable `name`) + `get_or_create_dm` service + route.
3. DM sidebar section + people-picker (frontend).
4. (Later) group DMs if wanted.
