## 1. Un-gate YAML

- [x] 1.1 Make `serde_yaml` a required dependency in `Cargo.toml` (drop it from the `recall-tantivy` feature list; update the feature comment), refresh `Cargo.lock`, and remove the feature gate from `src/frontmatter.rs`'s module declaration.

## 2. Frontmatter serialization

- [x] 2.1 Add `frontmatter::merge(content, updates: &serde_json::Map) -> Result<String, …>` that parses the existing block (refusing a fence that does not parse), applies upserts and `null`-deletes, re-serializes `---\n<yaml>\n---\n` with stable key order (omitting the block when empty), and re-attaches the byte-identical body.
- [x] 2.2 Unit tests: upsert/delete merge, block creation, block removal when emptied, malformed-fence refusal, nested values and arrays round-tripping, body byte-identity (including CRLF fences).

## 3. Tools

- [x] 3.1 Add `PropertiesReadFields { path }` and `PropertiesUpdateFields { path, properties }` schemars structs; register `read_note_properties` and `update_note_properties` in `TOOL_NAMES`/`build_tools`; dispatch in `Toolbox::call`.
- [x] 3.2 Read handler: gate exactly like `read_memory_note`, parse via `frontmatter::parse`, return `{ properties }`.
- [x] 3.3 Update handler: reject root-reserved paths; gate writes by policy/visibility; require the file to exist; apply `frontmatter::merge` under `read_modify_write`; call `recall_on_write`; return the post-update `{ properties }`.

## 4. Integration tests

- [x] 4.1 `tests/tools.rs`: read/update round trip, body untouched, emptied-block removal, malformed-fence `invalid_argument`, wrapper-reserved and policy/visibility/`not_found` parity.
- [x] 4.2 Tantivy integration: updated property immediately matches a `filters` recall (`--features recall-tantivy`).
- [x] 4.3 Verify the default build (no features) compiles with the tools present; update schema snapshots and the README tool table.

## 5. Verification

- [x] 5.1 Run `cargo fmt --check`, `cargo clippy --all-targets --all-features`, `cargo test`, and `cargo test --all-features`.
