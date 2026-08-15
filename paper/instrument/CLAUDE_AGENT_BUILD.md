# CLAUDE.md — Weld Study Agent Build

> **Purpose.** Engineering spec for the *context-scoped AI agent* used in the Weld agent-context study. This is the critical-path, highest-risk component. The study compares three agent conditions that differ ONLY in how much of Weld's context the agent can see and act on. The validity of the entire study rests on one property: **context scoping must be enforced and provable, never merely intended.** A chat-only agent that can somehow see the board silently invalidates every participant in that condition.
>
> Read alongside `../replication/WELD_AGENT_STUDY_DESIGN.md` (the why) and `CLAUDE_STUDY.md` (the existing study build — provisioning, seed, telemetry spine, privacy model, all still apply). This file covers only what's new for the agent.

---

## 0. The One Rule That Matters Most

**The agent must only ever receive the context its condition allows, and this must be verifiable from logs after the fact.** Every other requirement is secondary to this. If you must cut corners somewhere, never cut here. Build the scoping as a hard boundary at the context-assembly layer, not as a prompt instruction ("please ignore the board") — prompt-level restrictions leak and cannot be trusted or proven.

---

## 1. The Three Conditions

A participant is assigned exactly one condition at provisioning (`agent_condition` ∈ {C1, C2, C3}), fixed for their whole session.

| Condition | Agent SEES (context sources) | Agent CAN DO (tools/actions) |
|---|---|---|
| **C1 — Chat-only** | Chat messages in the participant's project channels only | Read chat; produce text advice. **No write/act tools.** |
| **C2 — Chat + Board** | Chat + board (cards, columns, labels, assignees) | Read chat+board; create/edit/move/assign/label cards; post chat messages |
| **C3 — Full context** | Chat + board + GitHub (seeded issues, PRs, commits, README) | Everything in C2 + read GitHub fixtures, link cards to issues/PRs |

**Capability scales with context by design** (an agent can't act on what it can't see). C1 is advise-only; C2/C3 are act-capable. This coupling is intentional and is the phenomenon under study.

> **Note on baseline (C0):** the 6 manual baseline tasks have NO agent. The agent UI should be absent/disabled during C0. No agent context is assembled.

## 2. Architecture: the Context Gate

Build a single chokepoint — call it the **Context Gate** — that every agent invocation passes through. Nothing reaches the LLM except through it.

```
participant action → agent invoked
        ↓
  [ Context Gate ]  ← reads agent_condition, assembles ONLY allowed context,
        ↓             exposes ONLY allowed tools, logs the exact context sent
  LLM (OpenAI API) with scoped context + scoped tool set
        ↓
  agent response (text and/or tool calls)
        ↓
  [ Tool Executor ] ← rejects any tool not allowed for this condition (defense in depth)
        ↓
  result rendered to user + fully logged
```

**Design requirements for the Gate:**
- It is the *only* path that builds the agent's context payload. No other code path may inject project data into the prompt.
- It takes `agent_condition` as input and uses an allowlist per condition (see §3). Default-deny: if a source isn't explicitly allowed, it is excluded.
- It assembles context from the participant's **isolated** seed instance only (per `CLAUDE_STUDY.md` §2.2) — never another participant's data, never global data.
- It logs, for every invocation, the exact context categories included and a hash/snapshot of the payload (see §5).

## 3. Context Scoping (the hard boundary)

Implement scoping as explicit allowlists, enforced in code at the Gate:

```
CONTEXT_RULES = {
  C1: { sources: [chat],                tools: [] },
  C2: { sources: [chat, board],         tools: [board_write, chat_post] },
  C3: { sources: [chat, board, github], tools: [board_write, chat_post, github_link] },
}
```

- **Default-deny.** Any source or tool not listed is unavailable. Adding a new data source later must require explicitly adding it to a condition, or it stays excluded.
- **No prompt-only restrictions.** Do not implement scoping by telling the model "don't look at the board." The board data must simply *not be in the payload* for C1.
- **Tool gating is enforced twice:** (a) only allowed tools are offered to the model, and (b) the Tool Executor independently rejects any disallowed tool call even if the model somehow emits one. Log any such rejection as an anomaly.
- **Cross-references respect scope.** If a chat message contains a `[[card:...]]` link, a C1 agent may see the link text but must NOT be able to resolve/expand it into board data. Resolving a card reference is a board-context action.

## 4. Agent Capabilities (tools)

Tools available only in the conditions noted. All tool calls operate on the participant's isolated seed and are logged.

- **`board_write`** (C2, C3): create card, edit card, move card (column), set priority, assign, label. Operates on Weld data only.
- **`chat_post`** (C2, C3): post a message to a project channel as the agent (clearly attributed to the agent, never impersonating a teammate). Flagged `agent_authored`.
- **`github_link`** (C3): link a card to an existing seeded issue/PR. **No GitHub writes** — consistent with `CLAUDE_STUDY.md` §4 (GitHub is read-only/mocked; write controls hidden). The agent can read GitHub fixtures and create the Weld-side link only.
- **C1 has no tools** — it returns text advice only.

