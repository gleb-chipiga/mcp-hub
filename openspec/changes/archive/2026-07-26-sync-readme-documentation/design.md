## Context

The README already describes runtime behavior and contains the project roadmap, but
no specification requires those sections to change with the behavior or plan they
describe. This can leave the public documentation inconsistent with the project.

## Goals / Non-Goals

**Goals:**

- Make README synchronization an explicit, reviewable project requirement.
- Cover both runtime behavior and roadmap completion or removal.

**Non-Goals:**

- Automate README edits or validate prose mechanically.
- Change runtime behavior, MCP behavior, configuration, or release workflows.

## Decisions

### Add a dedicated project-documentation capability

The requirement applies across all runtime capabilities, so it belongs in one
cross-cutting capability rather than being copied into individual runtime specs.

### Require updates in the same change

The documentation change must accompany the implementation or planning decision.
Deferring it creates drift and makes review less reliable.

### Keep enforcement review-based

The requirements state the synchronization contract but do not add a prose linter or
automation. Determining whether behavior is user-visible and whether the explanation
is accurate requires project context.

## Risks / Trade-offs

- [Documentation changes add review work] -> Limit the requirement to README
  sections affected by the change.
- [A behavior change may be missed] -> Keep the requirement in the baseline OpenSpec
  so it is visible during proposal and review.
