#!/usr/bin/env python3
"""Assert the two copies of the pnpm overrides agree.

They exist twice on purpose: pnpm 10+ reads pnpm-workspace.yaml, pnpm 9 reads
package.json, and whichever the builder reads must match the `overrides` block
recorded in pnpm-lock.yaml or --frozen-lockfile fails. Two hand-maintained
lists drift, so this fails the build the moment they do.
"""

import json
import pathlib
import re
import sys

WEB = pathlib.Path(__file__).resolve().parent.parent / "apps" / "web"


def from_yaml(path: pathlib.Path) -> dict[str, str]:
    """Parse the flat `overrides:` block without taking a PyYAML dependency."""
    out, in_block = {}, False
    for line in path.read_text().splitlines():
        if re.match(r"^overrides:\s*$", line):
            in_block = True
            continue
        if in_block:
            if line and not line.startswith((" ", "\t")):
                break
            m = re.match(r"^\s+(\S+):\s*'?([^'#]+?)'?\s*$", line)
            if m:
                out[m.group(1)] = m.group(2)
    return out


def main() -> int:
    yaml_ov = from_yaml(WEB / "pnpm-workspace.yaml")
    json_ov = json.loads((WEB / "package.json").read_text()).get("pnpm", {}).get("overrides", {})

    if not yaml_ov:
        print("FAIL: no overrides parsed from pnpm-workspace.yaml", file=sys.stderr)
        return 1
    if yaml_ov == json_ov:
        print(f"pnpm overrides agree ({len(yaml_ov)} entries)")
        return 0

    print("FAIL: pnpm overrides disagree between the two files.", file=sys.stderr)
    for k in sorted(set(yaml_ov) | set(json_ov)):
        a, b = yaml_ov.get(k), json_ov.get(k)
        if a != b:
            print(f"  {k}: pnpm-workspace.yaml={a!r}  package.json={b!r}", file=sys.stderr)
    return 1


if __name__ == "__main__":
    sys.exit(main())
