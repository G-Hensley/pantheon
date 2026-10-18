#!/usr/bin/env bash
# Check the day-one surface of a scaffolded repository.
#
# This is the narrow check, not the conformance check. check-tree.py --repo in
# G-Hensley/projects is the authority, and the skill runs it afterwards and shows
# its output verbatim. This script covers what that mode does not assert: the
# .github/ set, the four docs/ indexes, CODEOWNERS routing, the AGENTS.md line
# budget, surviving placeholders, em dashes, and sections still carrying the
# skeleton's guidance text. The two overlap on the instruction files, the root
# README.md and docs/README.md; where they ever disagree, the checker is right and
# this script is the thing to fix.
#
# Usage: verify-day-one.sh TARGET_DIR [--lifecycle]
# --lifecycle asserts the opt-in Life Cycle section landed in AGENTS.md; omit it
# when the project did not opt in, since nothing else here knows that decision.
# Exit 0 all clear, 1 at least one failure.

set -uo pipefail

target=${1:-}
lifecycle=0
if [ "${2:-}" = "--lifecycle" ]; then lifecycle=1; fi
if [ -z "$target" ] || [ ! -d "$target" ]; then
  echo "usage: verify-day-one.sh TARGET_DIR [--lifecycle]" >&2
  exit 64
fi

fails=0
fail() { printf 'FAIL  %s\n' "$1"; fails=$((fails + 1)); }
ok()   { printf 'ok    %s\n' "$1"; }

FILES=(
  README.md AGENTS.md CLAUDE.md
  .github/CODEOWNERS .github/pull_request_template.md
  .github/ISSUE_TEMPLATE/work-item.yml .github/ISSUE_TEMPLATE/bug.yml
  .github/ISSUE_TEMPLATE/security.yml .github/ISSUE_TEMPLATE/config.yml
  docs/README.md docs/specs/README.md docs/decisions/README.md
  docs/ideas/README.md docs/archive/README.md
)

for f in "${FILES[@]}"; do
  if [ -f "$target/$f" ]; then ok "$f present"; else fail "$f is missing"; fi
done

claude="$target/CLAUDE.md"
if [ -L "$claude" ]; then
  if [ "$(readlink "$claude")" = "AGENTS.md" ]; then
    ok "CLAUDE.md is a symlink to its sibling AGENTS.md"
  else
    fail "CLAUDE.md is a symlink to $(readlink "$claude"), not its sibling AGENTS.md"
  fi
elif [ -f "$claude" ]; then
  if [ "$(tr -d '[:space:]' < "$claude")" = "@AGENTS.md" ]; then
    ok "CLAUDE.md holds only @AGENTS.md"
  else
    fail "CLAUDE.md holds more than @AGENTS.md, so the rules now have two homes"
  fi
fi

if [ -f "$target/AGENTS.md" ]; then
  lines=$(wc -l < "$target/AGENTS.md")
  if [ "$lines" -le 150 ]; then
    ok "AGENTS.md is $lines lines, within the 150 line budget"
  else
    fail "AGENTS.md is $lines lines, over the 150 line budget; move depth into docs/ or a skill"
  fi
fi

co="$target/.github/CODEOWNERS"
if [ -f "$co" ]; then
  for path in /AGENTS.md /CLAUDE.md /.github/CODEOWNERS; do
    if grep -q "^${path}[[:space:]]" "$co"; then
      ok "CODEOWNERS routes $path"
    else
      fail "CODEOWNERS has no rule for $path"
    fi
  done
  if grep -qE '@[A-Za-z0-9][-A-Za-z0-9/]*' "$co"; then
    ok "CODEOWNERS names an owner"
  else
    fail "CODEOWNERS names no owner, so it routes nothing"
  fi
fi

GREP_EXCLUDES=(
  --exclude-dir=.git --exclude-dir=.agents --exclude-dir=.claude
  --exclude-dir=.codex --exclude-dir=.opencode --exclude-dir=node_modules
  --exclude-dir=.venv
)