> Reuse the existing `STUDY_MODE` GitHub lockdown: the agent, like the user, cannot write to GitHub. `github_link` writes to Weld's link table, not GitHub.

## 5. Agent Telemetry (study-critical)

Every agent invocation logs an event tied to `participant_id`, `current_task_id`, and `agent_condition`. Without this you cannot analyze RQ2 or prove scoping held.

**Per invocation:**
- `agent_invoked` — payload: `{ trigger, condition }`
- `agent_context_sent` — **payload: the exact context categories included + a snapshot/hash of the assembled payload.** This is the proof-of-scoping record. Must allow post-hoc verification that, e.g., no board data appeared in a C1 payload.
- `agent_response` — payload: `{ latency_ms, response_type ∈ {advice_only, tool_calls, mixed}, num_tool_calls, model, temperature }`
- `agent_tool_call` — one per call: `{ tool_name, args_summary, allowed (bool), executed (bool) }`
- `agent_tool_rejected` — anomaly event if a disallowed tool was emitted (should be ~never; if it fires, investigate before trusting data).

**User response to agent (for RQ1 — delegation/verification):**
- `agent_suggestion_accepted` / `agent_suggestion_rejected` / `agent_action_undone` / `agent_output_edited` — how the user verifies/repairs agent work (parallels the validate-and-repair behaviors in the Tang et al. paper).

**Determinism controls (log once per session, and per call where relevant):**
- Fixed `model` and `temperature` — record them. Recommend a low temperature for reproducibility; document the choice.
- Log the full prompt template version. If prompts change between pilot and launch, version them.

**Privacy:** the `agent_context_sent` snapshot contains only PetTracker seed data (no real user data exists in the study), so storing it is safe and is required for scoping verification. Continue to exclude everything in `CLAUDE_STUDY.md` §5 "MUST NOT LOG."

## 6. The LLM Call (hosted API, pinned model)

**Provider decision: OpenAI API.** The model is a *controlled, reproducible variable* — called from the agent service. Do NOT self-host a model. (Railway has no GPU; CPU-only inference would be too weak/slow for a study where agent quality and latency are the measures, and a self-hosted setup makes reproducibility harder, not easier.) The agent *orchestration* runs on Railway; only the inference is a remote API call.

> **Settled (2026-08): the pin is `gpt-5.4-2026-03-05`**, the `OPENAI_MODEL` default in `config.rs` and the snapshot named in the paper's methods. The candidate comparison below is the historical reasoning that led there; do not re-derive a different answer from it. The reason the pin moved off the cheapest option is the risk called out two paragraphs down and in the design doc's critical-path list: a model too weak to exploit wide context floors exactly the C3 arm the study is about, and yields a null that says nothing about scope.

**Model choice — cost-effective but capable (historical):**
- **Primary: `gpt-4o-mini`** (~$0.15 / $0.60 per 1M input/output tokens as of May 2026). Start here: it's the cheapest capable option that still supports function calling, so we try it first and only pay more if it proves too weak. **Pilot-verify it can reliably drive the C2/C3 tool actions before committing** — a too-weak model fails regardless of context and would muddy the context-ladder comparison. If the pilot shows it can't reliably complete the C3 cross-surface tasks, switch to the backup.
- **Documented backup: `gpt-4.1-mini`** (~$0.40 / $1.60). More capable and ~3–4× the cost; switch to this only if `gpt-4o-mini` underperforms on C2/C3 tool-calling during the pilot. Whichever model the pilot settles on is pinned for the whole study.
- **Avoid the very cheapest (e.g. `gpt-4.1-nano`)** as the agent model: tuned for routing/extraction, likely too weak to act well in C2/C3, which would hide the effect you're measuring.
- Whatever you pick, **pin it and hold it constant for the entire study**. Decide during the pilot by checking which model reliably completes the C3 cross-surface tasks at acceptable cost.

**Cost control (keep spend minimal — explicit goal):**
- **Prompt caching:** order prompts stable-content-first (system prompt → seed context → user request) so OpenAI prompt caching applies — up to ~90% off repeated input tokens. The system prompt + PetTracker seed repeat on every call, so this is the biggest lever.
- **Cap `max_tokens`** on responses to what tasks actually need; never leave it unbounded.
- **Log per-call token usage** (input/output) so spend is visible live during the pilot — confirm real cost before full launch.
- Set a **billing limit / usage alert** in the OpenAI dashboard as a backstop.
- Scale context: this study is ~a few thousand calls over a small context, realistically low tens of dollars total even before caching. The free tier (500 RPM for mini models) covers early testing; paid tier unlocks at $5 cumulative spend.

