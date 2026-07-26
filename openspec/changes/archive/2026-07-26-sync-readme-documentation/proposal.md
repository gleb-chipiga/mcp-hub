## Why

The README can drift from implemented runtime behavior and from decisions recorded
in its roadmap. The project needs a durable requirement that keeps both sections
current whenever their underlying behavior or plans change.

## What Changes

- Add a project documentation capability that requires runtime behavior changes to
  update the README `Operational behavior` section in the same change.
- Require README roadmap items to be updated when they are implemented or removed
  from the plan.

## Capabilities

### New Capabilities

- `project-documentation`: Keeps the README operational behavior and roadmap aligned
  with implementation and planning decisions.

### Modified Capabilities

- None.

## Impact

- Affects the README and OpenSpec documentation process only.
- Does not change runtime code, the MCP API, configuration, or dependencies.
