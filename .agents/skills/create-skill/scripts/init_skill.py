#!/usr/bin/env python3
"""Scaffold a skill folder, so the template is not retyped every time.

What this writes is deliberately incomplete. The frontmatter carries a
placeholder description and the body carries the section headings, because a
scaffold that guesses at triggers produces a description nobody rewrites. The
placeholder is designed to fail `validate_skill.py` until a human has replaced
it: an unfinished skill that validates is the failure this avoids.

Usage:
    python3 scripts/init_skill.py <name> --path <skills-root> [--resources scripts,assets]
    python3 scripts/init_skill.py <name> --path <skills-root> --evals

Exits 0 on success, 1 if the name is invalid or the folder already exists.
"""

from __future__ import annotations

import argparse
import re
from pathlib import Path

NAME_RE = re.compile(r"[a-z0-9]+(?:-[a-z0-9]+)*")
MAX_NAME_CHARS = 64
PLACEHOLDER = "TODO"

SKILL_TEMPLATE = """---
name: {name}
description: {placeholder} state the capability in one clause. Use when {placeholder} \
name the concrete requests, artifacts, or symptoms that should load this skill.
---

# {title}

{placeholder} one or two sentences on what this skill is for. Everything about
*when* to load it belongs in the description above, because the body only loads
after the skill has already triggered.

## Workflow

### 1. {placeholder}

- {placeholder} imperative steps, in order, where order matters.
- Include only what an agent cannot reliably infer.

### 2. {placeholder}

## Resources

{resources}
"""

RESOURCE_LINES = {
    "references": "- [reference.md](references/reference.md) - {placeholder} when and why to read it.",
    "scripts": "- `scripts/helper.py` - {placeholder} when and why to run it.",
    "assets": "- `assets/template.md` - {placeholder} what output it produces.",
}

EVAL_TEMPLATE = """# {placeholder} the failure this scenario tests

## Prompt

> {placeholder} what a person would actually say, with enough detail to be real.

## Baseline failure

{placeholder} what an agent does *without* this skill. Observe it; do not
predict it. This is the RED half, and a scenario written without it is a guess
about a failure rather than a record of one.

## Pass criteria

- {placeholder}

## Fail signals

- {placeholder}

## Not yet run

Fixture only.
"""


def _render(text: str, **fields: str) -> str:
    return text.format(placeholder=PLACEHOLDER, **fields)


def create_skill(name: str, root: Path, resources: list[str], evals: bool) -> Path:
    """Write the folder and return it. Raises ValueError rather than overwriting."""
    if not NAME_RE.fullmatch(name) or len(name) > MAX_NAME_CHARS:
        raise ValueError(
            f"Invalid name {name!r}: lowercase letters, digits and single hyphens, "
            f"at most {MAX_NAME_CHARS} characters. Prefer a verb-led name such as "
            f"'review-migrations', because a skill is a procedure."
        )

    folder = root / name
    if folder.exists():
        raise ValueError(f"{folder} already exists; refusing to overwrite it")

    wanted = ["references", *(r for r in resources if r in RESOURCE_LINES)]
    lines = "\n".join(_render(RESOURCE_LINES[r]) for r in dict.fromkeys(wanted))

    folder.mkdir(parents=True)
    (folder / "SKILL.md").write_text(
        _render(SKILL_TEMPLATE, name=name, title=name.replace("-", " ").title(), resources=lines),
        encoding="utf-8",
    )

    for resource in dict.fromkeys(wanted):
        (folder / resource).mkdir()
    (folder / "references" / "reference.md").write_text(
        f"# {PLACEHOLDER}\n\nDepth that would bloat the body. Loaded on demand.\n",
        encoding="utf-8",
    )

    if evals:
        (folder / "evals").mkdir()
        (folder / "evals" / "scenario.md").write_text(_render(EVAL_TEMPLATE), encoding="utf-8")

    return folder


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("name", help="Skill name, which is also the folder name")
    parser.add_argument("--path", required=True, type=Path, help="Skills root to create it in")
    parser.add_argument("--resources", default="", help="Comma-separated: scripts,assets")
    parser.add_argument("--evals", action="store_true", help="Also scaffold an eval scenario")
    args = parser.parse_args(argv)

    try:
        folder = create_skill(
            args.name,
            args.path,
            [r.strip() for r in args.resources.split(",") if r.strip()],
            args.evals,
        )
    except ValueError as exc:
        print(str(exc))
        return 1

    print(f"Created {folder}")
    print(f"Replace every {PLACEHOLDER} before shipping. Next: write the description, then")
    print("micro-test it, then observe a baseline failure. validate_skill.py will fail")
    print("until the placeholders are gone, which is deliberate.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