**Reproducibility requirements (these protect the science — non-negotiable):**
- **Pin the exact model version string** (a dated/snapshot id where available, not a floating alias). Record it in config and log it on every call. A mid-study model change is a confound — treat it as a new study phase.
- **Fix temperature** at a low value (e.g., 0–0.2) for consistency across participants. Record it.
- **Version the system prompt and tool definitions.** Any change between pilot and launch must bump a `prompt_version` logged per call.
- **Log the full request and response** per call: assembled (scoped) context, system prompt version, model id, temperature, token usage, and the model's complete output (text + tool/function calls). With `agent_context_sent` (§5) this proves both scoping and reproducibility after the fact.

**Mechanics:**
- Integration entirely server-side on the Railway agent service; never expose the key client-side. Store as a Railway secret (`OPENAI_API_KEY`).
- The Context Gate builds the messages array from scoped context only (§2, §3).
- Use OpenAI **function calling / tools**; offer only the condition's allowed tools in the definitions passed to the model (§3, §4).
- Capture and log latency (`latency_ms`) and token usage per call.
- Handle API errors gracefully: a failed call surfaces a clean "the assistant is unavailable, please try again" and logs `agent_error` — never a stack trace or a silent hang that corrupts task timing. Consider one automatic retry with backoff first.

**Provider flexibility (optional safety net):** isolate the LLM call behind a thin internal interface so swapping models (or providers, if credits appear elsewhere) is a one-file change. The OpenAI tools/function-calling shape is also what most OpenAI-compatible providers expose, keeping options open cheaply.

## 6A. Railway Deployment Notes

- The agent service is a normal server-side component of the Weld deployment on Railway — it calls the OpenAI API over HTTPS. No GPU, no model weights, no special infra.
- **Secrets:** `OPENAI_API_KEY` lives in Railway env vars, never in code or client.
- **Cost monitoring:** log per-call token usage (§6) and set an OpenAI dashboard usage alert; watch spend during the pilot and catch anything unexpected before full launch.
- **Rate/latency:** if Railway-to-API latency is ever an issue, it's part of what you measure (agent latency) — log it, don't hide it. At this scale it should be fine.

## 7. Agent UI (minimal)

- A simple agent panel/chat within the task workspace, present only in C1/C2/C3 (absent in C0 baseline tasks).
- User types a request; agent responds with advice and/or performs actions.
- When the agent acts (C2/C3), the action must be **visible and attributable** — the user sees "Agent moved card X to In Progress" so they can verify/override. This visibility is what generates the accept/reject/undo telemetry for RQ1.
- Keep the UI consistent across conditions so the only experienced difference is what the agent can actually do — not how the panel looks.

## 8. Build Order

1. **Context Gate + scoping rules (§2, §3)** — the hard boundary. Build and unit-test this first.
2. **Agent telemetry, especially `agent_context_sent` (§5)** — so you can prove scoping from day one.
3. **LLM call wiring (§6)** with C1 (advise-only) first — simplest, no tools.
4. **Tool executor + `board_write`, `chat_post` (C2)** with double-gating.
5. **`github_link` (C3)**, reusing the GitHub read-only lockdown.
6. **Agent UI (§7).**
7. **Scoping verification pass (§9)** before any real participant.

## 9. Acceptance Checklist (scoping is non-negotiable)

- [ ] **Proof of scoping:** for a C1 session, inspect `agent_context_sent` logs and confirm board/GitHub data NEVER appears in any payload. Repeat for C2 (no GitHub). Automate this check if possible.
- [ ] A C1 agent offered no write tools; a disallowed tool call (if ever emitted) is rejected by the executor and logged as `agent_tool_rejected`.
- [ ] A C1 agent cannot resolve a `[[card:...]]` chat reference into board data.
- [ ] Agent operates only on the participant's isolated seed; no cross-participant or global data ever enters a payload.
- [ ] Every invocation logs context-sent, latency, response type, tool calls, and (where relevant) accept/reject/undo.
- [ ] Model and temperature are fixed and logged; prompt template is versioned.
- [ ] Model is a **pinned dated version string** (not a floating "latest" alias); a mid-study change is treated as a confound.
- [ ] Every call logs the full scoped request + complete response + model id + prompt_version + temperature + token usage.
- [ ] OpenAI API key is a Railway secret, never client-side; LLM call site is abstracted behind a thin interface.
- [ ] Agent absent/disabled during C0 baseline tasks.
- [ ] Agent never writes to GitHub; `github_link` writes only to Weld's link table.
- [ ] API failures degrade gracefully and are logged; task timing isn't silently corrupted.
- [ ] `agent_condition` is recorded on every agent event and fixed per participant for the whole session.

## 10. Out of Scope

- Multi-step autonomous planning beyond what a task needs — keep the agent reactive to user requests.
- Any real GitHub writes.
- Letting the agent impersonate a seeded teammate — agent messages are always attributed to the agent.
- Cross-condition switching within a participant — condition is fixed at provisioning.
- Agent memory across sessions — each participant session is self-contained.
