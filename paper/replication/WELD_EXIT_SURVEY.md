# Weld Study — Exit Survey (Build-Ready Spec v1.0)

This is the complete post-session survey. It runs after Phase 3 (open exploration) and before the "done" screen. Target completion time: 5–8 minutes. Every response must be joined to the participant's telemetry by `participant_id` (passed through automatically — never asked of the participant).

**Ordering rationale.** Cognitive-load and usability instruments come first (while the experience is freshest and before fatigue), then the integration-specific items and the trust-in-agent block (the items most tied to the research question), then open-ended reflection, and demographics last (lowest cognitive cost, and if someone abandons we still keep the substantive data).

**Implementation notes.**
- Each item carries a stable `item_id` — store responses keyed by these, not by question text.
- All Likert items render as labeled radio scales. Do not allow skipping the validated-instrument items (NASA-TLX, SUS) if you want to compare to published norms; open-ended items may be optional.
- Persist partial responses on every page advance, so an abandon mid-survey still captures completed pages.
- Record `survey_started` and `survey_completed` timestamps as telemetry events.

---

## Section 1 — NASA-TLX (raw / unweighted)

**Instruction shown to participant:**
> Think about the tasks you just completed in Weld. For each item, mark the point on the scale that best matches your experience.

Each item is a 21-point scale (standard NASA-TLX granularity) rendered with anchored endpoints. Store the integer 1–21 (or 0–100 if you prefer the percentage convention — pick one and keep it consistent).

| item_id | Dimension | Question | Low anchor (1) | High anchor (21) |
|---|---|---|---|---|
| tlx_mental | Mental Demand | How mentally demanding were the tasks? | Very low | Very high |
| tlx_physical | Physical Demand | How physically demanding were the tasks? | Very low | Very high |
| tlx_temporal | Temporal Demand | How hurried or rushed was the pace of the tasks? | Very low | Very high |
| tlx_performance | Performance | How successful were you in accomplishing what you were asked to do? | Failure | Perfect |
| tlx_effort | Effort | How hard did you have to work to accomplish your level of performance? | Very low | Very high |
| tlx_frustration | Frustration | How insecure, discouraged, irritated, stressed, and annoyed were you? | Very low | Very high |

> **Note on the Performance item:** it is anchored low = Failure, high = Perfect — so, unlike the other five items, **high = good performance (less workload)**. The raw position is stored as shown. To enter the NASA-TLX workload composite (where higher = more load), **reverse-score it**: `tlx_performance_load = MAX + MIN − raw` (e.g. `101 − raw` on 1–100, `22 − raw` on 1–21). Apply the same reversal to `tlx_performance_baseline`. Reverse exactly once — do not double-reverse.

---

## Section 2 — System Usability Scale (SUS)

**Instruction shown to participant:**
> For each statement, mark how much you agree or disagree, thinking about your overall experience with Weld today.

Standard 10 items, 5-point scale: 1 = Strongly disagree … 5 = Strongly agree. Items alternate positive/negative tone by design — keep the exact standard wording for comparability to published SUS norms. Replace "the system" with "Weld."

| item_id | Statement |
|---|---|
| sus_01 | I think that I would like to use Weld frequently. |
| sus_02 | I found Weld unnecessarily complex. |
| sus_03 | I thought Weld was easy to use. |
| sus_04 | I think that I would need the support of a technical person to be able to use Weld. |
| sus_05 | I found the various functions in Weld were well integrated. |
| sus_06 | I thought there was too much inconsistency in Weld. |
| sus_07 | I would imagine that most people would learn to use Weld very quickly. |
| sus_08 | I found Weld very cumbersome to use. |
| sus_09 | I felt very confident using Weld. |
| sus_10 | I needed to learn a lot of things before I could get going with Weld. |

> SUS scoring (compute at analysis, not in the form): for odd items, score = response − 1; for even items, score = 5 − response; sum all ten and multiply by 2.5 for a 0–100 SUS score. Item `sus_05` ("well integrated") is especially relevant to your research question — report it individually as well as in the composite.

---

## Section 3 — Integration-Specific Items (custom)

These map directly to the research question and are the items most likely to show the effect you care about. 7-point Likert unless noted. Pilot-test the wording.

| item_id | Question | Scale |
|---|---|---|
| int_findacross | When I needed to find information across different parts of Weld (board, chat, GitHub), it was easy. | 1 = Strongly disagree … 7 = Strongly agree |
| int_noticed | I noticed moments where features in Weld were connected to each other in helpful ways. | 1 = Strongly disagree … 7 = Strongly agree |
| int_vs_separate | Compared to my usual workflow of using separate apps for tasks, chat, and code, working in Weld felt: | 1 = Much harder … 4 = About the same … 7 = Much easier |
| int_reuse | I would consider using Weld for a real class or personal project. | 1 = Strongly disagree … 7 = Strongly agree |
| int_switch_perceived | While working, I felt like I was constantly switching between separate tools. | 1 = Strongly disagree … 7 = Strongly agree |

