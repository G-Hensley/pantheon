#!/usr/bin/env bash
# Copy the day-one documentation surface out of the Repository Documentation
# Standard's skeleton and substitute its placeholders.
#
# Mechanical only. It writes no prose: every file it lands still carries the
# skeleton's guidance headings, and the skill's caller replaces those with real
# content afterwards.
#
# Usage:
#   copy-skeleton.sh --skeleton DIR --target DIR --owner NAME --repo NAME \
#                    --project-name "Human readable name"
#
# Refuses to overwrite. If any file it would write already exists, it names all
# of them and exits 2 without touching the target, because a half-scaffolded
# repository is worse than an unscaffolded one.

set -euo pipefail

skeleton=''
target=''
owner=''
repo=''
project_name=''

while [ $# -gt 0 ]; do
  case "$1" in
    --skeleton) skeleton=$2; shift 2 ;;
    --target) target=$2; shift 2 ;;
    --owner) owner=$2; shift 2 ;;
    --repo) repo=$2; shift 2 ;;
    --project-name) project_name=$2; shift 2 ;;
    *) echo "unknown argument: $1" >&2; exit 64 ;;
  esac
done

for required in skeleton target owner repo project_name; do
  if [ -z "${!required}" ]; then
    echo "missing --${required//_/-}" >&2
    exit 64
  fi
done

if [ ! -d "$skeleton" ]; then
  echo "skeleton directory not found: $skeleton" >&2
  exit 66
fi

# The three values are spliced into a sed script below. sed reads a newline as
# the end of one command and the start of the next, so a value carrying one
# could append commands of its own (GNU sed's `e` runs a shell). Names do not
# contain control characters; refuse them rather than escape them.
for value in "$owner" "$repo" "$project_name"; do
  case "$value" in
    *[[:cntrl:]]*)
      echo "--owner, --repo and --project-name must not contain control characters" >&2
      exit 64 ;;
  esac
done

# The day-one surface. Everything the standard creates only when content exists
# for it is deliberately absent; see references/day-one-surface.md.
FILES=(
  README.md
  AGENTS.md
  CLAUDE.md
  .github/CODEOWNERS
  .github/pull_request_template.md
  .github/ISSUE_TEMPLATE/work-item.yml
  .github/ISSUE_TEMPLATE/bug.yml
  .github/ISSUE_TEMPLATE/security.yml
  .github/ISSUE_TEMPLATE/config.yml
  docs/README.md
  docs/specs/README.md
  docs/specs/0000-template/spec.md
  docs/specs/0000-template/design.md
  docs/decisions/README.md
  docs/decisions/0000-template.md
  docs/ideas/README.md
  docs/archive/README.md
)

missing=()
for f in "${FILES[@]}"; do
  [ -f "$skeleton/$f" ] || missing+=("$f")
done
if [ ${#missing[@]} -gt 0 ]; then
  echo "skeleton is missing files this skill expects; it has moved or changed shape:" >&2
  printf '  %s\n' "${missing[@]}" >&2
  exit 66
fi

existing=()
for f in "${FILES[@]}"; do
  [ -e "$target/$f" ] && existing+=("$f")
done
if [ ${#existing[@]} -gt 0 ]; then
  echo "target already has files this would overwrite; nothing was written:" >&2
  printf '  %s\n' "${existing[@]}" >&2
  echo "Scaffold into an empty repository, or land the missing files by hand." >&2
  exit 2
fi

for f in "${FILES[@]}"; do
  mkdir -p "$target/$(dirname "$f")"
  cp "$skeleton/$f" "$target/$f"
done

# Placeholder substitution. Each pattern is one the skeleton actually contains;
# see references/day-one-surface.md for where each appears and why.
esc() { printf '%s' "$1" | sed -e 's/[&|\\]/\\&/g'; }
owner_e=$(esc "$owner")
repo_e=$(esc "$repo")
name_e=$(esc "$project_name")

for f in "${FILES[@]}"; do
  sed -i \
    -e "s|G-Hensley/<repo>|${owner_e}/${repo_e}|g" \
    -e "s|OWNER/REPO|${owner_e}/${repo_e}|g" \
    -e "s|@<owner>|@${owner_e}|g" \
    -e "s|- <owner>|- ${owner_e}|g" \
    -e "s|<Project name>|${name_e}|g" \
    "$target/$f"
done

written=()
for f in "${FILES[@]}"; do
  written+=("$target/$f")
done
if [ ${#written[@]} -eq 0 ]; then
  echo "internal error: no scaffold files to check" >&2
  exit 70
fi
remaining=$(grep -ln '<repo>\|<owner>\|<Project name>\|OWNER/REPO' -- "${written[@]}" || true)
if [ -n "$remaining" ]; then
  echo "placeholders survived substitution in:" >&2
  printf '%s\n' "$remaining" >&2
  exit 3
fi

printf 'Wrote %d files into %s\n' "${#FILES[@]}" "$target"
