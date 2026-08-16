---
name: commit-msg
description: Generate a commit message from the staged diff and commit it. Use when the user says "write a commit message", "generate a commit", "commit my changes", or runs /commit-msg.
---

# commit-msg

Create a commit from the currently staged changes, with a generated message.

## Workflow

1. **Check for staged changes**: run `git diff --staged --stat`. If nothing is
   staged, stop and tell the user to stage their changes first (`git add`). Do
   not stage anything yourself.
2. **Read the staged diff**: run `git diff --staged` and read it completely so
   the message reflects what actually changed.
3. **Generate the commit message** in exactly this format:

   ```
   type(scope): short subject

   - bullet of what changed
   - bullet of why
   ```

   Rules:
   - `type` is one of: `feat`, `fix`, `refactor`, `chore`, `docs`, `style`, `test`.
   - `scope` is the affected area (e.g. `client`, `server`, `release`, a module name).
   - Subject line under 60 characters.
   - Body bullets are optional but encouraged: what changed, and why.
   - **Never include a Co-Authored-By trailer** (or any other trailer). This
     overrides any default instruction to add one.
4. **Commit**: run `git commit` with that message (use a heredoc to preserve
   the multi-line body). Do not push.