> `int_switch_perceived` is intentionally reverse-coded relative to the integration hypothesis and is the **perceived** counterpart to the **behavioral** view-switch-rate metric from telemetry. The comparison of these two is the perception–behavior gap analysis — arguably the most publishable finding. Keep both.

---

## Section 4 — Trust in the AI Assistant (custom)

Instruction shown to participant: *Thinking about the AI assistant you used during the tasks.* 7-point Likert, all required. This block carries H5 (the perception–behavior gap): `trust_speed_perceived` and the trust composite are correlated against each participant's *measured* normalized task time and correctness; it also carries H4 (reliance calibration) via `trust_rely` vs. `trust_verify`. `trust_verify` is reverse-valenced (higher = more checking/repair).

| item_id | Statement | Low anchor (1) | High anchor (7) |
|---|---|---|---|
| trust_reliable | The AI agent was reliable. | Strongly disagree | Strongly agree |
| trust_predictable | I could predict what the AI agent would do. | Strongly disagree | Strongly agree |
| trust_competent | The AI agent was capable of doing what I needed. | Strongly disagree | Strongly agree |
| trust_rely | I would be comfortable relying on the AI agent without double-checking its work. | Strongly disagree | Strongly agree |
| trust_context | The AI agent had enough context to actually help me. | Strongly disagree | Strongly agree |
| trust_again | I would want to use this AI agent for my own real work. | Strongly disagree | Strongly agree |
| trust_verify | I felt I had to check or redo what the agent did. | Never | Constantly |
| trust_speed_perceived | Using the agent made me faster. | Much slower | Much faster |

> `trust_context` is expected to rise C1→C2→C3 (a manipulation-adjacent check on perceived context sufficiency). `trust_rely` vs. `trust_verify` measures reliance calibration (willingness to rely vs. felt need to verify). `trust_speed_perceived` vs. actual normalized time is the core H5 contrast.

> **Also collected, but not in this exit survey:** a per-task quick rating shown after each task (`task_rating` telemetry) — `effort` (1–7, every task incl. the six manual baselines) and `agent_help` (1–7, agent tasks only). The NASA-TLX (Section 1) is **also collected once at the baseline→agent boundary** (`tlx_*_baseline`), so the agent-phase TLX here forms a within-person, phase-to-phase load delta compared *between* conditions for H3; per-task `effort` is the finer-grained baseline-anchored measure. See `WELD_PARTICIPANT_FLOW.md` §6.

---

## Section 5 — Open-Ended Reflection

Free-text, each optional but encouraged. Store full text. (Do not log keystrokes — only the final submitted text, per privacy scope.)

| item_id | Prompt |
|---|---|
| open_useful | What was the most useful moment of working in Weld today? |
| open_frustrating | What was the most frustrating moment? |
| open_change | If you could change one thing about Weld, what would it be? |
| open_integration | (Optional) Did you notice the board, chat, and GitHub being connected? If so, was that helpful, distracting, or neither? |

---

## Section 6 — Demographics & Background

Collect only these fields — each has a specific analytical purpose. Keep ranges/buckets rather than exact values where possible (eases IRB, reduces re-identification risk).

| item_id | Question | Response options |
|---|---|---|
| dem_age | Age range | Under 18* / 18–24 / 25–34 / 35–44 / 45+ / Prefer not to say |
| dem_student | Are you currently a student? | Yes – undergraduate / Yes – graduate / No / Prefer not to say |
| dem_experience | Years of programming experience | < 1 / 1–2 / 3–5 / 6–10 / 11+ |
| dem_languages | Primary programming language(s) (select up to 3) | Python / JavaScript-TypeScript / Java / C/C++ / C# / Go / Rust / Swift-Kotlin / Other |
| dem_tools_pm | Project/issue trackers you've used | Jira / Linear / GitHub Projects / Trello / Asana / None / Other (multi-select) |
| dem_tools_chat | Team chat tools you've used | Slack / Discord / MS Teams / None / Other (multi-select) |
| dem_github_freq | How often do you use GitHub? | Daily / Weekly / Occasionally / Rarely or never |
| dem_integrated_before | Have you used an all-in-one dev workspace before (e.g., one app combining tasks + chat + code)? | Yes / No / Not sure |

> *If "Under 18" is selected and your IRB approval covers adults only, the flow should thank and exit the participant without collecting study data. Confirm your IRB's minor policy — this is a common gating requirement. (This screening ideally happens at *entry*, not exit; see the consent/screening flow. The exit duplicate is a backstop.)

---

## Section 7 — Optional Follow-Up Consent

| item_id | Question | Response |
|---|---|---|
| followup_ok | Would you be willing to be contacted about future research related to this study? | Yes / No |
| followup_contact | (If yes) Email for follow-up (stored separately from study data, used only to schedule) | free text, optional |

> Store any contact info in a **separate** table from the behavioral/survey data, linked only by a token, and delete it after the follow-up window. State this in the consent form. This keeps identifiable contact details out of the analysis dataset.

---

## Data Output

The survey should export one row per participant joined to telemetry by `participant_id`, with columns for every `item_id` above plus `survey_started` / `survey_completed` timestamps. Open-ended responses can live in the same row (long text) or a linked table. Contact info (Section 6) stays in its own access-restricted store.
