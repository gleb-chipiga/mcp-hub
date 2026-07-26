# project-documentation Specification

## Purpose

Defines how the README operational behavior and roadmap remain aligned with
implemented project behavior and planning decisions.
## Requirements
### Requirement: README operational behavior stays current
The project SHALL update the `Operational behavior` section of the README in the
same change that alters user-visible runtime behavior.

#### Scenario: Runtime behavior changes
- **WHEN** a change adds, removes, or changes an observable runtime behavior
- **THEN** the change updates the README `Operational behavior` section to describe
  the resulting behavior

### Requirement: README roadmap reflects resolved items
The project SHALL update the README roadmap in the same change that implements or
removes a roadmap item.

#### Scenario: Roadmap item is implemented
- **WHEN** a change implements behavior listed in the README roadmap
- **THEN** the change updates that roadmap item to reflect its completion

#### Scenario: Roadmap item is removed
- **WHEN** the project decides not to pursue behavior listed in the README roadmap
- **THEN** the change removes or updates that roadmap item to reflect the decision
