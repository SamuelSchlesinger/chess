#!/usr/bin/env python3
"""Check navigation, local links, and citation hygiene for this branch."""

from __future__ import annotations

import re
from collections import defaultdict, deque
from pathlib import Path
from urllib.parse import unquote


ROOT = Path(__file__).resolve().parent.parent
INDEX = ROOT / "index.md"
INLINE_LINK = re.compile(r"\[[^\]]+\]\(([^)]+)\)")
REFERENCE_DEF = re.compile(r"^\[([a-zA-Z0-9._-]+)\]:\s+(\S+)", re.MULTILINE)
INLINE_CITATION = re.compile(r"\[([a-zA-Z0-9._-]+)\]\[\1\]")


def local_target(source: Path, raw: str) -> Path | None:
    raw = raw.strip().strip("<>")
    if raw.startswith(("http://", "https://", "mailto:", "#")):
        return None
    path = unquote(raw.split("#", 1)[0])
    return (source.parent / path).resolve()


def main() -> None:
    markdown = sorted(ROOT.rglob("*.md"))
    failures: list[str] = []
    graph: dict[Path, set[Path]] = defaultdict(set)
    citations = 0
    checked_links = 0

    for path in markdown:
        text = path.read_text(encoding="utf-8")
        definitions = dict(REFERENCE_DEF.findall(text))
        cited = set(INLINE_CITATION.findall(text))
        citations += len(INLINE_CITATION.findall(text))

        if cited and "## Local References" not in text:
            failures.append(f"{path.relative_to(ROOT)}: missing Local References")
        for key in sorted(cited):
            if key not in definitions:
                failures.append(f"{path.relative_to(ROOT)}: undefined citation {key}")
            if not re.search(rf"^- \*\*{re.escape(key)}\*\*\s+—", text, re.MULTILINE):
                failures.append(
                    f"{path.relative_to(ROOT)}: no full Local References entry for {key}"
                )

        targets = list(INLINE_LINK.findall(text)) + list(definitions.values())
        for raw in targets:
            target = local_target(path, raw)
            if target is None:
                continue
            checked_links += 1
            if not target.exists():
                failures.append(
                    f"{path.relative_to(ROOT)}: missing local target {raw}"
                )
                continue
            if target.suffix == ".md" and target.is_relative_to(ROOT):
                graph[path].add(target)

    reached: set[Path] = set()
    queue = deque([INDEX])
    while queue:
        current = queue.popleft()
        if current in reached:
            continue
        reached.add(current)
        queue.extend(graph[current] - reached)
    for path in markdown:
        if path not in reached:
            failures.append(f"{path.relative_to(ROOT)}: not reachable from index.md")

    if failures:
        raise SystemExit("\n".join(failures))
    print(f"markdown documents reachable: {len(markdown)}")
    print(f"local link targets checked: {checked_links}")
    print(f"inline citations checked: {citations}")


if __name__ == "__main__":
    main()
