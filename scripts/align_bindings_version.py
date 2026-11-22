#!/usr/bin/env python3
"""
Sets the version in all bindings/*/package.json to match workspace version in Cargo.toml.
"""
import json
import re
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent

def get_workspace_version() -> str:
    cargo_toml = (ROOT / "Cargo.toml").read_text(encoding="utf-8")
    match = re.search(r'\[workspace\.package\][\s\S]*?version\s*=\s*"([^"]+)"', cargo_toml)
    if not match:
        raise ValueError("Could not find [workspace.package] version in Cargo.toml")
    return match.group(1)

def main():
    version = get_workspace_version()
    print(f"Aligning bindings packages to workspace version {version}...")
    bindings_dir = ROOT / "bindings"
    for pkg_json in bindings_dir.glob("*/package.json"):
        data = json.loads(pkg_json.read_text(encoding="utf-8"))
        old_ver = data.get("version")
        data["version"] = version
        pkg_json.write_text(json.dumps(data, indent=2) + "\n", encoding="utf-8")
        print(f"  {pkg_json.parent.name}: {old_ver} -> {version}")
    print("Done!")

if __name__ == "__main__":
    main()
