#!/usr/bin/env python3
"""Check a skill's structure without needing the catalog CLI.

`lexicon validate` is the gate for this catalog and is the better checker: it is
tested, it sees every artifact at once, and it knows about agents and schemas.
It is also unreachable from an installed skill, because a project that installs
`create-skill` does not get the CLI. This script is what travels, so the
instruction to validate is one the reader can actually follow.

The two are kept in agreement by `tests/test_skill_validator_parity.py`, which
runs both over the catalog and over a table of seeded defects. When a rule
changes, change it in both and let that test prove it.

Usage:
    python3 scripts/validate_skill.py <path/to/skill-folder> [more...]

Exits 0 when every skill is clean, 1 when any problem is found, and 2 when a
path is not a skill folder at all.
"""

from __future__ import annotations

import re
import sys
from pathlib import Path

NAME_RE = re.compile(r"[a-z0-9]+(?:-[a-z0-9]+)*")
MAX_NAME_CHARS = 64
MAX_DESCRIPTION_CHARS = 1024
MAX_COMPATIBILITY_CHARS = 500
MAX_BODY_LINES = 200

# The Agent Skills format defines these six and nothing else. A field outside
# the set is rejected by packaging and upload rather than ignored, so an
# invented field is a distribution failure rather than a harmless extra.
SPEC_FIELDS = frozenset(
    {"name", "description", "license", "compatibility", "metadata", "allowed-tools"}
)

FRONTMATTER_RE = re.compile(r"\A---\n(.*?)\n---\n(.*)\Z", re.DOTALL)
LINK_RE = re.compile(r"\[[^\]]*\]\(([^)]+)\)")

# An unreplaced scaffold marker. Checked in the description only: a body may
# legitimately discuss TODO comments, while a description never holds one
# deliberately. A scaffolded skill that passes every check still triggers for
# no request, and that failure looks like success.
PLACEHOLDER_RE = re.compile(r"\bTODO\b")


def _parse_frontmatter(raw: str) -> dict:
    """Parse the frontmatter, preferring PyYAML and falling back to a subset.

    Every failure leaves here as a ValueError, whichever parser produced it.
    `yaml.YAMLError` inherits from Exception rather than ValueError, so letting
    it out turns a skill with a typo in its frontmatter into a traceback, and
    the run stops at the first bad file instead of reporting it and continuing.
    The catalog learned this already: `src/agent_toolkit/yamlfile.py` translates
    at the parse call for the same reason, and this script follows it rather
    than asking every caller to know which parser ran.
    """
    try:
        import yaml
    except ImportError:
        return _parse_without_yaml(raw)

    try:
        loaded = yaml.safe_load(raw)
    except yaml.YAMLError as exc:
        raise ValueError(f"Invalid YAML: {exc}") from exc

    if loaded is None:
        return {}
    if not isinstance(loaded, dict):
        # TRY004 asks for TypeError here and this script briefly did that, which
        # is what put the raise outside its caller's except clause. The rule is
        # about a caller passing the wrong type, a bug in the program. This is a
        # value someone typed into a file, so it is bad input and belongs with
        # every other bad-input error.
        raise ValueError("Frontmatter is not a mapping")  # noqa: TRY004
    return loaded


# A key declared with no value, before the lines under it say whether it is a
# map, a list, or genuinely empty.
_UNSET = object()

# YAML 1.1 words PyYAML resolves to a bool, which matter here because a field
# checked for being a string has to look like one under both parsers.
_TRUE = frozenset({"true", "yes", "on"})
_FALSE = frozenset({"false", "no", "off"})
_NULL = frozenset({"null", "~", ""})


def _scalar(text: str):
    """One value, typed the way `yaml.safe_load` types it.

    Quoting wins: a quoted `"true"` is the string, which is what keeps a
    description reading `"TODO"` a description rather than a boolean.
    """
    text = text.strip()
    if len(text) >= 2 and text[0] == text[-1] and text[0] in "'\"":
        return text[1:-1]
    lowered = text.lower()
    if lowered in _TRUE:
        return True
    if lowered in _FALSE:
        return False
    if lowered in _NULL:
        return None
    for cast in (int, float):
        try:
            return cast(text)
        except ValueError:
            continue
    return text


