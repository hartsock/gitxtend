# gitxtend - Agent Instructions (general)

Mirror of `CLAUDE.md` for non-Claude agents (Codex, Gemini, local
models, etc.). When the two files disagree, `CLAUDE.md` is canonical for
Claude sessions and this file is canonical for everyone else.

## Model attribution

- If an LLM materially contributes to a commit, identify it with a
  `Co-authored-by` trailer in the commit message.
- Use the model/tool identity the session is actually running under. Do not
  credit a generic "AI Assistant".
- Known trailers:
  - `Co-authored-by: Codex <codex@openai.com>`
  - `Co-authored-by: Claude Opus 4.8 (1M context) <noreply@anthropic.com>`
- If multiple LLMs contribute to the same commit, include one trailer per
  contributing model.
