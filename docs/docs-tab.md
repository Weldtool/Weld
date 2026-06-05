# Docs Tab — Design Notes

> Status: **decided, not yet implemented.**

## Overview

A first-class Docs tab giving teams a unified place to manage both internal documentation and GitHub repo files — without leaving Weld.

## Supported Formats

| Format | Extension | Editor | Internal | GitHub |
|--------|-----------|--------|----------|--------|
| Markdown | `.md` | Tiptap (rich) | Create + Edit | Create + Edit |
| Plain Text | `.txt` | CodeMirror | Create + Edit | Create + Edit |
| MDX | `.mdx` | CodeMirror | Create + Edit | Create + Edit |
| reStructuredText | `.rst` | CodeMirror | Create + Edit | Create + Edit |

**Two editor modes, not four.** Markdown gets the existing Tiptap rich editor with WYSIWYG toolbar. MDX, RST, and TXT use CodeMirror with per-format syntax highlighting.

## Storage

**Internal docs** are stored in a `documents` table scoped to a project:

```
documents {
    id: Uuid,
    project_id: Uuid,
    title: String,
    content: String,           -- raw source (Markdown / RST / MDX / TXT)
    format: DocFormat,         -- Markdown | PlainText | MDX | RST
    created_by: Uuid,
    created_at: DateTime,
    updated_at: DateTime,
    deleted_at: Option<DateTime>,
}
```

**GitHub docs** are fetched live from the GitHub Contents API and cached per session. No duplicate storage — Weld is a read/write proxy to the repo.

## GitHub File Operations

Uses GitHub's Contents API:
- `GET /repos/{owner}/{repo}/contents/{path}` — read file or directory listing
- `GET /repos/{owner}/{repo}/readme` — get the repo README
- `PUT /repos/{owner}/{repo}/contents/{path}` — create or update a file (requires commit message; update requires current file SHA to prevent conflicts)

When a user opens a GitHub file for editing, its SHA is fetched and held in state. On save, the SHA is included in the PUT request.

## GitHub Edit Flow — User Choice

When a project has a linked GitHub repo, users choose their preferred edit flow in project settings:

**Direct commit** — edits save straight to the target branch with a commit message. Simple, fast, suits solo projects or trusted collaborators.

**Branch + PR** — edits are committed to a new branch and a PR is opened automatically. Safer for team repos; integrates naturally with Weld's existing PR machinery.

The setting lives on the project (`github_doc_edit_flow: DirectCommit | BranchPR`) and can be changed at any time.

## UI Structure

```
/projects/[id]/docs
├── Sidebar
│   ├── Internal (tree view of documents)
│   └── GitHub (file browser rooted at repo root, lazy-loaded)
├── Editor pane (Tiptap or CodeMirror depending on format)
└── Save dialog
    ├── Internal: title + save button
    └── GitHub: file path + commit message (+ branch name if BranchPR flow)
```

The unified list on the docs landing page shows all docs (internal + GitHub) with a source badge. GitHub docs show the file path; internal docs show the title.

## What's Not In Scope (Yet)

- Word / PDF / AsciiDoc support
- Full-text search across docs
- Doc versioning for internal docs (GitHub history covers the GitHub side)
- Embeds or live previews of MDX component blocks
