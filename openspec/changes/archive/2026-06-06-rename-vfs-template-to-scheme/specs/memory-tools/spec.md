## MODIFIED Requirements

### Requirement: Common tool input contract
The system SHALL ensure every tool's input schema includes the scope parameters whose names are the placeholder idents of `AGENTMEM_VFS_SCHEME`, and SHALL reject calls whose scope arguments do not satisfy that contract.

#### Scenario: All scheme keys required
- **WHEN** scheme is `<agent>.<user>` and a tool is called with `agent` set but `user` missing
- **THEN** the call is rejected with code `missing_scope` and the message names `user`

#### Scenario: Unexpected scope parameter
- **WHEN** scheme is `<agent>` and a tool is called with both `agent` and `user`
- **THEN** the call is rejected at schema validation because the input schema does NOT include `user` under this scheme

#### Scenario: Custom scheme keys are honoured
- **WHEN** scheme is `<team>.<agent>.<env>.<user>` and a tool is called with exactly those four fields
- **THEN** the call proceeds to resolution with the rendered suffix `<team>.<agent>.<env>.<user>`

#### Scenario: Empty scheme requires no scope arguments
- **WHEN** scheme is the empty string and a tool is called with no scope fields
- **THEN** the call proceeds; if any scope field is supplied, the call is rejected at schema validation
