# Weld Study — Operator Runbook

Everything you need to stand up the study, run participants, and pull the data.
This is the practical "how to operate it" companion to the design docs
(`CLAUDE_STUDY.md`, `../replication/WELD_AGENT_STUDY_DESIGN.md`). If a value here
disagrees with code, code wins — file paths are given so you can check.

> **Note:** this used to sit in the gitignored `research-paper/` folder, which
> meant the operator's notebook existed on exactly one machine. It is tracked now.

---

## 0. TL;DR — the shortest path to a real run

1. Deploy the backend with these env vars set (details in §2):
   `STUDY_MODE=true`, `OPENAI_API_KEY=…`, `STUDY_EXPORT_TOKEN=…`,
   `STUDY_REQUIRE_ACCESS_CODE=true`, `SECURE_COOKIES=true`, plus the always-required
   `DATABASE_URL` / `JWT_SECRET`.
2. Deploy the frontend with `PUBLIC_API_BASE_URL` pointing at that backend.
3. Migrations run automatically on backend start.
4. Mint one access code per participant (§4) and hand them out.
5. Do the §7 pre-run verification (scoping + a pilot), then open recruitment.
6. Pull data anytime with the export endpoint (§6).

---

## 1. What has to be true for a valid run

- **Backend in study mode** (`STUDY_MODE=true`). Off = normal product; all study
  endpoints 404.
- **OpenAI key set** (`OPENAI_API_KEY`). Without it the agent degrades to "not
  configured" and the agent tasks (T1–T6) can't be done — a no-go for a real run.
- **Access codes required** (`STUDY_REQUIRE_ACCESS_CODE=true`) so one person can't
  run the study twice. (Leave it `false` only while *you* are testing.)
- **Export token set** (`STUDY_EXPORT_TOKEN`) so you can mint codes and pull data.
- **HTTPS + secure cookies** in production (`SECURE_COOKIES=true`) — the participant
  session uses a refresh cookie.

---

## 2. Backend environment variables

Read by `crates/weld-server/src/config.rs`. Set these on the deploy (Fly/Railway/
container env — **never commit them**).

### Always required
| Var | Example | Notes |
|---|---|---|
| `DATABASE_URL` | `postgres://user:pass@host:5432/weld` | Postgres 16. |
| `JWT_SECRET` | 64-char hex | Signs participant session tokens. |
| `HOST` / `PORT` | `0.0.0.0` / `3000` | Bind address. |

### Study-specific
| Var | Set to | What it does |
|---|---|---|
| `STUDY_MODE` | `true` | Turns on all study behavior (provisioning, telemetry, agent, GitHub mock + write-lockdown, the participant flow). |
| `OPENAI_API_KEY` | `sk-…` | Enables the context-scoped agent. **Server-side only** — never expose to the client. |
| `OPENAI_MODEL` | leave unset — the `config.rs` default `gpt-5.4-2026-03-05` is the pin | Pinned model snapshot, and the string the paper's methods section names. Don't change mid-study — a model change is a confound. Setting this to anything else silently makes the paper wrong. |
| `STUDY_REQUIRE_ACCESS_CODE` | `true` for a real run | Requires a valid, unused single-use code to start. `false` (default) lets you provision without a code while testing. |
| `STUDY_EXPORT_TOKEN` | a long random secret | Guards **both** data export and code-minting. When unset, those endpoints 404 (disabled). |
| `SECURE_COOKIES` | `true` in prod | `Secure` flag on the refresh cookie. Requires HTTPS. |
| `FRONTEND_URL` | `https://study.yourhost` | Used for CORS / redirects. |

> The agent's temperature (`0.2`) and prompt version (`v3`) are pinned in code
> (`crates/weld-server/src/main.rs`), not env — intentionally held constant.

> `STUDY_REQUIRE_ACCESS_CODE` and `STUDY_EXPORT_TOKEN` are documented in the
> repo's `.env.example` — but as placeholders only. Set the real values directly
> on the deploy; never commit them.

---

## 3. Frontend setup

The web app (`apps/web`) reads one relevant build-time var:

| Var | Example | Notes |
|---|---|---|
| `PUBLIC_API_BASE_URL` | `https://api.study.yourhost` | Backend origin. Empty = same-origin. See `apps/web/src/lib/api/client.ts`. |

The frontend learns study settings at runtime from `GET /api/v1/config`
(`study_mode`, `study_require_access_code`) — so when the backend has
`STUDY_REQUIRE_ACCESS_CODE=true`, the intro automatically shows the access-code
field. No separate frontend flag to flip.

Build & deploy:
```bash
cd apps/web
pnpm install
pnpm build        # static/SSR output per adapter
```

---

## 4. Minting access codes

