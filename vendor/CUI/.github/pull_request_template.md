<!-- Read docs/CONTRIBUTING.md first. Humans and AI agents fill in the same template.
     Label claims as verified (ran it, output below), inferred (from code, cite file:line) or unverified. -->

## What and why

<!-- The problem and how this change solves it. Link the issue: Fixes #123 -->

## Disclosure

```text
Author:       Human | AI agent | Human with AI assistance
AI model:     exact model name and version, or "none"
AI tool:      Claude Code, Cursor, Copilot, Codex CLI, a script..., or "none"
AI role:      found / analyzed / wrote the code / wrote this description / reviewed
Human review: yes / no
```

## How it was tested

<!-- The exact commands you ran and the tail of their output, pasted verbatim.
     At least the ctest summary line. Say which commit you tested. -->

```text

```

## Platforms tested

<!-- OS, compiler and renderer you ran it on. CI covers the rest. -->

## Screenshots

<!-- Before and after, light and dark, for any visual change. Delete if not visual. -->

## Risks and what was not tested

## Checklist

- [ ] Built and ran `ctest` on at least one platform; the output above is verbatim
- [ ] Ran the sanitizer build, or explained why it was not needed
- [ ] Regenerated and looked at the documentation images, or this change is not visual
- [ ] Updated the header comment and docs if behavior or the API changed
- [ ] Commits written in part by an AI carry a `Co-Authored-By` trailer naming the model
- [ ] No AI tool files added (`CLAUDE.md`, `AGENTS.md`, `.cursorrules`, editor settings)
