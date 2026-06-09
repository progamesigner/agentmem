## ADDED Requirements

### Requirement: `recall_memory_notes` tool registration
The system SHALL register a `recall_memory_notes` tool alongside the existing memory
tools whenever the recall backend is not `off`. Its scope extraction and visibility
semantics SHALL match `list_memory_notes` exactly: it returns only results the caller
could otherwise reach via `list_memory_notes` + `read_memory_note`, and never results
from another scope or from an ignored/hidden note.

#### Scenario: Tool is listed when recall is enabled
- **WHEN** the server starts with a recall backend other than `off`
- **THEN** `recall_memory_notes` appears in the tool listing alongside the existing
  memory tools, taking the same scope keys

#### Scenario: Visibility matches list_memory_notes
- **WHEN** `recall_memory_notes` and `list_memory_notes` are invoked for the same scope
  and policy
- **THEN** every path returned by `recall_memory_notes` is one that `list_memory_notes`
  would also return for that scope; no path outside that visible set ever appears
