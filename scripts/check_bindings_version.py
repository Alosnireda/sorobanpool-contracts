#!/usr/bin/env python3
"""
Checks that all bindings/*/package.json use the exact workspace version from Cargo.toml
and share consistent SDK dependencies.
"""
import json
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent

def get_workspace_version() -> str:
    cargo_toml = (ROOT / "Cargo.toml").read_text(encoding="utf-8")
    match = re.search(r'\[workspace\.package\][\s\S]*?version\s*=\s*"([^"]+)"', cargo_toml)
    if not match:
        raise ValueError("Could not find [workspace.package] version in Cargo.toml")
    return match.group(1)

def main():
    expected_version = get_workspace_version()
    bindings_dir = ROOT / "bindings"
    pkg_files = sorted(bindings_dir.glob("*/package.json"))
    if not pkg_files:
        print("Error: No bindings packages found", file=sys.stderr)
        sys.exit(1)

    errors = []
    sdk_versions = {}

    for pkg_json in pkg_files:
        pkg_name = pkg_json.parent.name
        try:
            data = json.loads(pkg_json.read_text(encoding="utf-8"))
        except Exception as e:
            errors.append(f"{pkg_name}: invalid JSON ({e})")
            continue

        ver = data.get("version")
        if ver != expected_version:
            errors.append(f"{pkg_name}: version '{ver}' does not match workspace '{expected_version}'")

        # Check @stellar/stellar-sdk dependency consistency
        deps = data.get("dependencies", {})
        sdk_dep = deps.get("@stellar/stellar-sdk")
        if sdk_dep:
            sdk_versions[pkg_name] = sdk_dep

    # Verify all bindings have identical SDK dependency version
    unique_sdk_versions = set(sdk_versions.values())
    if len(unique_sdk_versions) > 1:
        errors.append(f"Mismatched @stellar/stellar-sdk dependencies across packages: {sdk_versions}")

    if errors:
        print("Bindings version check failed:", file=sys.stderr)
        for err in errors:
            print(f"  - {err}", file=sys.stderr)
        sys.exit(1)

    print(f"OK: All {len(pkg_files)} bindings packages use consistent workspace version {expected_version}")

if __name__ == "__main__":
    main()
