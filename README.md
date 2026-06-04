# gdep

A minimal addon manager for Godot projects. Write a `gdep.toml`, run `gdep install`.

## Install

```bash
cargo install --git https://codeberg.org/desertmouse/gdep --tag v0.2.2
```

Or from source:

```bash
cargo install --path .
```

## Usage

Create `gdep.toml` in your Godot project root:

```toml
[addons.gut]
url = "https://github.com/bitwes/Gut"
tag = "v9.4.0"
subdir = "addons/gut"

[addons.phantom-camera]
url = "https://github.com/ramokz/phantom-camera"
tag = "v0.8"
subdir = "addons/phantom_camera"

[addons.dialogue-manager]
url = "https://github.com/nathanhoad/godot_dialogue_manager"
commit = "b8e5ec9"
subdir = "addons/dialogue_manager"

[addons.netfox]
url = "https://github.com/foxssake/netfox"
branch = "main"
subdir = "addons/netfox"
```

Then run:

```bash
gdep install
```

To wipe the local clone cache (forces a fresh clone on next install):

```bash
gdep clean
```

Each `[addons.<name>]` entry copies the contents of `subdir` from the source repo into `addons/<name>/` in your project. `subdir` is always required.

Commit `gdep.toml` and `gdep.lock`. Add this to `.gitignore` to exclude managed addons:

```gitignore
addons/*
!addons/my-own-addon/
```

## How it works

1. Reads `gdep.toml` from the current directory.
2. Bare-clones repos into `~/.cache/gdep/` and resolves tags/branches/commits to exact SHAs.
3. Pins SHAs to `gdep.lock` — subsequent installs use the locked SHA for reproducibility.
4. Copies `subdir` contents into `addons/<name>/`. Skips if already at the correct commit.
