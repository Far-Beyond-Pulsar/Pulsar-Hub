# Pulsar Hub

**The official launcher and project manager for the [Pulsar Game Engine](https://github.com/Far-Beyond-Pulsar/Pulsar-Native).**

A modern, cross-platform hub — in the spirit of Unity Hub or the Unreal Project Browser — built with Rust and GPUI. It manages your projects, installs and updates engine versions, handles git for you, and launches the editor.

## WIP, Contribs welcome as platform support for the engine improves

<img width="1300" height="800" alt="image" src="https://github.com/user-attachments/assets/14231233-16c8-4bf3-8586-afabb0592eee" />
<img width="1300" height="800" alt="image" src="https://github.com/user-attachments/assets/85458d9a-0465-48ee-8604-87785e1a3f3d" />
<img width="1300" height="800" alt="image" src="https://github.com/user-attachments/assets/8bc63625-0f97-4e74-8cd4-306aba1878e9" />
<img width="1300" height="800" alt="image" src="https://github.com/user-attachments/assets/f3cd8a4d-d01a-4297-a147-d1597e1e9fa0" />
<img width="1300" height="800" alt="image" src="https://github.com/user-attachments/assets/ed6e8a1e-d825-4481-bbee-1661fdb31ea5" />
<img width="1300" height="800" alt="image" src="https://github.com/user-attachments/assets/d76d207f-c936-4dda-9985-d49f0f9ffdcb" />
<img width="1300" height="800" alt="image" src="https://github.com/user-attachments/assets/c5f779e3-4d80-45f1-ae22-10941f32f62a" />
<img width="1300" height="800" alt="image" src="https://github.com/user-attachments/assets/1644acca-cb28-4157-bf3f-80d8d4085646" />

---

## Why the Hub exists

The Pulsar editor no longer embeds a project browser or launcher. **The engine binary will not open on its own** — launched without a project it prints:

> `No project specified. Please launch Pulsar Engine via the Pulsar Hub launcher or specify a project path.`

…and exits. The Hub is now the primary (and recommended) way to get into the editor. The only alternatives are the manual `pulsar://` URI commands and raw CLI arguments documented [below](#launching-the-engine-without-the-hub), which are supported for scripting, deep links, and power users.

## What the Hub does

### Projects
- **Recent Projects** grid with thumbnails, live search/filter, and right-click actions.
- **New Project** wizard that scaffolds the directory layout, writes `Pulsar.toml`, and `git init`s the repo.
- **Templates** gallery — clone a starter project straight from GitHub.
- **Clone from Git** — clone any repository and register it as a project, with an upstream-remote prompt for forks.

### Engine version management
- Browse official releases by channel (**Stable / Nightly**, multiple source repos), read release notes, install/uninstall any version side-by-side.
- Installs land in `%LOCALAPPDATA%\Programs\Pulsar\<version>` on Windows, `/Applications/Pulsar` on macOS, `~/.local/share/pulsar/<version>` on Linux.
- **Source builds**: point the Hub at a local checkout of Pulsar-Native (the special `src` version) and it will run `cargo build --release` with a live progress overlay, then launch the result.
- Per-project auto-install: opening a project pinned to an engine you don't have prompts to fetch it automatically.

### Launching projects
Clicking a card resolves the project's `Pulsar.toml` → `[project] engine_version`, picks the best installed engine (or triggers the auto-install prompt, or builds `src`), then spawns the editor detached with the project. The Hub stays open as your control center.

### Git integration (built in)
- Background auto-fetch on a configurable interval; cards show **behind-count badges** with one-click pull.
- Full **Git Manager window** (stage/commit/diff/branches/history) per project, plus git settings (identity, credentials).
- Storage page: per-project disk usage split into working files vs `.git` history, with repo-health indicators.

### Settings
Settings use the engine's TOML settings database:
- Global editor settings live under your OS config dir (e.g. `%APPDATA%\PulsarEngine\editor\*.toml`).
- Per-project settings live inside the project at `.pulsar/project/*.toml`.
The sidebar **Settings** entry edits globals; the gear icon on a project card opens the settings window *scoped to that project*.

### Also included
Cloud Projects (self-hosted workspaces), Friends/presence, GitHub sign-in via device flow, 21 themes, first-run onboarding, a download manager, and self-updating of the Hub itself (`--updated` relaunch flow).

### Rollback
If an update goes wrong, the previous binary is kept next to the updated one as `Pulsar.bak` (removed automatically once the new version launches cleanly). To recover manually, rename `Pulsar.bak` over `Pulsar.exe`.

---

## Launching the engine without the Hub

For scripts, CI, file managers, or debugging — three manual routes exist. In all cases the target directory must exist and contain a `Pulsar.toml`, otherwise the engine refuses to start.

### 1. `pulsar://` URI scheme

The engine registers itself as the handler for the OS-level `pulsar://` protocol on first run (Windows: `HKCU` — no admin required; macOS: app bundle in `~/Applications`; Linux: `~/.local/share/applications`). Anywhere URIs work — Run dialog, browser address bar, scripts — you can use:

```
pulsar://open_project/<url-encoded-path>
```

This is currently the only implemented command; more are planned (`open_file`, `create_project`, …).

Example (Windows path):

```
pulsar://open_project/C%3A%5CUsers%5Cyou%5CProjects%5Cmy_game
```

From PowerShell:

```powershell
Start-Process "pulsar://open_project/$([uri]::EscapeDataString('C:\Users\you\Projects\my_game'))"
```

Or invoke the binary directly with the URI as its argument:

```powershell
& "$env:LOCALAPPDATA\Programs\Pulsar\v0.1.23\pulsar.exe" "pulsar://open_project/C%3A%5CUsers%5Cyou%5CProjects%5Cmy_game"
```

Notes:
- The path segment must be URL-encoded (this is what lets Windows-style paths survive). `/`, `\`, `:` and spaces all need encoding.
- The Hub itself launches the engine exactly this way — it spawns `pulsar.exe pulsar://open_project/<encoded>` detached, so behavior is identical.

### 2. Positional CLI argument

A plain filesystem path also works and skips URI parsing:

```powershell
pulsar.exe C:\Users\you\Projects\my_game
```

The first non-flag argument is treated as the project path. It must exist on disk (no `Pulsar.toml` re-validation happens on this route beyond normal startup).

### 3. Everything else fails fast

No arguments, no valid URI, or a missing project ⇒ warning + clean exit. There is deliberately no "empty editor" mode; the project model starts at the Hub (or an explicit command above).

---

## Project anatomy

A minimal `Pulsar.toml` (what the New Project wizard generates):

```toml
[project]
name = "my_awesome_game"
version = "0.1.0"
engine_version = "0.1.23"

[settings]
default_scene = "scenes/main.scene"
```

- `engine_version` semantics: the literal `src` always uses your local source build; a `nightly-…` value must match an installed nightly exactly; any plain `x.y.z` is treated as a **minimum** — the Hub launches the newest installed engine that satisfies it, and offers to auto-install one if nothing qualifies.
- Standard folder layout scaffolded alongside it: `assets/`, `scenes/`, `scripts/`, `prefabs/`.

## Keyboard shortcuts (Hub)

| Shortcut | Action |
|---|---|
| `Ctrl+N` | New project modal |
| `Ctrl+O` | Add existing project from folder |
| `Ctrl+G` | Clone from Git modal |
| `Ctrl+Shift+R` | Check all projects for git updates |
| `Ctrl+,` | Open global settings |

## Building from source

Requires a recent stable Rust toolchain.

```sh
git clone https://github.com/Far-Beyond-Pulsar/Pulsar-Hub
cd Pulsar-Hub
cargo build --release -p pulsar-installer
```

The Hub depends on Pulsar-Native crates (`ui`, `engine_state`, `ui_git_manager`, …) pinned via git revisions in the root [`Cargo.toml`](Cargo.toml), so the first build fetches those. Run it with `cargo run -p pulsar-installer`.

To develop against local checkouts of the engine instead of the pins, redirect the dependencies with `[patch]` sections pointing at your `Pulsar-Native` working tree.

## Repository layout

```
crates/
  installer/   thin release binary: updater + boots the Hub UI
  hub/         the Hub application (screens, services, windows)
  patch-tool/  helper utilities
themes/        bundled color themes
```
