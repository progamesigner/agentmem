# Tasks

## 1. Dependency + manifest bump

- [x] 1.1 In `Cargo.toml`, bump `tantivy = { version = "0.22.0", optional = true }` to `version = "0.26.1"`.
- [x] 1.2 In `Cargo.toml`, raise `rust-version` from `1.85` to `1.95`.
- [x] 1.3 Refresh `Cargo.lock` (`cargo update -p tantivy --precise 0.26.1`, then `cargo check --features recall-tantivy`). Confirm `lru ≥ 0.16.3` and `time = 0.3.47` in the lock.

## 2. Migrate the tantivy backend (`src/recall/tantivy.rs`)

- [x] 2.1 Build with the feature on (`cargo build --features recall-tantivy`) and resolve each API break at the known touchpoints: schema builder/field flags, `Index::create_in_ram` + writer/reader construction (`ReloadPolicy`), `IndexWriter` (`delete_term`/`Term::from_field_text`/`add_document`/`commit`), `IndexReader::reload`, `searcher` (`num_docs`/`search`/`doc::<TantivyDocument>`), `QueryParser`/`AllQuery`/`TopDocs`, `TantivyDocument`/`OwnedValue`/`get_first`, and `SnippetGenerator`.
- [x] 2.2 Make the smallest changes that compile; do not refactor unrelated code. If shared recall types in `src/recall/mod.rs` or wiring in `src/tools.rs`/`src/config.rs` must change, keep edits minimal and localized.

## 3. Verify behavior preserved

- [x] 3.1 Run the tantivy backend unit tests: `cargo test --features recall-tantivy recall::tantivy`. All must pass unchanged (BM25 rank+snippet, eq/contains/numeric filters, text+filter compose, regex over candidates, path-only hits, remove-then-flush).
- [x] 3.2 Run the recall-search integration tests with the feature on. Investigate and explicitly triage any ranking/snippet drift rather than adjusting test expectations to match.
- [x] 3.3 `cargo fmt --check`; `cargo clippy --all-targets --all-features`; full `cargo test --all-features`.

## 4. Confirm alerts closed

- [x] 4.1 Verify the working tree closes both Dependabot alerts: `lru ≥ 0.16.3` (GHSA-rhfx-m35p-ff5j) and `time = 0.3.47` (GHSA-r6v5-fh4h-64xc) in `Cargo.lock`.
- [x] 4.2 `openspec validate upgrade-tantivy-and-msrv --strict`. The `build-toolchain` delta (MSRV = 1.95) is the change's only requirement delta; the tantivy bump is behavior-preserving implementation under existing `recall-search` requirements.
