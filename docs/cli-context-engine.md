# Weld CLI + Context Engine — Design Notes

> Status: **design only, not yet implemented.** This captures the plan so we
> can build it later. Nothing here is committed code.

## Motivation

Today the study build ships an **in-app, context-scoped AI agent** (`services/study/agent/`).
That was the right call for a controlled experiment, but it is **not** the
long-term direction. We don't want a chatbot living inside the app.

What we want instead: Weld becomes the **context substrate** — the single place
that already knows the tasks, chat, GitHub state, secrets, time entries, and the
**edges between them** — and external tools reach into it.

The pain we're solving: an agent normally has to be *told* "the ticket is WD-123,
the PR is #45, the relevant discussion is this thread, the staging key is here."
Weld already has all of that **and the links between them**. So the win isn't
"an API exists" — it's that an agent can ask **one source** for *"everything
about WD-123"* and get the task + linked PR + the chat where it was discussed +
who touched it, already joined.

**The in-app agent is being completely replaced by this.** No agent in the app.

## Goals

- A **CLI** that lets a human *and* an agent do everything from the terminal —
  read and write across board, chat, GitHub, time, secrets.
- A **"current task" context model**: focus a task, and the CLI can answer
  "what's happened with this so far" — including activity that arrived since you
  last looked (e.g. someone messaged you about it).
- Reuse the existing backend service layer; the CLI is a **thin client over the
  Axum API**, not a reimplementation.
- Be the foundation an **MCP server** can sit on next, with no rework.

## Non-Goals (for v1)

- Local-first / offline operation (see Architecture decision below).
- True push / ambient notifications (the agent reacting the instant a message
  lands). v1 is pull-with-delta; push is a later layer.
- Keeping the in-app agent. It goes away.

## Architecture Decision: CLI → Backend API (not local-first)

The CLI talks to the **Axum backend over HTTP**, reusing existing `services/`.
It does **not** run against a local `weld-core` SQLite cache.

Reasoning:

1. **The killer feature is inherently server-side.** "What's happened with this
   task" = GitHub webhook state + chat + task refs, freshly joined. GitHub events
   and chat land on the server; the realtime hub is server-side. A local-first
   CLI would always show a *staler* version of the exact thing being asked for.
2. **The local layer doesn't exist yet.** `weld-core` is currently models +
   errors; the sync / SQLCipher layer is still planned. Local-first means
   building a full sync engine *before* getting any value. Hitting the API keeps
   the CLI thin and shippable because every service already exists.
3. **Offline isn't the use case.** A dev talking to their dev workspace is
   online. Local-first earns its cost for mobile/flaky clients, not a terminal.

Local-first stays a **later optimization** (offline read cache), never a v1 blocker.

```
weld CLI  ──HTTP──▶  Axum API  ──▶  services/   ──▶  Postgres / Redis / GitHub
   │                                  ▲
   └── (later) MCP server ────────────┘   same services, second adapter
```

Both CLI and the future MCP server are thin adapters over `weld-core` + the API.

## Authentication: Personal Access Tokens (new)

JWT access tokens (15 min, browser-oriented) are wrong for a terminal/agent tool.
Add a **Personal Access Token** concept:

- Long-lived, **revocable**, **scoped** (e.g. read-only vs read-write, per-team).
- Stored by the CLI in OS keychain / `~/.config/weld/`.
- Audited separately from human logins — important once *agents* and CI use them.
- `weld login` issues/stores one; `weld auth revoke` kills it.

This is also exactly what the future MCP server and CI will need, so it's not
CLI-specific plumbing.

> Decision: add PATs rather than have the CLI juggle refresh-token rotation.

## Context Endpoints

### Aggregate: "everything about this task"

New `services/` aggregation behind a single endpoint so **both** the CLI and the
future MCP server get it for free (composing it client-side would duplicate the
join in every consumer):

```
GET /api/v1/tasks/:id/context
```

Returns the joined view:
- the task (status, column, assignee, labels, description)
- linked GitHub issue + PR, with current state (open/merged/closed, checks)
- chat messages that reference the task (`messages.task_refs` already exists)
- who has touched it (recent activity / changed_fields history)
- linked secrets/time entries as applicable

### Granular endpoints alongside it

Keep focused endpoints too, so callers can fetch just one slice cheaply:

```
GET /api/v1/tasks/:id/chat        # messages referencing this task
GET /api/v1/tasks/:id/github      # linked issue/PR + live state
GET /api/v1/tasks/:id/activity    # who changed what, when
```

The aggregate endpoint composes these server-side.

## "Current Task" + Ambient Context

The idea: **focus a task**, and "what's happened" automatically includes activity
that arrived **since you last looked** — e.g. someone messaged you about it.

### Mechanism (v1 — pull with delta)

- **Focus pointer**: CLI tracks a current task (`weld task focus WD-123`),
  stored in CLI config (and optionally server-side per user so MCP/other clients
  share it).
- **Watermark**: a "last seen" timestamp per focused task. `weld task context`
  surfaces *deltas* since the watermark — new referencing messages, new PR
  activity, new mentions of you — then advances the watermark.
- **Already-present building blocks**: `messages.task_refs` (GIN-indexed) +
  `GET /teams/:id/task-refs` make "messages about this task" directly queryable.
  The delta layer is the watermark on top.

So an agent that runs `weld task context` before/after working automatically
"knows" someone messaged about the current task — no manual telling.

### Later — push / ambient

True "the agent knows the instant a message lands" needs the **realtime WS hub**
(already exists) or a small background daemon that watches the focused task's
rooms and notifies. Deferred; v1's pull-with-delta is sufficient for a terminal
tool.

## Write Path — Reuse What Exists

"Do everything from the terminal" includes writes. The study agent's
`services/study/agent/tools.rs` already wraps the service layer for
`board_write` / `chat_post` / `github_link` (with realtime publish). The CLI
calls the **same services**. So replacing the in-app agent re-fronts this work
from a UI panel to the terminal — it isn't thrown away.

Example write commands:
- `weld task move WD-123 "In Progress"`
- `weld task comment WD-123 "..."` / `weld chat post #channel "..."`
- `weld task link-pr WD-123 45`

## Command Surface (sketch)

```
weld login / auth revoke
weld task focus WD-123            # set current task
weld task context [WD-123]        # aggregate join; deltas since last seen
weld task chat WD-123             # granular: referencing messages
weld task github WD-123           # granular: linked issue/PR state
weld task move WD-123 "Done"
weld chat post #channel "..."
weld time start WD-123 / stop
```

Output: human-readable by default, `--json` for agents/scripts.

## Phasing

1. **PATs** — token model, `weld login`, revoke, scopes. (Unblocks everything.)
2. **Aggregate + granular context endpoints** — `services/` join, `--json`.
3. **CLI skeleton** — auth, config, `task context`, read commands.
4. **Current-task focus + watermark deltas** — the "someone messaged me" win.
5. **Write commands** — reuse `tools.rs`-style service calls.
6. **Remove the in-app agent.**
7. **(Later) MCP server** — second adapter over the same services.
8. **(Later) Push/ambient** + local-first offline cache.

## Open Questions

- Should the focus pointer + watermark live **server-side per user** (so MCP and
  multiple terminals agree on "current task") or **purely CLI-local**?
- PAT scope granularity — coarse (read / read-write) or per-domain
  (board/chat/github/secrets/time)?
- Does the aggregate context endpoint pull **live** GitHub state on each call, or
  serve last-synced + a freshness hint?
- For deltas, watermark per `(user, task)` — store where, and how is "mentions of
  you" defined vs. "references the task"?
```