def _value(text: str):
    """A value written on the same line as its key: inline list, scalar, or nothing."""
    text = text.strip()
    if not text:
        return _UNSET
    if text.startswith("["):
        # An unterminated flow sequence is a YAML error. Reading it as a plain
        # string would let this parser pass a file PyYAML fails on.
        if not text.endswith("]"):
            raise ValueError(f"Unterminated inline list: {text!r}")
        inner = text[1:-1].strip()
        return [_scalar(part) for part in inner.split(",")] if inner else []
    # An unquoted value cannot hold ": " or end in a colon; YAML reads that as a
    # nested mapping and refuses it. Rejecting it here keeps this parser from
    # passing a file the catalog's YAML parser fails on.
    if text[0] not in "'\"" and (": " in text or text.endswith(":")):
        raise ValueError(
            f"Value {text!r} needs quoting: an unquoted value cannot contain "
            f"a colon followed by a space, or end in one."
        )
    return _scalar(text)


def _parse_without_yaml(raw: str) -> dict:
    """Read the frontmatter without PyYAML, for an install that does not have it.

    Covers the shapes the six fields can take: flat scalars, one level of nested
    map for `metadata`, and sequences in block or inline form for
    `allowed-tools`. It is a lint, not a YAML implementation: anything outside
    those shapes is reported rather than guessed at, because a checker that
    quietly misreads frontmatter is worse than no checker.

    This half runs precisely where the script matters, in a project with no
    catalog and no dependency guarantee, so
    `tests/test_skill_validator_parity.py` pins its output against PyYAML's.
    Without that, the two parsers reach different verdicts on the same file and
    the one nobody tests is the one the reader runs.
    """
    data: dict = {}
    current: str | None = None
    for number, line in enumerate(raw.splitlines(), start=1):
        if not line.strip() or line.lstrip().startswith("#"):
            continue

        # YAML forbids a tab as an indentation character outright. Accepting one
        # here would pass a file PyYAML refuses, which is the same defect as
        # rejecting one it accepts, pointed the other way.
        indent = line[: len(line) - len(line.lstrip())]
        if "\t" in indent:
            raise ValueError(f"Line {number} is indented with a tab, which YAML forbids")

        if line.startswith(" "):
            if current is None:
                raise ValueError(f"Indented line {number} has no parent key")
            entry = line.strip()

            if entry.startswith("-"):
                if data[current] is _UNSET:
                    data[current] = []
                if not isinstance(data[current], list):
                    raise ValueError(f"Line {number} adds a list item to a {current!r} that is not a list")
                data[current].append(_scalar(entry[1:]))
                continue

            key, separator, value = entry.partition(":")
            if not separator:
                raise ValueError(f"Line {number} is not a key/value pair")
            if data[current] is _UNSET:
                data[current] = {}
            if not isinstance(data[current], dict):
                raise ValueError(f"Line {number} nests under a scalar")
            nested = _value(value)
            data[current][key.strip()] = None if nested is _UNSET else nested
            continue

        key, separator, value = line.partition(":")
        if not separator:
            raise ValueError(f"Line {number} is not a key/value pair")
        current = key.strip()
        data[current] = _value(value)

    # PyYAML reads a key with nothing under it as None. Match that, so a rule
    # written against one parser reaches the same verdict under the other.
    return {key: (None if value is _UNSET else value) for key, value in data.items()}


def _check_fields(metadata: dict, problems: list[str]) -> None:
    extras = sorted(set(metadata) - SPEC_FIELDS)
    if extras:
        problems.append(
            f"Frontmatter fields outside the format: {', '.join(extras)}. "
            f"The format defines {', '.join(sorted(SPEC_FIELDS))}; host-only "
            f"behavior belongs in an adapter."
        )

    # Membership, not `.get()`: `metadata:` with nothing under it parses to
    # None, which is a map a host cannot read rather than a field left out.
    # `lexicon validate` reports it, so this has to as well.
    if "metadata" in metadata:
        value = metadata["metadata"]
        if not isinstance(value, dict):
            problems.append("'metadata' must be a map; a host drops any other shape silently")
        else:
            bad = sorted(
                str(key)
                for key, item in value.items()
                if not isinstance(key, str) or not isinstance(item, str)
            )
            if bad:
                problems.append(f"'metadata' takes string keys and values: {', '.join(bad)}")

    compatibility = metadata.get("compatibility")
    if compatibility is not None:
        if not isinstance(compatibility, str):
            problems.append("'compatibility' must be a string")
        elif len(compatibility) > MAX_COMPATIBILITY_CHARS:
            problems.append(f"'compatibility' is over {MAX_COMPATIBILITY_CHARS} characters")

    if metadata.get("license") is not None and not isinstance(metadata["license"], str):
        problems.append("'license' must be a string")

    tools = metadata.get("allowed-tools")
    if tools is not None and not isinstance(tools, (str, list)):
        problems.append("'allowed-tools' must be a string or a list")


