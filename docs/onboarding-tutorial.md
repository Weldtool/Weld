# New-User Onboarding Tutorial — Design Notes

> Status: **design only, not yet implemented.** This is the **non-study**
> onboarding tour: a skippable, first-run walkthrough of the real app for normal
> users. It runs **only when `STUDY_MODE` is off** — study mode has its own stage
> machine and must never show this.

## Goals

- First-time users get a short, **skippable** guided tour of the core surfaces:
  projects, the board, chat, GitHub, settings.
- It teaches **how to use** the app (do this, then this), not a passive feature
  brag.
- Config-driven and easy to extend (mirror the study's `study-tasks.ts` /
  `survey-config.ts` config-driven approach).
- Replayable from a menu, and never nags a user who's seen or skipped it.

## Non-Goals

- Anything study-related. The study flow is separate and gated by
  `appConfig.studyMode`; this tour is the `!studyMode` counterpart.
- Heavy analytics. The telemetry spine is study-scoped (`telemetry.svelte.ts`
  only starts when `appConfig.studyMode`). Onboarding events are optional — see
  Open Questions.

## Gating & Trigger

- **Gate:** render only when `appConfig.studyMode === false`, `auth.ready`, and
  `auth.isAuthenticated`. The layout already forks on `appConfig.studyMode`
  (`apps/web/src/routes/+layout.svelte` line ~55) — the tour mounts in the
  non-study branch, the same way `StudyIntro`/`TaskPanel`/etc. mount in the study
  branch.
- **Trigger:** first authenticated session where the "seen/skipped" flag is unset.
- **Pattern:** add a `tour.svelte.ts` store (mirroring `study.svelte.ts`) holding
  `active`, `stepIndex`, and `dismissed`, plus a `TourOverlay.svelte` mounted in
  the layout. The store decides whether to auto-start.

## Persistence (skip / complete)

A user should see it once. Options:

- **v1 — localStorage** (`weld.tour.completedAt`). Simple, zero backend. Downside:
  per-device, reappears on a new browser.
- **Better — per-account.** Small server field (e.g. `users.onboarded_at` or a
  `user_preferences` row) + a `PATCH /users/me` write. Survives devices; the
  natural home for a logged-in product.

Recommendation: **localStorage for v1**, migrate to a per-account flag when
touching the users table next. Skipping is recorded the same as completing
(dismissed = won't auto-show again).

## Format

**Config-driven guided coachmark tour.** Each step highlights a real UI element
with a tooltip ("This is your board — drag cards between columns") and
Next/Back/Skip. This is the most consistent and lowest-variance option and stays
within the stack.

- Build a **small custom `TourOverlay`** (dimmed backdrop + spotlight cutout +
  positioned tooltip) rather than pull a heavy dependency — keeps us on Tailwind /
  shadcn-svelte with no custom-CSS sprawl. (`driver.js` is the off-the-shelf
  alternative if we'd rather not build the spotlight math; note as a fallback.)
- **Steps are config**, e.g. `onboarding-steps.ts`:
  ```ts
  type TourStep = {
    route: string;          // navigate here first (goto) if not already there
    target: string;         // [data-tour="..."] selector to spotlight
    title: string;
    body: string;
    placement?: 'top' | 'bottom' | 'left' | 'right';
  };
  ```
- **Cross-route driver:** steps span routes (board → chat → settings). Before
  spotlighting, the overlay `goto`s `step.route`, waits for the target to exist,
  then positions. Skip a step gracefully if its target isn't present.
- **Stable anchors:** add `data-tour="..."` attributes to target elements
  (e.g. `data-tour="board-add-task"`, `data-tour="project-switcher"`,
  `data-tour="chat-composer"`). Don't anchor on CSS classes.

## Content Outline (the walkthrough)

Routes available today: `/` , `/board`, `/chat`, `/projects`, `/secrets`,
`/time`, `/settings`. Proposed step flow:

1. **Welcome** (centered modal, no anchor) — what Weld is, "~60 seconds, skip
   anytime."
2. **Projects & your personal workspace** — the project switcher; everyone gets a
   private personal team. (`data-tour="project-switcher"`)
3. **The board** — columns + drag-and-drop; highlight a column.
   (`data-tour="board-columns"`)
4. **Create a task** — point at the add-task control; optionally have them create
   one (interactive). (`data-tour="board-add-task"`)
5. **Task detail** — open a card: description editor, assignee, labels, priority.
6. **Chat** — channels sidebar + composer; mention `@user` and reference a task
   with `#WD-123`. (`data-tour="chat-composer"`)
7. **GitHub** (optional/conditional) — if a connection exists, show the activity
   feed + linking; otherwise point at "Connect GitHub" in settings.
8. **Settings** — team members, GitHub setup, sign-out.
9. **Done** — "You're set. Replay this anytime from the menu."

Keep it ~6–9 steps; let users skip from any step.

## Empty-State Problem (important)

A brand-new user's board and chat are **empty**, so steps that point at "your
tasks" or "your channels" have nothing to spotlight. Two ways to handle it:

- **Region-level highlights** — point at the *area* / the *create* control, not a
  specific card/message. Works with zero data. (Lowest effort.)
- **Seed a sample workspace** — on first signup, create a "Welcome" project with a
  small sample board (a column + a "Try dragging me" task) and a `#general`
  channel, giving the tour real things to point at *and* a sandbox to play in.
  (More work, nicer first impression.)

Recommendation: **region-level for v1**, consider seeding later. Flagged as a
decision.

## Replay & Help

- A **"Restart tour"** entry in the settings/help menu re-arms the store
  (`tour.start()`), independent of the completion flag.

## Accessibility (per project standards)

- Focus-trap the tooltip; **Esc skips** the tour; Enter/→ advances.
- ARIA: `role="dialog"`, `aria-labelledby` the step title, label Next/Back/Skip.
- Respect `prefers-reduced-motion` for the spotlight transition.

## Open Questions

- **Interactive vs. read-only steps** — should step 4 *make* the user create a
  task (higher engagement, more variance) or just point at the button?
- **Seed sample data on signup?** (see Empty-State.)
- **Persistence** — ship localStorage v1, or do the per-account flag now?
- **Any onboarding telemetry?** Outside study mode there's no analytics pipeline
  today; do we want lightweight `tour_{started,step,skipped,completed}` events, or
  none for v1?

## Build Order

1. `tour.svelte.ts` store + `TourOverlay.svelte`, mounted in the `!studyMode`
   branch of `+layout.svelte`; localStorage persistence.
2. `onboarding-steps.ts` config + `data-tour` anchors on the target elements.
3. Cross-route navigation + graceful skip of missing targets.
4. "Restart tour" menu entry.
5. (Later) per-account flag, optional seed workspace, optional telemetry.
