## MODIFIED Requirements

### Requirement: README operational behavior stays current
The project SHALL update the `Operational behavior` section of the README in the
same change that alters user-visible runtime behavior, including the lifecycle
semantics of routed tool calls.

#### Scenario: Runtime behavior changes
- **WHEN** a change adds, removes, or changes an observable runtime behavior
- **THEN** the change updates the README `Operational behavior` section to describe
  the resulting behavior

#### Scenario: Tool-call lifecycle forwarding is implemented
- **WHEN** the hub forwards cancellation and progress for routed tool calls
- **THEN** the README states that cancellation is best effort
- **AND** the README states that progress is forwarded only for a client-supplied progress token
- **AND** the README states that calls still return one final result without a custom streamed-result protocol

### Requirement: README roadmap reflects resolved items
The project SHALL update the README roadmap in the same change that implements or
removes a roadmap item.

#### Scenario: Roadmap item is implemented
- **WHEN** a change implements behavior listed in the README roadmap
- **THEN** the change updates that roadmap item to reflect its completion

#### Scenario: Tool-call lifecycle roadmap item is implemented
- **WHEN** the hub implements the README's tool-call cancellation and progress roadmap item
- **THEN** the README no longer presents that item as pending roadmap work

#### Scenario: Roadmap item is removed
- **WHEN** the project decides not to pursue behavior listed in the README roadmap
- **THEN** the change removes or updates that roadmap item to reflect the decision