Codes cap participation to one run per person. Mint a batch with the export token
(prefer the **Posting TUI**? see §11):

```bash
curl -X POST https://API_HOST/api/v1/study/codes \
  -H "Authorization: Bearer $STUDY_EXPORT_TOKEN" \
  -H 'Content-Type: application/json' \
  -d '{"count": 60, "note": "wave-1"}'
```

- Returns the new codes as plain text, **one per line** (format `WELD-XXXX-XXXX`).
  Save that output — it's how you hand codes out.
- `count` is 1–500. `note` is an optional label for tracking (e.g. `pilot`,
  `wave-1`) and shows up in the access-codes export.
- Hand **one code to each recruited participant**. Don't reuse.

Check redemption anytime (who used what, which are still unused):
```bash
curl "https://API_HOST/api/v1/study/export?dataset=access_codes" \
  -H "Authorization: Bearer $STUDY_EXPORT_TOKEN" -o access_codes.csv
```

---

## 5. What a participant experiences

1. Opens the study URL → welcome → consent → short eligibility screening.
2. (If codes required) enters their access code. Invalid → "isn't valid";
   already-used by *someone else's* flow → resumes that session (see below).
3. Gets randomly assigned a condition (balanced across **C1 chat-only /
   C2 +board / C3 +github**), then works through **12 tasks**: 6 manual baselines
   (B1–B6, agent hidden) + 6 agent tasks (T1–T6). Task script:
   `../replication/WELD_AGENT_TASK_SCRIPTS.md`.
4. They self-pace: a task button opens the instruction; they hit **Next task**
   when they believe they're done (no auto-advance).
5. At the baseline→agent boundary a **post-baseline NASA-TLX** interstitial is
   shown (manual-phase load, before any agent exposure).
6. Exit survey (NASA-TLX sliders + Likert instrument), then a done screen.

**Resume (single-code, any device):** if a participant closes the tab / switches
device / clears their browser and re-enters the **same code**, they resume their
existing session at the exact task they left off on — no new workspace, no
re-running tasks. A finished participant who re-enters their code just sees the
done screen. (Progress is mirrored server-side via `POST /api/v1/study/progress`;
their actual work always lived in their isolated DB project.)

**Safeguards already in place:**
- Agent message cap: **50** messages/participant (`run.rs`, `MAX_AGENT_MESSAGES`) —
  stops a spammer from running up OpenAI cost.
- `session_abandoned` telemetry marks drop-outs (tab closed mid-study).
- GitHub is mocked and write-locked under study mode (no real repo is touched).

---

## 6. Pulling the data

All exports are CSV, guarded by the export token (Bearer header only — never a
query param). Endpoint: `GET /api/v1/study/export?dataset=<name>`. Prefer the
**Posting TUI**? see §11.

| dataset | file | contents |
|---|---|---|
| `participants` | study_participants.csv | roster + lifecycle (code, condition, status, timestamps). |
| `events` | study_events.csv | one row per behavioral telemetry event (the bulk of the analysis data). |
| `survey` | study_survey.csv | long format, one row per (participant, survey item). |
| `access_codes` | study_access_codes.csv | code redemption tracking. |

```bash
for d in participants events survey access_codes; do
  curl -s "https://API_HOST/api/v1/study/export?dataset=$d" \
    -H "Authorization: Bearer $STUDY_EXPORT_TOKEN" -o "study_$d.csv"
done
```

**Privacy:** the optional follow-up-interview email is stored separately and is
**never** in any export (by design — `../replication/WELD_EXIT_SURVEY.md` §6). CSV fields are
defused against spreadsheet formula injection.

---

## 7. Pre-run verification (do this before recruiting)

1. **Scoping proof (automated).** With the DB up:
   ```bash
   make start                 # Postgres + migrations
   cargo test -p weld-server --test study_agent_tests
   ```
   Confirms C1 never assembles board/GitHub and C2 never GitHub on real seeded
   data. Also run `cargo test -p weld-server --test study_access_code_tests` to
   confirm the one-run-per-person + resume invariants.

