# gdep

Godot addon dependency manager. Single command: `gdep install`.

## Design rules

- `gdep install` is the only command — no add, remove, update, list.
- `subdir` is required in every addon entry; error if missing.
- Exactly one of `tag`, `branch`, or `commit` is required per addon.
- Contents of `subdir` are copied into `addons/<table-name>/` in the Godot project.
- `gdep.toml` is read from the current working directory only.
- Bare clones are cached at `~/.cache/gdep/`.
- `gdep.lock` pins resolved SHAs; if an entry exists, use that SHA (no re-resolve).
- Skip install if `addons/<name>/.gdep` already contains the target SHA.
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