left=$(grep -rln "${GREP_EXCLUDES[@]}" '<repo>\|<owner>\|<Project name>\|OWNER/REPO' -- "$target" 2>/dev/null)
if [ -n "$left" ]; then
  fail "placeholders remain in: $(echo "$left" | tr '\n' ' ')"
else
  ok "no placeholders remain"
fi

dashes=$(grep -rln "${GREP_EXCLUDES[@]}" '—' -- "$target" 2>/dev/null)
if [ -n "$dashes" ]; then
  fail "em dashes in authored prose: $(echo "$dashes" | tr '\n' ' ')"
else
  ok "no em dashes"
fi

# Skeleton guidance left verbatim is the common failure: the file exists, reads
# plausibly, and says nothing about this repository. One entry per section,
# FILE|first line of its guidance, as the skeleton carried it at
# G-Hensley/projects d09972b. Matched as a whole line, so a sentence that only
# starts the same way passes; a guidance line kept above real content still
# fails, because the guidance is a question to answer, not an opening to keep.
# A section may be deleted rather than filled.
unfilled=0
while IFS='|' read -r file line; do
  if [ -f "$target/$file" ] && grep -qxF -- "$line" "$target/$file"; then
    fail "$file still carries skeleton guidance: \"$line\""
    unfilled=1
  fi
done <<'GUIDANCE'
AGENTS.md|What this is, its current status, and what it is not.
AGENTS.md|Only the paths an agent would get wrong by looking: what is generated, what is not editable, what a
AGENTS.md|Invariants that hold in this repository and would be wrong elsewhere.
AGENTS.md|Minimum verification expectations.
AGENTS.md|Trust assumptions specific to this repository: its boundaries, its privileged components, how it
AGENTS.md|Only what differs here: branch naming, required checks, and whether any standing delivery
AGENTS.md|Relevant agent skills and when to use them.
AGENTS.md|Important recurring failure modes to avoid.
README.md|One paragraph: what this is, who it is for, and what it is not. The root README is for humans;
README.md|Active, planning, or archived, with the date this line was last true.
README.md|Prerequisites, then the build, test and run commands. Keep them identical to the `Commands`
GUIDANCE
# The skeleton's Quick start is a fenced block holding only a comment. Keeping
# the comment is fine; fenced code with nothing but comments is not. A Quick
# start rewritten as prose, with no fence, is left to the reader.
if [ -f "$target/README.md" ] && grep -q '^## Quick start' "$target/README.md"; then
  if ! awk '
    /^```/ { fence = !fence; if (section) fenced = 1; next }
    !fence && /^## / { section = ($0 ~ /^## Quick start/); next }
    section && fence && $0 !~ /^[[:space:]]*(#|$)/ { found = 1 }
    END { exit fenced && !found }
  ' "$target/README.md"; then
    fail "README.md Quick start has no command"
    unfilled=1
  fi
fi
# The skeleton's Commands list names each command with nothing after it. Say
# "none" and why rather than leave one empty, or delete the line.
if [ -f "$target/AGENTS.md" ]; then
  empty=$(sed -n -E 's/^- (build|test|lint|typecheck|run):[[:space:]]*$/\1/p' "$target/AGENTS.md" | tr '\n' ' ')
  if [ -n "$empty" ]; then
    fail "AGENTS.md leaves a command empty: ${empty% }"
    unfilled=1
  fi
fi
[ "$unfilled" -eq 0 ] && ok "AGENTS.md and README.md sections have been filled in"

if [ "$lifecycle" -eq 1 ]; then
  if [ -f "$target/AGENTS.md" ] && grep -q "docs/specs/0010-ai-development-life-cycle/spec.md" "$target/AGENTS.md"; then
    ok "Life Cycle section present (opted in)"
  else
    fail "opted into the life cycle but AGENTS.md has no Life Cycle section"
  fi
fi

printf '\n%d failure(s)\n' "$fails"
[ "$fails" -eq 0 ]
