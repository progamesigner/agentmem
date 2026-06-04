---
name: openspec-commit
description: Commit work during OpenSpec implementation in small, focused commits with plain descriptive commit titles (no "chore:"/"feat:" conventional-commit prefixes). Use while implementing or applying an OpenSpec change, after completing a task or a small logical unit of work.
---

Commit work in small, focused chunks while implementing an OpenSpec change.

**When to use**: After completing each OpenSpec task (or a small, self-contained logical unit of work within a task). Commit early and often — do not batch many tasks into one large commit.

**Commit title rules**

- Write a plain, descriptive sentence that says what the change does.
- Use the imperative mood: "Add user session timeout", "Fix race condition in cache eviction", "Rename token field to access_token".
- **Do NOT use conventional-commit prefixes.** No `chore:`, `feat:`, `fix:`, `refactor:`, `docs:`, etc. The title is just the description itself.
- Keep it concise (aim for ≤ 72 characters) but specific — describe the actual change, not the task number.

  | ❌ Avoid | ✅ Use |
  |---------|--------|
  | `chore: update tasks` | `Mark login validation task complete` |
  | `feat: add auth` | `Add password reset email flow` |
  | `fix: bug` | `Prevent duplicate submission on slow networks` |

**Steps**

1. **Stage only the relevant changes**
   - Review what changed: `git status` and `git diff`.
   - Stage the files that belong to this one logical unit. Avoid `git add -A` if unrelated changes are present.

2. **Write the commit**
   - Title: a plain descriptive sentence (see rules above).
   - Optional body: brief context or the OpenSpec change/task it relates to, e.g. a line like `Change: <change-name>` or which task it completes.

   ```bash
   git commit -m "Add password reset email flow" -m "Change: add-password-reset (task 3/7)"
   ```

3. **Keep commits small**
   - One task → one commit is the default. If a task is large, split into multiple descriptive commits.
   - Each commit should leave the code in a coherent state.

**Guardrails**
- Never use `chore:`/`feat:`/`fix:` style prefixes — the title is the description.
- Commit only when the user has asked to commit, or per their standing instruction to commit per task during implementation.
- If on the default branch and unsure whether to branch, ask first.
- Do not push unless the user asks.
