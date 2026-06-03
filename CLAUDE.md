# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Commands

```bash
cargo build                  # debug build
cargo build --release        # release build
cargo clippy                 # lint (must be clean — zero warnings)
cargo fmt                    # format (run before committing)
cargo test                   # run tests
```

Run the CLI directly during development:
```bash
cargo run -- <command>       # e.g. cargo run -- init
```

## Testing

Tests live inline as `#[cfg(test)]` modules inside each source file. Run with:

```bash
cargo test                   # run all tests (no network required)
cargo test cache             # run only cache tests
cargo test -- --test-threads=1  # serialize tests (needed if env-var races occur)
```

**Test isolation:** fetcher and installer tests set the `GDEP_CACHE_DIR` env var to a `TempDir` so the bare-clone cache never touches `~/.cache/gdep/`. All tests hold `test_helpers::CACHE_MUTEX` before mutating this env var.

**Synthetic repos:** tests build real git repos in memory using `git2` directly (`Repository::init`, `TreeBuilder`, `blob`, `commit`, `tag_lightweight`). No network, no fixture files. Three helpers in `src/test_helpers.rs`:
- `make_addons_repo(name, tag)` — repo with `addons/<name>/plugin.gd` (triggers installer strategy 1)
- `make_root_repo(tag)` — repo with `plugin.gd` at root (triggers installer strategy 3)
- `make_subdir_repo(subdir, tag)` — repo with `<subdir>/plugin.gd` (for `subdirectory` field testing)

## Architecture

gdep is a single-binary Rust CLI. The data flow for `gdep install` is:

```
gdep.toml  →  resolve refs (fetcher)  →  gdep.lock  →  extract files (installer)  →  project/addons/
```

**Key design decisions:**

- **Bare-clone cache** (`cache.rs`): repos are stored at `~/.cache/gdep/<djb2-url-hash>/` as bare clones. The hash is computed in `url_hash()`. No working tree is ever created.
- **Lockfile semantics** (`lockfile.rs`): tag/commit entries are frozen — `install` only re-fetches on cache miss. Branch entries are also frozen after first install; `gdep update` is the only way to advance them.
- **Install strategy** (`installer.rs`): if the repo has an `addons/` directory, its contents are copied directly into `project/addons/` (e.g. `repo/addons/gut/` → `project/addons/gut/`). If `subdirectory` is set in the manifest, that subtree goes to `project/addons/<name>/`. If neither applies, the repo root goes to `project/addons/<name>/`.
- **Tag resolution** (`fetcher.rs`): `peel_to_commit` is used so annotated tags resolve to their underlying commit SHA, ensuring lockfile stability even if a tag is force-pushed.
- **Error propagation**: `GdepError` (`error.rs`) is used within library code via `thiserror`; `anyhow` is used only in `main.rs` for top-level propagation.
