# Weld Study — In-App Flow, Screening & Recruitment Content

All participant-facing copy for the study, in the order a participant encounters it. Wording is ready to use; adjust tone to match Weld's voice ("technical but approachable"). Anything in [brackets] needs your input.

---

## 1. Recruitment Materials

### 1a. Email / mailing-list post

> **Subject:** Help test a developer workspace — ~40 min
>
> Hi all,
>
> I'm a student researcher at Michigan Technological University studying how developers work when their task board, team chat, and GitHub integration live in one app instead of three separate tools.
>
> I'm looking for people with **at least a year of programming experience** to try a tool called Weld and complete a short set of realistic tasks. It takes about **an hour**, it's **fully online and self-paced** (do it whenever you want), and you'll work in a practice project — no setup or real code of your own required. There is no compensation for taking part.
>
> This study has been reviewed and approved by the Michigan Tech Institutional Review Board (IRB-2026-111). Participation is voluntary; you may stop at any time without penalty.
>
> If you're interested, start here: **[study link]**
>
> Thanks! Questions welcome at awsadler@mtu.edu, or contact the MTU IRB at irb@mtu.edu.

### 1b. Short community / social post

> Are you a developer (student or professional, 1+ yr experience)? I'm running a 40-min online research study on integrated dev workspaces — task board + chat + GitHub in one app. Self-paced, work in a practice project. [study link]

### 1c. Flyer one-liner

> **Developers wanted** for a 1-hour online study on all-in-one developer tools. 1+ year experience. → [study link / QR]

---

## 2. Landing Page (study entry)

> ## Welcome 👋
>
> Thanks for your interest in this research study. Here's what to expect:
>
> - **About an hour**, done online at your own pace.
> - You'll work through some realistic tasks in **Weld**, a developer workspace, using a **practice project** — none of your real code or accounts are involved.
> - Then a **short survey** about your experience.
>
> You'll be asked for your email and institution so we can link your responses to you; these are stored securely and separately from your study data. Before you begin, please read the consent information on the next page.
>
> **[ Continue ]**

---

## 3. Consent Screen
Renders the IRB-approved consent form (see `WELD_CONSENT_FORM.md`). Affirmative action required to proceed. The consent affirmation is the 18+ age gate: the participant must confirm they are 18 or older here before continuing. Log `consent_given` with timestamp + consent version.

---

## 4. Screening / Demographics (entry)

> ## A few quick questions
> This helps us confirm you're eligible and understand who's taking part. (~1 minute.)

Eligibility-gating questions (if disqualifying answer, route to the polite-exit screen in §8):

| Field | Question | Options | Gate |
|---|---|---|---|
| screen_experience | How many years of programming experience do you have? | < 1 / 1–2 / 3–5 / 6–10 / 11+ | "< 1" → exit |
| screen_familiarity | Have you used an issue tracker, a team chat tool, AND Git/GitHub before? | Yes, all three / Some / None | "None" → exit |
| screen_prior_weld | Have you used Weld before? | No / A little / Yes, a lot | "Yes, a lot" → exit |

Non-gating background (can also be deferred to the exit survey — collect once, not twice):
- Student status, primary languages, specific tools used. *(If collected here, don't repeat in the exit survey.)*

> **[ Continue ]**

---

## 5. Pre-Task Orientation (in-app, before Phase 1)

> ## You're in! Here's the setup.
>
> You've just joined the **PetTracker** team — a small group building a pet-care app. Your teammates (Sarah, Alex, Priya, and Marcus) have been working for a couple of weeks, so there's already a project board, some team chat, and code activity to look at.
>
> A **task panel** on the [side] will guide you one step at a time. When you finish a step, click **"I'm done with this task"** to move on.
>
> There are no trick questions — just do each task the way that feels natural to you. Ready?
>
> **[ Start the tasks ]**

**Minimal feature primer** (show as a one-time dismissible overlay or inline tips — keep it light; discovery is part of what we measure, so don't over-teach):
- "This is your **board** — task cards in columns."
- "This is **chat** — your team's messages."
- "This is the **GitHub** view — issues and pull requests."
- Don't explain cross-references or any integration features. Whether participants discover those is a core measure.

---

## 6. Task Panel Copy (Phases 1–3)

Each task's text is the verbatim instruction from `WELD_AGENT_TASK_SCRIPTS.md`, loaded from `apps/web/src/lib/study-tasks.ts`. The panel shows one task at a time with a progress indicator ("Task 3 of 12" — 6 manual baseline, then 6 agent) and the **"I'm done with this task"** button. Phase transition interstitials:

**Before Phase 2:**
> ### Nice work — onboarding done.
> Next are a few tasks closer to real project work. Same idea: use the task panel, work however feels natural, and click "I'm done" when you finish each one.
> **[ Continue ]**

**Before Phase 3:**
> ### Last part: explore freely.
> You have **5 minutes** to poke around Weld however you like — try features you haven't used, customize things, or just look around. There's no specific task. We'll move you to a short survey when the time's up.
> **[ Start exploring ]**

**After Phase 3 (auto or on click):**
> ### Time's up — thanks for exploring!
> One short survey left and you're done.
> **[ Go to survey ]**

---

## 7. Exit Survey
Renders the survey from `WELD_EXIT_SURVEY.md`. Log `survey_started` / `survey_completed`.

---

## 8. End Screens

### 8a. Completion (success)
> ## All done — thank you! 🎉
> You've completed the study. Your responses are recorded.
> If you opted in to future research related to this study, we'll reach out via the email you provided.
> Questions? Contact awsadler@mtu.edu.

### 8b. Polite exit (ineligible)
> ## Thanks for your interest!
> Based on your answers, you're not eligible for this particular study, but we really appreciate you stopping by. No data has been collected.
> [Optional: "If you'd like to hear about future studies, you can leave your email here — entirely optional." Only if your IRB permits a contact list.]

### 8c. Abandonment (if detectable)
No screen needed, but log `session_abandoned` with the last completed task so partial data can be handled per the inclusion threshold.

---

## 9. Notes for Implementation
- Keep all this copy in a config/content file so wording can change without code edits.
- The feature primer (§5) should be deliberately minimal — over-teaching contaminates the discoverability measures.
- Every screen transition that matters for analysis (consent, screening pass/fail, phase changes, survey start/end, completion) emits a telemetry event per `../instrument/CLAUDE_STUDY.md` §5/§5A.
- Age (18+) is gated by the consent affirmation (§3); experience / tool familiarity / prior-Weld use are gated at screening (§4). The exit-survey demographics are a backstop, not the primary gate.