2. **Model smoke test — still do this FIRST.** Everything up to this point is
   verified without ever calling OpenAI. Send one trivial agent message on a
   staging deploy with `OPENAI_API_KEY` set and confirm a 200.

   **This already bit us once.** The 2026-08 pilot's first agent call returned a
   hard 400: the pinned snapshot rejects `max_tokens` in favour of
   `max_completion_tokens`. Every agent turn failed for that participant until it
   was fixed. `temperature: 0.2` was *accepted*, so the fixed temperature the
   paper's methods section names still holds.

   `openai.rs` now corrects itself: a rejection that names a request parameter is
   read, the shape fixed, and the call retried, with the correction remembered
   for the life of the process. It defaults to `max_completion_tokens`, so on the
   current pin the first call should simply succeed. Two things still to check by
   eye after the smoke test:
   - the server log has no `OpenAI rejected the …` warning — if it does, the
     pin's request shape moved again;
   - `agent_response` telemetry carries `"temperature": 0.2` and not `null`.
     `null` means the snapshot refused the fixed temperature and the run is at
     the provider default — the methods section must then say so.

   Fix the request shape; do **not** "fix" it by changing `OPENAI_MODEL`, which
   would contradict the model named in the paper's methods. Retries no longer
   compound this: only transport faults, 429 and 5xx are retried, so a
   deterministic 400 surfaces on the first attempt.

3. **Live-model pilot.** With `OPENAI_API_KEY` set on a staging deploy, run one
   participant per condition end-to-end:
   - All 12 tasks are completable with the seeded data (watch T6 — the vaccination
     PR is #34, card starts unlinked on purpose).
   - The `agent_context_sent` telemetry snapshots show the right scope per
     condition.
   - Telemetry lands and all four CSV exports round-trip.
   - The pinned snapshot is strong enough that C2/C3 actually *call tools* rather
     than describing what they would do. A model that only advises silently
     collapses C2 and C3 into C1 and turns the study into a null (design doc,
     critical-path risk 5).

4. **Analysis dry run.** Point `paper/analysis/run_all.py` at the pilot's real
   exports, not just the synthetic ones. This is what catches a measure that is
   collected but not derivable — the exports round-tripping is not the same as
   the pipeline producing a number from them.

   ```bash
   cd paper/analysis
   python3 -m venv .venv && . .venv/bin/activate && pip install -r requirements.txt
   mkdir -p data                      # gitignored — real participant data never gets committed
   # drop the four CSVs from §6 into data/, then:
   python run_all.py --data data --out out
   ```
   Read `out/report.txt`. Section 1 must show C1 at 0%/0%, C2 at 100%/0%, C3 at
   100%/100%, and **0 violations** in every row.

   Done for the 2026-08 pilot (3 participants, one per condition), and it earned
   its keep: the manipulation check was scanning the context snapshot for the
   *words* "board" and "github", which the seeded chat legitimately contains, and
   reported a total scoping failure against a gate that was working perfectly.
   Fixed in `analyze.py`. The lesson generalises — when this table says something
   catastrophic, confirm it against `sources_included` in the raw
   `agent_context_sent` payloads before believing it.

   What that run could **not** test, and still needs real N: the mixed model, the
   ordinal correctness model, Cronbach's α, and the RQ3 correlations. All of them
   degrade to "insufficient data" rather than printing something wrong, which is
   the behaviour you want but not the same as being exercised.

   Two paths remain untested end-to-end: `correctness.csv` against the coders'
   actual file, and `out/correctness_model.R` — the model of record for
   correctness — which needs R and `ordinal::clmm`. Run the R snippet once on
   synthetic data before results day rather than debugging R with real numbers
   waiting.

5. **Seed sanity.** Newly provisioned participants get the current seed
   (`crates/weld-server/study/pettracker_seed.json`, baked at build time). If you
   edited the seed, **rebuild/redeploy** the backend — already-provisioned
   participants keep their cloned board.

---

## 8. Local development / testing

```bash
make start          # Docker Postgres/Redis/MinIO + migrations
make server         # backend (loads .env)
cd apps/web && pnpm dev
```
For testing the study without juggling codes, leave `STUDY_REQUIRE_ACCESS_CODE`
unset/`false` — provisioning works without a code. Set `STUDY_MODE=true` and
`OPENAI_API_KEY` to exercise the agent.

---

## 9. Operational gotchas

- **Re-provision to reset.** There's no migration that retro-edits an
  already-cloned participant board (by design). To test fresh state, start a new
  participant (or, in code-required mode, use a fresh code).
- **A code is spent on first use.** If a participant's run genuinely breaks and
  you don't want a resume, issue them a *new* code and discard the partial data
  (track via the access_codes export).
- **Don't rotate `OPENAI_MODEL` mid-study** — it's a confound, and the paper's
  methods section names the pinned string.
- **Condition assignment is minimisation, not simple randomisation.** Each new
  participant goes to the least-filled cell, ties broken at random, counted over
  *all* rows in `study_participants` — including people who provisioned and
  dropped out. A dropout therefore holds its cell's slot. If several participants
  abandon in one condition, check the `by_condition` counts on the dashboard
  before the last few sessions and expect completed-cell sizes to be less even
  than the provisioned ones.
- **Consent text and `CONSENT_VERSION` move together.** Editing the consent copy
  in `StudyIntro.svelte` without bumping `CONSENT_VERSION` in `study.svelte.ts`
  makes the recorded version wrong for everyone after the change; the current
  value is `v1.2`, and the consent form PDF handed to the IRB must say the same
  session length as the app.
- **Keep `STUDY_EXPORT_TOKEN` secret.** It mints codes and exports data; treat it
  like an admin credential. Rotating it disables old export scripts until updated.

---

## 10. Endpoint quick reference (study mode)

| Method | Path | Auth | Purpose |
|---|---|---|---|
| GET | `/api/v1/config` | none | study flags for the frontend |
| POST | `/api/v1/study/provision` | none | start/resume a participant (needs code if required) |
| POST | `/api/v1/study/progress` | participant | save resume point |
| POST | `/api/v1/study/events` | participant | telemetry batch |
| POST | `/api/v1/study/agent` | participant | one agent turn (scoped) |
| POST | `/api/v1/study/survey` | participant | submit exit survey |
| POST | `/api/v1/study/codes` | export token | mint access codes |
| GET | `/api/v1/study/export?dataset=…` | export token | CSV export |

All return 404 unless `STUDY_MODE=true`; the token-guarded ones also 404 unless
`STUDY_EXPORT_TOKEN` is set.

---

## 11. Doing it from the Posting TUI

If you'd rather use [Posting](https://posting.sh) instead of `curl`, here's the
exact setup for the two researcher operations: **mint codes** and **pull data**.
Both just need your `STUDY_EXPORT_TOKEN` in the `Authorization` header as a
Bearer token. The participant-facing endpoints are not something you drive by
hand — ignore them here.

### 11.1 One-time: an env file for the token

Make a file (e.g. `study.posting.env`) so the token isn't typed into requests:

```env
API_HOST=https://api.study.yourhost
STUDY_EXPORT_TOKEN=paste-your-real-token-here
```

Launch Posting pointed at it (variables are then referenced as `$VAR`):

```bash
posting --env study.posting.env
```

### 11.2 The two requests, set up by hand in the UI

Posting fields map 1:1 to the endpoints. Create each request and fill in:

**A) Mint access codes**
- **Method:** `POST`
- **URL:** `$API_HOST/api/v1/study/codes`
- **Headers tab:**
  - `Authorization` → `Bearer $STUDY_EXPORT_TOKEN`
  - `Content-Type` → `application/json`
