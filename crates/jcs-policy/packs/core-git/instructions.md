# Git conventions

- Commit in the imperative mood: `add`, `reject`, `remove` — not `added` or `adds`.
- Keep the subject under 72 characters. Put the reasoning in the body.
- A commit must be revertible on its own. If reverting it would break the build, it is
  probably two commits.
- Never rewrite history that has been pushed. Fix forward with a new commit.
- Stage intentional changes only. Do not `git add -A` in a tree with unrelated edits.
- If a change requires a data migration, say so in the body and reference the migration.
