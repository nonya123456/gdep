# gdep

Godot addon dependency manager. Commands: `gdep install`, `gdep clean`.

## Design rules

- Commands: `install` (manage addons) and `clean` (wipe cache). No add, remove, update, list.
- `subdir` is required in every addon entry; error if missing or empty.
- Exactly one of `tag`, `branch`, or `commit` is required per addon; value must be non-empty.
- `url` must be non-empty.
- Contents of `subdir` are copied into `addons/<table-name>/` in the Godot project.
- `gdep.toml` is read from the current working directory only.
- Bare clones are cached at `~/.cache/gdep/`.
- `gdep.lock` pins resolved SHAs; if an entry exists, use that SHA (no re-resolve).
- `gdep clean` deletes `~/.cache/gdep/` entirely; next install re-clones from scratch.
- Output is minimal plain text — one line per addon.

## TOML format

```toml
[addons.phantom-camera]
url = "https://github.com/ramokz/phantom-camera"
tag = "v0.8"
subdir = "addons/phantom-camera"
```

## Dev

```bash
cargo build
cargo fmt
cargo clippy -- -D warnings
```
