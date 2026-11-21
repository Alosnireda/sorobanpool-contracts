#!/usr/bin/env python3
"""
Checks that every public function in contracts/group_buy/src/lib.rs and
contracts/disputes/src/lib.rs appears in docs/state-machine.md.
"""
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
DOC = ROOT / "docs" / "state-machine.md"
GB_SRC = ROOT / "contracts" / "group_buy" / "src" / "lib.rs"
DIS_SRC = ROOT / "contracts" / "disputes" / "src" / "lib.rs"

def extract_pub_fns(src_file: Path, impl_name: str):
    text = src_file.read_text(encoding="utf-8")
    match = re.search(rf"impl {impl_name} \{{([\s\S]*?)\n\}}", text)
    if not match:
        raise ValueError(f"Could not find `impl {impl_name}` in {src_file}")
    body = match.group(1)
    return set(re.findall(r"pub fn ([a-z0-9_]+)\(", body))

def main():
    if not DOC.exists():
        print(f"Error: {DOC} does not exist", file=sys.stderr)
        sys.exit(1)

    doc_text = DOC.read_text(encoding="utf-8")

    gb_fns = extract_pub_fns(GB_SRC, "GroupBuy")
    dis_fns = extract_pub_fns(DIS_SRC, "Disputes")

    missing = []
    for fn in sorted(gb_fns):
        pattern = rf"\[`{re.escape(fn)}`\]"
        if not re.search(pattern, doc_text):
            missing.append(f"group_buy::{fn}")

    for fn in sorted(dis_fns):
        pattern = rf"\[`{re.escape(fn)}`\]"
        if not re.search(pattern, doc_text):
            missing.append(f"disputes::{fn}")

    if missing:
        print("Missing functions from docs/state-machine.md:", file=sys.stderr)
        for m in missing:
            print(f"  - {m}", file=sys.stderr)
        sys.exit(1)

    print(f"OK: All {len(gb_fns)} group_buy functions and {len(dis_fns)} disputes functions appear in {DOC.name}")

if __name__ == "__main__":
    main()