def _check_links(body: str, folder: Path, problems: list[str]) -> None:
    """A dead link is depth that never loads, which fails silently."""
    for target in LINK_RE.findall(body):
        target = target.split("#", 1)[0].strip()
        if not target or "://" in target or target.startswith(("mailto:", "/", "~")):
            continue
        if not (folder / target).exists():
            problems.append(f"Reference link does not resolve: {target}")


def check_skill(folder: Path) -> list[str]:
    """Every problem found in one skill folder, as sentences a reader can act on."""
    problems: list[str] = []
    source = folder / "SKILL.md"
    if not source.is_file():
        return [f"No SKILL.md in {folder}"]

    match = FRONTMATTER_RE.match(source.read_text(encoding="utf-8"))
    if match is None:
        return [
            (
                "Frontmatter is missing or malformed. This fails silently on a host: "
                "the body loads with empty metadata, so direct invocation still works "
                "while the description matches nothing and the skill never triggers."
            )
        ]

    try:
        metadata = _parse_frontmatter(match.group(1))
    except ValueError as exc:
        return [f"Frontmatter does not parse: {exc}"]

    body = match.group(2)
    _check_fields(metadata, problems)

    name = metadata.get("name")
    if not isinstance(name, str) or not name.strip():
        problems.append("Missing name")
    else:
        if not NAME_RE.fullmatch(name) or len(name) > MAX_NAME_CHARS:
            problems.append(
                f"Invalid name {name!r}: lowercase letters, digits and single "
                f"hyphens, at most {MAX_NAME_CHARS} characters"
            )
        if name != folder.name:
            problems.append(f"Folder {folder.name!r} does not match name {name!r}")

    description = metadata.get("description")
    if not isinstance(description, str) or not description.strip():
        problems.append("Missing description")
    elif len(description) > MAX_DESCRIPTION_CHARS:
        problems.append(f"Description is over {MAX_DESCRIPTION_CHARS} characters")
    elif PLACEHOLDER_RE.search(description):
        problems.append(
            "Description still holds a TODO placeholder. A scaffolded description "
            "routes on nothing, so the skill never triggers."
        )
    elif "use when" not in description.lower():
        # A house rule of this catalog, not a requirement of the format. The
        # format caps the length and mandates no wording.
        problems.append("House rule: description should state triggers as 'Use when ...'")

    if not body.strip():
        problems.append("Empty instructions")

    lines = len(body.splitlines())
    if lines > MAX_BODY_LINES:
        problems.append(
            f"Body is {lines} lines, over the {MAX_BODY_LINES} line limit. "
            f"Move depth into references/ so it loads only when needed."
        )

    nested = sorted(p for p in folder.rglob("SKILL.md") if p != source)
    problems.extend(
        f"Nested SKILL.md: {path.relative_to(folder)}. A host discovers it as its "
        f"own skill, so one skill becomes several fragments."
        for path in nested
    )

    _check_links(body, folder, problems)
    return problems


def main(argv: list[str]) -> int:
    if not argv:
        print(__doc__)
        return 2

    failed = False
    for raw in argv:
        folder = Path(raw).resolve()
        if not folder.is_dir():
            print(f"{raw}: not a directory")
            return 2
        problems = check_skill(folder)
        if problems:
            failed = True
            print(f"{folder.name}: {len(problems)} problem(s)")
            for problem in problems:
                print(f"  - {problem}")
        else:
            print(f"{folder.name}: ok")
    return 1 if failed else 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
