# gdep — Godot Dependency Manager

> Vibe coded with [Claude Code](https://claude.ai/code).

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
gdep add https://github.com/nathanhoad/godot_dialogue_manager --commit a3f9c12
gdep add https://github.com/bitwes/Gut --branch main
gdep install
```

Commit `gdep.toml` and `gdep.lock`. Add `addons/` to `.gitignore`.

## Manifest — `gdep.toml`

```toml
[addons.phantom-camera]
git = "https://github.com/ramokz/phantom-camera"
tag = "v0.8"

[addons.dialogue-manager]
git = "https://github.com/nathanhoad/godot_dialogue_manager"
commit = "a3f9c12"

[addons.gut]
git = "https://github.com/bitwes/Gut"
branch = "main"
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
4. Files are copied into `project/addons/` using the following strategy (same as gd-plug):
   - If the repo contains an `addons/` directory, its contents are copied directly into `project/addons/` — so `repo/addons/gut/` lands at `project/addons/gut/`. No `subdirectory` needed for standard Godot addon repos.
   - If `subdirectory` is set, that specific path is copied to `project/addons/<name>/`.
   - If neither applies, the repo root is copied to `project/addons/<name>/`.

## Crates used

| Crate | Purpose |
|---|---|
| `clap` (derive) | CLI subcommands |
| `serde` + `toml` | Manifest and lockfile parsing |
| `git2` | Clone, fetch, checkout (no shelling out) |
| `dirs` | Cross-platform `~/.cache/gdep` |
| `indicatif` | Progress bars during fetch |
| `thiserror` / `anyhow` | Typed + propagated errors |