- **Body tab:** choose *Raw (JSON)* and enter:
  ```json
  { "count": 60, "note": "wave-1" }
  ```
- **Send.** The response body is your codes, **one per line**
  (`WELD-XXXX-XXXX`). Save the response to a file (Posting's response pane has a
  save action) — e.g. `codes_wave-1.txt` — and hand them out one per participant.

**B) Export a dataset (the data at the end)**
- **Method:** `GET`
- **URL:** `$API_HOST/api/v1/study/export`
- **Query/Params tab:** add one param `dataset` with value one of:
  `participants`, `events`, `survey`, `access_codes`
  (or just put `?dataset=events` on the URL).
- **Headers tab:** `Authorization` → `Bearer $STUDY_EXPORT_TOKEN`
- **Send.** The response body is the CSV. Use Posting's **save-response-to-file**
  action and name it to match, e.g. `study_events.csv`. Repeat for each of the
  four datasets (change the `dataset` param each time).

> Common gotchas: a `401` means the token/header is wrong (must be exactly
> `Bearer <token>`); a `404` means either `STUDY_MODE` is off or
> `STUDY_EXPORT_TOKEN` isn't set on the backend. The token goes in the **header
> only** — never as a URL query param.

### 11.3 Optional: save these as a Posting collection

Posting stores requests as YAML files in a collection folder so you don't rebuild
them each time. Drop these two files in a folder and open it with
`posting --collection ./weld-study --env study.posting.env`:

`weld-study/mint-codes.posting.yaml`
```yaml
name: Mint access codes
method: POST
url: $API_HOST/api/v1/study/codes
headers:
  - name: Authorization
    value: Bearer $STUDY_EXPORT_TOKEN
  - name: Content-Type
    value: application/json
body:
  content: |
    { "count": 60, "note": "wave-1" }
```

`weld-study/export-events.posting.yaml`
```yaml
name: Export events CSV
method: GET
url: $API_HOST/api/v1/study/export
params:
  - name: dataset
    value: events
headers:
  - name: Authorization
    value: Bearer $STUDY_EXPORT_TOKEN
```

Duplicate the export file three more times, changing the `dataset` value to
`participants`, `survey`, and `access_codes`. (Field names can vary slightly
between Posting versions — if the YAML doesn't load, the hand-entry in §11.2
always works.)
