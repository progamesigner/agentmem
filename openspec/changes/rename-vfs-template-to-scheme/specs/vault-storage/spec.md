## RENAMED Requirements

- FROM: `### Requirement: VFS template resolution`
- TO: `### Requirement: VFS scheme resolution`

## MODIFIED Requirements

### Requirement: Vault root containment
The system SHALL canonicalise every virtual path against the configured vault root and SHALL reject any resolution whose canonical absolute path is not a descendant of that root.

#### Scenario: Traversal attempt is rejected
- **WHEN** a tool is called with virtual path `../../etc/passwd`
- **THEN** the operation is refused with a structured error of code `path_escapes_root` before any filesystem call is issued

#### Scenario: Symlink escape is rejected
- **WHEN** a symlink inside the vault points to a path outside the vault root, and a tool resolves to that symlink
- **THEN** the operation is refused with code `path_escapes_root`

#### Scenario: Legitimate path inside root is accepted
- **WHEN** a tool is called with a virtual path that resolves under `AGENTMEM_ROOT_DIR`
- **THEN** the operation proceeds to scheme resolution and policy enforcement

### Requirement: VFS scheme resolution
The system SHALL, on every tool call, validate that the supplied scope arguments exactly match the placeholder idents of the configured `AGENTMEM_VFS_SCHEME`, and SHALL render the scheme into a single string used as both the per-scope directory segment under the agents folder and the dotted suffix appended to the file stem inside the agents folder.

#### Scenario: Default scheme resolves agent and user
- **WHEN** scheme is `<agent>.<user>`, scope is `{agent:"coder", user:"alice"}`, agents folder is `Agents`, and virtual path is `tasks/plan.md`
- **THEN** the resolved physical path is `<root>/Agents/coder.alice/tasks/plan.coder.alice.md`

#### Scenario: Single-key scheme
- **WHEN** scheme is `<agent>`, scope is `{agent:"coder"}`, agents folder is `Agents`, and virtual path is `HEARTBEAT-STATE.md`
- **THEN** the resolved physical path is `<root>/Agents/coder/HEARTBEAT-STATE.coder.md`

#### Scenario: Multi-key scheme
- **WHEN** scheme is `<team>.<agent>.<env>.<user>`, scope is `{team:"platform", agent:"coder", env:"prod", user:"alice"}`, agents folder is `Agents`, and virtual path is `tasks/plan.md`
- **THEN** the resolved physical path is `<root>/Agents/platform.coder.prod.alice/tasks/plan.platform.coder.prod.alice.md`

#### Scenario: Scheme with literal segment
- **WHEN** scheme is `v1.<agent>.<user>`, scope is `{agent:"coder", user:"alice"}`, agents folder is `Agents`, and virtual path is `tasks/plan.md`
- **THEN** the resolved physical path is `<root>/Agents/v1.coder.alice/tasks/plan.v1.coder.alice.md`

#### Scenario: Empty scheme applies no suffix
- **WHEN** scheme is the empty string and virtual path is `notes.md`
- **THEN** the resolved physical path is `<root>/<agents_dir>/notes.md` with no per-scope directory and no suffix

#### Scenario: Vault root as agents folder
- **WHEN** `AGENTMEM_AGENTS_DIR=.`, scheme is `<agent>.<user>`, scope is `{agent:"coder", user:"alice"}`, virtual path is `tasks/plan.md`
- **THEN** the resolved physical path is `<root>/coder.alice/tasks/plan.coder.alice.md` and the "outside the agents folder" region is empty

#### Scenario: Missing required scope key
- **WHEN** scheme is `<agent>.<user>` and a tool is called with `agent` set but `user` missing
- **THEN** the call is rejected with code `missing_scope` and a message naming `user`

#### Scenario: Extra scope key
- **WHEN** scheme is `<agent>` and a tool is called with both `agent` and `user`
- **THEN** the call is rejected at schema validation because the input schema does NOT include `user` under this scheme
