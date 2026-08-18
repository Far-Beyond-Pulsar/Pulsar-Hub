# Pulsar Hub Rearchitecture Plan

## Goal
Replace the current wizard-style installer with a modern application hub (like Unity Hub / Unreal Project Browser) that combines project management, version management, and installer functionality in a single unified UI. The hub will be visually and architecturally consistent with the `ui_entry` crate from Pulsar-Native.

## Strategy
Copy `ui_entry` source files into Pulsar-Installer as a new `crates/hub` crate. Reference Pulsar-Native crates via **local path dependencies** (not stubs, not git refs). Use `[patch]` sections to redirect transitive git deps to Pulsar-Native's local vendored copies, ensuring a single resolved copy of each crate.

**Key architectural decision:** The hub uses Pulsar-Native's `ui` crate (wgpui-component) directly — NOT the local `gpui-component` fork. This eliminates the tree-sitter version conflict (gpui-component uses 0.25.4, wgpui-component uses 0.24.x) since only one UI component library is in the dependency graph.

---

## Status: Phase 6 COMPLETE

### Completed:
1. ✅ **Phase 1-2**: Created `crates/hub/` with all ui_entry source files copied
2. ✅ **Phase 3**: Updated installer binary to launch hub instead of its own UI
3. ✅ **Phase 4**: Resolved tree-sitter conflict, both `pulsar-hub` and `pulsar-installer` compile
4. ✅ **Phase 5**: Added Version Management Page with service layer + UI
5. ✅ **Phase 6**: Warnings suppressed, navigation wiring verified, full build succeeds

---

## What We Built

### Architecture Changes
- **Removed** `crates/ui` (gpui-component), `crates/macros`, `crates/pulsar_macros`, `crates/assets` from workspace members — they conflict with Pulsar-Native's `ui`
- **Added** `crates/hub` as a workspace member using Pulsar-Native's `ui` (wgpui-component) via path deps
- **Added** `[patch]` sections to redirect all transitive git deps to Pulsar-Native's local paths
- **Added** `[workspace.lints.rust]` to suppress upstream crate warnings
- **Updated** `crates/installer/src/main.rs` to launch `pulsar_hub::EntryWindow` wrapped in `ui::Root`
- **Removed** `crates/installer/src/ui/` module (old wizard UI code, no longer compiled)

### Workspace Dependencies
All Pulsar-Native crates are referenced via local path dependencies:
```
PULSAR_NATIVE = "C:/Users/redst/Documents/GitHub/Pulsar-Native"
ui, ui-macros, ui_entry, pulsar_auth, engine_state, engine_fs,
window_manager, friends_engine, ui_common, ui_auth, ui_git_manager,
ui_friends, ui_types_common, ui_gen_macros, pulsar-config,
pulsar_settings, engine_backend, pulsar-multiplayer-core,
pulsar_reflection, pulsar_reflection_derive
```

### Patch Sections (in workspace Cargo.toml)
Redirect transitive git dependencies to Pulsar-Native's local vendored copies:
- `[patch."https://github.com/Far-Beyond-Pulsar/WGPUI"]` → local wgpui
- `[patch."https://github.com/Far-Beyond-Pulsar/WGPUI-Component"]` → local wgpui-component
- `[patch."https://github.com/Far-Beyond-Pulsar/Pulsar-Native"]` → all local paths
- `[patch."https://github.com/Far-Beyond-Pulsar/Pulsar-Reflection"]` → pinned git rev
- `[patch."https://github.com/Far-Beyond-Pulsar/PulsarConfig"]` → local pulsar-config

### Version Management (Phase 5)
- `service/installer_service.rs` — version scanning, GitHub release fetching, download/extract, launch, remove, platform helpers
- `screen/views/versions.rs` — full UI page with installed versions list, available releases, install/launch/remove actions
- `Versions` sidebar item under "ENGINE" section
- Background-threaded download with progress, error handling

### Navigation (Phase 6 verified)
All 7 `EntryScreenView` variants are fully wired:

| Variant | Layout | Sidebar | Module | State |
|---------|--------|---------|--------|-------|
| Recent | ✅ | ✅ | ✅ | ✅ |
| Templates | ✅ | ✅ | ✅ | ✅ |
| NewProject | ✅ | ✅ | ✅ | ✅ |
| CloneGit | ✅ | ✅ | ✅ | ✅ |
| Versions | ✅ | ✅ | ✅ | ✅ |
| CloudProjects | ✅ | ✅ | ✅ | ✅ |
| Friends | ✅ | ✅ | (external) | ✅ |

Additional conditional overlays (onboarding, dependency_setup, upstream_prompt, project_settings) are all wired.

---

## File Structure

```
crates/
  hub/                    NEW - combined hub + installer UI
    Cargo.toml            (depends on ui/wgpui-component, not gpui-component)
    src/
      lib.rs              (entry point, exports EntryWindow/EntryScreen)
      window.rs           (EntryWindow - wraps screen in Root)
      component/          (card_grid, gh_device_modal, modal, plugin_card, progress_bar, status_item)
      core/               (events, state, types)
      screen/
        mod.rs            (EntryScreen + version management methods)
        layout.rs         (sidebar + content layout, all 7 variant dispatch)
        views/            (all screen views from ui_entry + versions.rs)
      service/            (auth, cloud, dependency, git, integration, installer, plugin, project, thumbnail)
      util/               (formatters, path_helpers)
  installer/              MODIFIED - thin binary wrapper
    Cargo.toml            (depends on pulsar-hub, ui, not gpui-component)
    src/
      main.rs             (launches hub's EntryWindow)
      lib.rs              (keeps installer logic as library)
```

---

## Success Criteria
- [x] Hub compiles without errors on Windows
- [x] Installer binary compiles without errors (147MB debug build)
- [x] Zero warnings from pulsar-hub crate (upstream warnings suppressed via workspace lints)
- [x] All 7 navigation screens fully wired (layout + sidebar + modules + state)
- [ ] Hub launches and shows the sidebar + content layout matching ui_entry's visual style
- [ ] Can download and install a new engine version from the hub
- [ ] Can launch installed engine versions
- [ ] Can remove installed versions
- [ ] Dark theme works consistently across all pages
