#!/usr/bin/env python3
r"""Check that a repository plan answers every checklist item and carries a complete handoff.

Usage: check-plan.py PLAN.md

Exit 0 when the plan is complete in shape, 1 when it is not (each problem is printed), and 2 when
the file cannot be read, has no decision table at all, or the wrong number of arguments is given.
A pass says nothing about whether a decision is right; it says no item was skipped.

A literal `|` inside a cell must be escaped as `\|`, as in any Markdown table; a row with more
cells than the header fails rather than being read out of alignment.
"""

from __future__ import annotations

import re
import sys
from pathlib import Path

ITEMS = (
    "rulesets",
    "secret-scanning",
    "dependency-scanning",
    "sast",
    "dast",
    "docs-scaffold",
    "agent-host-hooks",
    "local-hooks",
    "server-checks",
    "test-layout",
    "frontmatter",
    "provenance",
)
OUTCOMES = ("decided", "deferred", "not-needed")
HANDOFF_KEYS = ("target", "owner", "repository", "project-name", "lifecycle")
LIFECYCLE = ("yes", "no", "owner-decides")

_HEADING = re.compile(r"^(#{1,6})\s+(.*?)\s*$")
_PLACEHOLDER = re.compile(r"<(?!https?://)[^<>]+>")
_CODE_SPAN = re.compile(r"`[^`]*`")


def _has_placeholder(text: str) -> bool:
    """A template `<...>` left anywhere in the text, ignoring code spans and autolinks."""
    return bool(_PLACEHOLDER.search(_CODE_SPAN.sub("", text)))


def _section(lines: list[str], title: str) -> list[str] | None:
    """Lines under the first heading named `title`, up to the next heading of the same or higher level."""
    for index, line in enumerate(lines):
        match = _HEADING.match(line)
        if match and match.group(2).strip().lower() == title.lower():
            level = len(match.group(1))
            body = []
            for following in lines[index + 1:]:
                nested = _HEADING.match(following)
                if nested and len(nested.group(1)) <= level:
                    break
                body.append(following)
            return body
    return None


def _cells(row: str) -> list[str]:
    parts = re.split(r"(?<!\\)\|", row.strip())
    if parts and parts[0] == "":
        parts = parts[1:]
    if parts and parts[-1] == "":
        parts = parts[:-1]
    return [part.strip() for part in parts]


def _table(body: list[str]) -> list[list[str]]:
    rows = []
    for line in body:
        if line.lstrip().startswith("|"):
            rows.append(_cells(line))
        elif rows:
            break
    return rows


def check_decisions(lines: list[str]) -> tuple[list[str], bool]:
    """Problems with the decision table, and whether a table was found at all."""
    body = _section(lines, "Decisions")
    if body is None:
        return ["no `## Decisions` section"], False
    rows = _table(body)
    if len(rows) < 2:
        return ["the `## Decisions` section has no table"], False

    header = [cell.lower() for cell in rows[0]]
    columns = {}
    for name in ("item", "outcome", "decision or reason", "revisit when"):
        if name not in header:
            return [f"the decision table has no `{name}` column"], True
        columns[name] = header.index(name)

    problems = []
    seen: dict[str, int] = {}
    for row in rows[1:]:
        if all(set(cell) <= set("-: ") for cell in row):
            continue
        item = row[columns["item"]].strip("` ") if len(row) > columns["item"] else ""
        if len(row) > len(header):
            problems.append(
                f"`{item}`: row has {len(row)} cells and the header {len(header)}; "
                "escape a literal | in a cell as \\|"
            )
            continue
        row = row + [""] * (len(header) - len(row))
        outcome = row[columns["outcome"]].strip("` ").lower()
        reason = row[columns["decision or reason"]]
        revisit = row[columns["revisit when"]]
        if item not in ITEMS:
            problems.append(f"unknown item `{item}`; the items are the twelve in checklist.md")
            continue
        seen[item] = seen.get(item, 0) + 1
        if outcome not in OUTCOMES:
            problems.append(f"`{item}`: outcome `{outcome or '(empty)'}` is not one of {', '.join(OUTCOMES)}")
        if not reason or _PLACEHOLDER.fullmatch(reason):
            problems.append(f"`{item}`: no decision or reason")
        elif _has_placeholder(reason):
            problems.append(f"`{item}`: the decision still contains a template placeholder")
        if outcome == "deferred" and (not revisit or _has_placeholder(revisit)):
            problems.append(f"`{item}`: deferred with no revisit trigger")

    for item in ITEMS:
        count = seen.get(item, 0)
        if count == 0:
            problems.append(f"`{item}`: missing")
        elif count > 1:
            problems.append(f"`{item}`: appears {count} times")
    return problems, True


def check_handoff(lines: list[str]) -> list[str]:
    body = _section(lines, "Scaffold handoff")
    if body is None:
        return ["no `## Scaffold handoff` section"]
    block: list[str] | None = None
    for line in body:
        if line.strip().startswith("```"):
            if block is None:
                block = []
                continue
            break
        if block is not None:
            block.append(line)
    if block is None:
        return ["the handoff section has no fenced block"]

    values = {}
    for line in block:
        key, sep, value = line.partition(":")
        if sep:
            values[key.strip()] = value.strip()
    problems = []
    for key in HANDOFF_KEYS:
        value = values.get(key, "")
        if not value or _has_placeholder(value):
            problems.append(f"handoff: `{key}` is missing or still a placeholder")
    lifecycle = values.get("lifecycle", "")
    if lifecycle and not _has_placeholder(lifecycle) and lifecycle not in LIFECYCLE:
        problems.append(f"handoff: `lifecycle` is `{lifecycle}`, not one of {', '.join(LIFECYCLE)}")
    return problems


def main(argv: list[str]) -> int:
    if len(argv) != 2:
        print(__doc__.strip().splitlines()[2], file=sys.stderr)
        return 2
    try:
        lines = Path(argv[1]).read_text(encoding="utf-8").splitlines()
    except OSError as error:
        print(f"cannot read {argv[1]}: {error}", file=sys.stderr)
        return 2

    problems, has_table = check_decisions(lines)
    problems += check_handoff(lines)
    for problem in problems:
        print(f"FAIL {problem}")
    if not has_table:
        return 2
    if problems:
        print(f"{len(problems)} problem(s)")
        return 1
    print(f"ok: {len(ITEMS)} items answered, handoff complete")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
