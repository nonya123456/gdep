# gdep — Godot Dependency Manager

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
gdep add https://github.com/ramokz/phantom-camera --tag v0.8 --subdir addons/phantom_camera
gdep add https://github.com/nathanhoad/godot_dialogue_manager --commit a3f9c12 --subdir addons/dialogue_manager
gdep add https://github.com/bitwes/Gut --branch main --subdir addons/gut
gdep install
```

Commit `gdep.toml` and `gdep.lock`. Add `addons/` to `.gitignore`.

## Manifest — `gdep.toml`

```toml
[addons.phantom-camera]
git = "https://github.com/ramokz/phantom-camera"
tag = "v0.8"
subdirectory = "addons/phantom_camera"

[addons.dialogue-manager]
git = "https://github.com/nathanhoad/godot_dialogue_manager"
commit = "a3f9c12"
subdirectory = "addons/dialogue_manager"

[addons.gut]
git = "https://github.com/bitwes/Gut"
branch = "main"
subdirectory = "addons/gut"
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
4. The pinned commit is checked out and copied into `project/addons/<name>/`. If the addon lives in a subdirectory of the repo, only that subtree is copied.

## Crates used

| Crate | Purpose |
|---|---|
| `clap` (derive) | CLI subcommands |
| `serde` + `toml` | Manifest and lockfile parsing |
| `git2` | Clone, fetch, checkout (no shelling out) |
| `dirs` | Cross-platform `~/.cache/gdep` |
| `indicatif` | Progress bars during fetch |
| `thiserror` / `anyhow` | Typed + propagated errors |
