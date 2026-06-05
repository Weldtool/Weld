# GitHub Integration Expansion — Design Notes

> Status: **under consideration, not yet decided or implemented.**

## Vision

The goal is for teams to create a repo, link it to Weld, and rarely if ever need to visit GitHub directly. The core daily developer workflow — branches, code, PRs, issues, Actions, settings — would all be accessible and manageable from within Weld.

The existing integration (Phase 6) covers webhooks, task↔issue/PR linking, two-way sync, and the activity feed. This document covers what would be needed to go further.

---

## What's Fully Achievable via GitHub API

### Code & Files
- File browser rooted at repo root (lazy directory loading)
- File viewer and editor (covered by the Docs tab for `.md`/`.txt`/`.mdx`/`.rst`; extendable to any text file via CodeMirror)
- Commit history and diffs
- Branch creation, deletion, and default branch setting

### Branch Protection
- Require PR reviews before merge
- Require status checks to pass
- Restrict who can push to a branch
- Require linear history / signed commits

### Pull Requests
- Create, view, and merge PRs (basic flow already shipped)
- Line-level review comments with suggestion blocks
- Approve / request changes / comment reviews
- PR checklist items and description templates
- Merge strategy selection (merge commit / squash / rebase)

### Issues
- Full CRUD — create, edit, close, reopen
- Labels, assignees, milestones
- Issue templates

### GitHub Actions
- View workflow runs and status
- Trigger workflow dispatches manually
- View step-level logs
- Edit workflow YAML files via the Contents API

### Releases & Tags
- Create and edit releases
- Upload release assets
- Tag management

### Repo Settings
- Description, topics, homepage URL
- Visibility (public / private)
- Feature toggles (issues, projects, wiki, discussions)
- Collaborator and team access management
- Webhook management

---

## Partial or Harder

| Area | Gap | Mitigation |
|------|-----|-----------|
| Actions visualization | GitHub's run graph (parallel steps, timings) is rich UI built on top of the logs API | Surface pass/fail + logs; skip the visual graph initially |
| Dependabot / code scanning | API exists but GitHub's security dashboard is deeply integrated | Show alert counts and link out for triage |
| GitHub Pages | API covers basic config | Low priority; add if requested |
| Org-level billing / SSO | Not exposed via standard API | Out of scope |

---

## Suggested Surfaces to Add

These would be added progressively as new tabs or panels on the project view, reusing the existing GitHub connection and webhook infrastructure:

1. **Code tab** — file browser + editor (shares work with the Docs tab)
2. **Branches tab** — list, create, delete, configure protection rules
3. **Actions tab** — workflow run list, logs, manual trigger
4. **Releases tab** — release list, create/edit
5. **Repo Settings panel** — within the existing project settings page

The existing PRs and Issues panels (Phase 6) would be expanded with the richer review and management capabilities noted above.

---

## The Realistic Bar

"Never go to GitHub again for day-to-day work" is achievable. GitHub's web UI has years of polish on things like PR review diff rendering and Actions run visualization — Weld wouldn't need pixel-perfect parity, but a good-enough implementation of each surface would cover the vast majority of what developers actually do in a week.

The foundation (App installation, token management, webhook processing, octocrab client) is already in place. Each surface above is an incremental addition rather than a rearchitecture.
