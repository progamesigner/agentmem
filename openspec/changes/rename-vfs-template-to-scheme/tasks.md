## 1. Rename the type and module

- [ ] 1.1 Rename `src/template.rs` → `src/scheme.rs`; rename `Template` → `Scheme`, `TemplateError` → `SchemeError` (keep `RenderError`); update all internal doc comments that describe the concept as a "template" to "scheme"
- [ ] 1.2 Update `src/lib.rs`: `pub mod template;` → `pub mod scheme;` and any re-exports

## 2. Configuration

- [ ] 2.1 In `src/config.rs`: rename the constant `VAR_TEMPLATE` and its value `"AGENTMEM_VFS_TEMPLATE"` → `"AGENTMEM_VFS_SCHEME"`; rename the `Config.template` field → `Config.scheme`; update `build`/`from_env`, the `resolver()` constructor call, and the `describe()` output (`template = …` → `scheme = …`)
- [ ] 2.2 Rename the CLI override: `Cli.vfs_template` → `Cli.vfs_scheme`, flag `--vfs-template` → `--vfs-scheme`, help text, and the `as_overrides` mapping key
- [ ] 2.3 Update `src/config.rs` tests: `malformed_template_is_rejected` and the default-assertion tests that reference `Template`/`VAR_TEMPLATE`/the field name

## 3. Path, tools, storage, policy, server

- [ ] 3.1 In `src/path.rs`: rename the `PathResolver.template` field, the `template()` accessor → `scheme()`, the `PathResolver::new` parameter, and all `template`/"template" references (including the empty-`template` checks and doc comments) to "scheme"
- [ ] 3.2 In `src/tools.rs`: rename the `template()` accessor and its call sites, the `merge_schema`/`build_tools` parameter names, and doc comments describing "template-derived scope fields" → "scheme-derived scope fields"
- [ ] 3.3 Update remaining references in `src/storage.rs`, `src/policy.rs`, and `src/mcp.rs` (accessors, doc comments) to use "scheme"

## 4. Tests and snapshots

- [ ] 4.1 Update `tests/schema_snapshots.rs` and `tests/tools.rs`: `Template::parse` → `Scheme::parse`, imports, and any helper/local names referencing "template"
- [ ] 4.2 Run the snapshot tests and confirm the four `tests/snapshots/schema_snapshots__*.snap` files are unchanged in content; if a diff appears, review it and confirm it is purely incidental before accepting via `cargo insta`

## 5. Docs and verification

- [ ] 5.1 Update `README.md`: the env-var table entry `AGENTMEM_VFS_TEMPLATE` → `AGENTMEM_VFS_SCHEME` and any prose describing the "VFS template"
- [ ] 5.2 Sweep for stragglers: `grep -rni "template" src/ tests/ README.md` and confirm every remaining hit refers to JSON-schema generation (`schemars`) or is otherwise unrelated to the VFS path concept
- [ ] 5.3 Run `cargo test` and `cargo clippy`; ensure `openspec validate rename-vfs-template-to-scheme --strict` passes
