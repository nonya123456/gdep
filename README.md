# gdep — Godot Dependency Manager

> ⚠️ Vibe coded. Use at your own risk.

A Cargo/go-mod-style addon manager for Godot projects.  
Fetches addons directly from git repos so you never commit third-party addons to your game repo.

## Install

```
cargo install --path .
```

## Quick start

```bash
# In your Godot project root:
gdep init
gdep add https://github.com/ramokz/phantom-camera --tag v0.8
gdep add https://github.com/nathanhoad/godot_dialogue_manager --commit b8e5ec9
gdep add https://github.com/bitwes/Gut --branch main
gdep install
```

Commit `gdep.toml` and `gdep.lock`. See [.gitignore](#gitignore) below for how to exclude managed addons.

## .gitignore

Add this to your Godot project's `.gitignore` to exclude all gdep-managed addons while committing `gdep.toml` and `gdep.lock`:

```gitignore
# Ignore all managed addons — gdep restores them from gdep.lock
addons/*
```

If you maintain one of your own addons directly in the repo, use a `!` exception. Note that `addons/*` must be used instead of `addons/` — git won't descend into a fully-ignored directory, so exceptions inside it silently have no effect:

```gitignore
# Ignore all managed addons
addons/*

# Keep your own in-repo addon tracked
!addons/my-own-addon/
```

## Manifest — `gdep.toml`

```toml
[addons.phantom-camera]
git = "https://github.com/ramokz/phantom-camera"
tag = "v0.8"

[addons.dialogue-manager]
git = "https://github.com/nathanhoad/godot_dialogue_manager"
commit = "b8e5ec9"

[addons.gut]
git = "https://github.com/bitwes/Gut"
branch = "main"
```

### Pulling multiple subdirectories from one repo

Some repos (e.g. [netfox](https://github.com/foxssake/netfox)) ship several independent addons under a single `addons/` tree. Use `subdirectory` with multiple entries pointing at the same `git` URL — gdep bare-clones the repo once and reuses the cache for each entry:

```toml
[addons.netfox]
git = "https://github.com/foxssake/netfox"
tag = "v1.35.3"
subdirectory = "addons/netfox"

[addons.netfox-extras]
git = "https://github.com/foxssake/netfox"
tag = "v1.35.3"
subdirectory = "addons/netfox.extras"

[addons.netfox-noray]
git = "https://github.com/foxssake/netfox"
tag = "v1.35.3"
subdirectory = "addons/netfox.noray"
```

Or via the CLI (the `--name` flag is required since all three share the same URL):

```bash
gdep add https://github.com/foxssake/netfox --name netfox       --tag v1.35.3 --subdir addons/netfox
gdep add https://github.com/foxssake/netfox --name netfox-extras --tag v1.35.3 --subdir addons/netfox.extras
gdep add https://github.com/foxssake/netfox --name netfox-noray  --tag v1.35.3 --subdir addons/netfox.noray
```

## Commands

| Command | Description |
|---|---|
| `gdep init` | Create `gdep.toml` in current directory |
| `gdep add <url> [--tag\|--branch\|--commit] [--subdir <path>] [--name <name>]` | Add an addon |
| `gdep install` | Install all addons from lockfile (resolves first if needed) |
| `gdep update [name]` | Re-fetch latest for branch-pinned addons |
| `gdep remove <name>` | Remove from manifest and delete from `addons/` |
| `gdep list` | Show installed addons and their pinned commits |

## How it works

1. `gdep.toml` lists addons with their git source and a ref (tag, branch, or commit).
2. On `gdep install`, tags and branches are resolved to concrete commit SHAs and written to `gdep.lock`.
3. Repos are cached as bare clones under `~/.cache/gdep/` — reinstalls are fully offline if the cache is warm.
4. Files are copied into `project/addons/` using the following strategy:
   - If the repo contains an `addons/` directory, its contents are copied directly into `project/addons/` — so `repo/addons/gut/` lands at `project/addons/gut/`. No `subdirectory` needed for standard Godot addon repos.
   - If `subdirectory` is set, that specific path is copied to `project/addons/<name>/`.
   - If neither applies, the repo root is copied to `project/addons/<name>/`.

## Development

```bash
cargo build          # debug build
cargo test           # run all 23 tests (no network required)
cargo clippy         # lint — must be clean
cargo fmt            # format before committing
```

Tests use `git2` to build synthetic repos in temporary directories — no network, no fixture files. The bare-clone cache is redirected to a `TempDir` per test via the `GDEP_CACHE_DIR` env var, so `~/.cache/gdep/` is never touched during testing.

## Crates used

| Crate | Purpose |
|---|---|
| `clap` (derive) | CLI subcommands |
| `serde` + `toml` | Manifest and lockfile parsing |
| `git2` | Clone, fetch, checkout (no shelling out) |
| `dirs` | Cross-platform `~/.cache/gdep` |
| `indicatif` | Progress bars during fetch |
| `thiserror` / `anyhow` | Typed + propagated errors |
| `tempfile` (dev) | Temporary directories in tests |
