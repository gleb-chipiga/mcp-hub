## MODIFIED Requirements

### Requirement: Hub routes tool calls to the correct upstream tool
The hub SHALL resolve an outward tool name to its owning upstream server and
original tool name, invoke that exact upstream tool, and return its one final
result through the outward `tools/call` response. The hub SHALL maintain
correlation for each active routed call until its upstream request resolves or
the hub session shuts down.

#### Scenario: Call a routed upstream tool
- **WHEN** a client calls an outward tool exposed by the hub
- **THEN** the hub invokes the matching original tool on the matching upstream server

#### Scenario: Routed call returns one final result
- **WHEN** an upstream completes a routed tool call
- **THEN** the hub returns that final result through the originating outward `tools/call`
- **AND** the hub does not emit a custom partial-result or streamed-result protocol

## ADDED Requirements

### Requirement: Hub forwards cancellation for active routed tool calls
The hub SHALL forward a client `notifications/cancelled` notification for an
active outward `tools/call` to that call's owning upstream using the upstream
request ID. It SHALL preserve the cancellation `reason` and `_meta` fields and
SHALL treat forwarding as best effort.

#### Scenario: Cancellation reaches the owning upstream
- **WHEN** a client cancels an active routed `tools/call` after its upstream request ID is known
- **THEN** the hub sends exactly one standard `notifications/cancelled` notification to that owning upstream
- **AND** the notification uses the upstream request ID while preserving the client cancellation reason and metadata
- **AND** the hub does not send that cancellation notification to any other upstream

#### Scenario: Cancellation races upstream request creation
- **WHEN** a client cancels a routed `tools/call` after the hub has registered the call but before the upstream request ID is available
- **THEN** the hub retains the cancellation until the upstream request ID is available
- **AND** the hub then forwards exactly one standard cancellation notification to the owning upstream

#### Scenario: Cancellation cannot be correlated to an active tool call
- **WHEN** the hub receives a cancellation without a request ID or for an unknown, completed, or non-tool request
- **THEN** the hub does not forward it to an upstream
- **AND** the hub continues serving the outward session

#### Scenario: Upstream does not stop after cancellation
- **WHEN** an upstream has already completed the call or ignores a forwarded cancellation notification
- **THEN** the hub does not retry, terminate, or restart that upstream on behalf of the cancelled call
- **AND** the cancellation attempt does not change routing for other active calls

### Requirement: Hub forwards qualified upstream progress notifications
The hub SHALL forward a standard upstream `notifications/progress` notification
only when it belongs to an active routed `tools/call` for which the client
supplied a valid MCP progress token. The outward notification SHALL use the
client-supplied token and preserve the upstream progress value, total, message,
and metadata.

#### Scenario: Progress from an active tokenized call is forwarded
- **WHEN** an upstream emits progress for an active routed call whose outward request supplied a valid progress token
- **THEN** the hub sends a standard `notifications/progress` notification to the originating client
- **AND** the notification uses the token supplied by that client
- **AND** its progress value, optional total, message, and metadata match the upstream notification

#### Scenario: Progress does not cross upstream or call boundaries
- **WHEN** multiple routed calls are active and an upstream emits a progress notification
- **THEN** the hub forwards it only to the client correlation registered for that upstream request
- **AND** the hub does not forward it as progress for another active call or upstream

#### Scenario: Progress without a client token is ignored
- **WHEN** an upstream emits progress for a routed call whose outward request did not supply a valid progress token
- **THEN** the hub does not send an outward progress notification
- **AND** the call may still return its one final result normally

#### Scenario: Progress after cancellation or completion is ignored
- **WHEN** an upstream emits progress after the hub has correlated the call as cancelled or has removed it after completion
- **THEN** the hub does not send that progress notification outward
