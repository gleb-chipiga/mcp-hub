## Purpose

Defines MCP protocol-version negotiation behavior exposed by `mcp-hub`.

## Requirements

### Requirement: Hub behaves predictably across MCP protocol-version edges
The hub SHALL negotiate supported MCP protocol versions with real peers and SHALL preserve `rmcp`'s version-selection behavior for future or otherwise unknown client versions.

#### Scenario: Supported client versions negotiate cleanly
- **WHEN** an `rmcp` client initializes against the hub using a supported MCP protocol version
- **THEN** the hub completes initialization and reports the negotiated version expected by the protocol stack

#### Scenario: Future client versions negotiate down to the hub's supported version
- **WHEN** an `rmcp` client initializes against the hub using an unknown future protocol version string
- **THEN** the hub still completes initialization and the negotiated outward server version is reduced to the hub's supported protocol version
