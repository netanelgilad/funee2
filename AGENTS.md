# Agent Working Agreement

This repository follows an explicit test-driven and approval-driven workflow for the AI gateway effort.

## Workflow

1. Write exactly one focused test for the next behavior.
2. Stop and request user review/approval of that test.
3. Revise the test until approved.
4. Implement only what is needed for that approved test.
5. Stop and request user review/approval of the implementation.
6. Revise implementation until approved.
7. Move to the next behavior and repeat.

## Scope for Current Initiative

- Build a minimal OpenAI-compatible AI gateway in funee.
- Start with mock upstream in tests (no real provider dependency).
- Expand behavior incrementally through TDD.

## Collaboration Rules

- Prefer smallest possible changes per step.
- Keep test intent readable and explicit.
- Do not batch multiple behavior changes into one step.
- Self-hosted tests must import only host modules (`host://...`) and relative repo files, never from `"funee"`.
- Assertions should use `otherwise` when extra failure context would make the error self-explanatory.
- Ask for feedback at each checkpoint (test, then implementation).
- If implementation of an approved step uncovers a missing prerequisite (runtime/API gap), pause and recurse through the same workflow for that prerequisite: one focused test, review/approval, minimal implementation, review/approval, then return to the original step.
