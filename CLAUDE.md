# gitxtend - Agent Instructions (Claude)

This file is loaded by Claude Code on every session in this repository.
Read it once at session start; the constraints below apply for the rest
of the session unless there is explicit human authorization to deviate.

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
