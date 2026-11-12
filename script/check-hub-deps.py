#!/usr/bin/env python3
"""Fail if the Hub's dependency graph pulls in the engine's editor/renderer stack.

The Hub installs and launches engines; it must not compile the renderer, the
scene database, the plugin API or any plugin. Reads the feature-resolved graph
from `cargo tree`, so optional dependencies that are not enabled do not count.
"""
import re
import subprocess
import sys

FORBIDDEN_PREFIXES = ("helio-", "helio_", "plugin_editor_api", "plugin_manager", "pulsar_lsp", "pulsar_script", "pulsar_scene", "pulsar_std", "pulsar_world_registry", "engine_backend", "engine_subsystems", "libhelio", "scenedb")
ALLOWED = {"helio-settings"}  # the settings types pulsar_settings describes

out = subprocess.run(
    ["cargo", "tree", "-p", "pulsar-hub", "-e", "normal", "--prefix", "none"],
    capture_output=True, text=True, check=True,
).stdout
names = sorted({m.group(1) for line in out.splitlines() if (m := re.match(r"(\S+) v", line))})
bad = [n for n in names if n.startswith(FORBIDDEN_PREFIXES) and n not in ALLOWED]
if bad:
    print("pulsar-hub depends on engine internals it must not compile:")
    for n in bad:
        print("  ", n)
    print("Reach them through a light crate (ui_shell, pulsar_studio_client, ...) instead.")
    sys.exit(1)
print(f"ok: {len(names)} packages, none of the engine's editor/renderer stack")
