---
name: resolve-pr-feedback
description: Fetch a GitHub pull request's unresolved review comments, apply code fixes for the actionable ones, and push the resulting commits back to that PR. Use when asked to address PR feedback, resolve review comments, respond to change requests on a pull request, or iterate on an open PR by number until reviewers' comments are resolved.
---

# Resolve PR Feedback

Turn a GitHub pull request's outstanding review feedback into pushed commits, then leave the PR ready for another review pass. Designed to be re-run on the same PR number after each new round of comments.

## Requirements

- GitHub CLI (`gh`) installed and authenticated (`gh auth status`) with write access to the PR's repository.
- Python 3.9+ available to run the bundled helper script.
- A local git checkout of the repository (or one this skill can create with `gh pr checkout`).

## Workflow

1. **Resolve the target.** Get `owner/name` (from the current repo's `origin` remote if not given) and the PR number from the user. Confirm both before continuing if either is ambiguous.
2. **Sync the checkout.** Run `gh pr checkout <number> --repo <owner>/<name>` to fetch and switch to the PR's branch. If the working tree already has uncommitted changes unrelated to this task, stop and ask how to handle them rather than discarding or mixing them in.
3. **Fetch feedback.**
   ```
   python scripts/pr_feedback.py fetch --repo <owner>/<name> --pr <number>
   ```
   Returns JSON with three feedback sources, each already filtered to what still needs attention:
   - `unresolved_threads`: inline review comment threads not yet marked resolved, any age.
   - `recent_issue_comments`: general PR conversation comments posted after the current head commit, from someone other than the authenticated `gh` user.
   - `recent_reviews`: review verdicts (`CHANGES_REQUESTED`/`COMMENTED`) with a body, posted after the current head commit.
4. **Triage each item.** For every comment decide: actionable code fix, discussion-only (needs a reply but no code change), or out of scope. Note threads marked `outdated` (the diff context has already moved) separately, but still address the underlying request if it still applies.
5. **Respect guarded boundaries.** If a comment asks for a change to authentication, secrets, persistence schemas, security controls, or deployment/infrastructure state, pause and get explicit confirmation for that specific change before applying it; everything else can proceed. This skill inherits the repository's normal guarded-change policy rather than relaxing it.
6. **Apply fixes.** Make the smallest change that satisfies each actionable comment. Keep unrelated code untouched.
7. **Verify.** Run the project's existing tests/build/lint commands proportional to what changed.
8. **Commit and push.**
   - `git commit` with a message that names the PR and summarizes what was addressed, e.g. `Address review feedback on PR #123: validate empty input, fix off-by-one in pagination`.
   - `git push`. Never force-push. If the push is rejected because the remote branch has moved, stop and ask rather than rebasing or force-pushing over someone else's commits.
9. **Close the loop on GitHub.** For each addressed item:
   - Reply so the reviewer sees it was handled: `python scripts/pr_feedback.py reply --repo <owner>/<name> --pr <number> --comment-id <first_comment_id> --body "..."`.
   - Mark the thread resolved: `python scripts/pr_feedback.py resolve --thread-id <thread_id>`.
   - For feedback not tied to a specific inline thread (general comments, review verdicts), post one summary comment instead: `python scripts/pr_feedback.py comment --repo <owner>/<name> --pr <number> --body "..."`.
10. **Report.** Tell the user what was fixed, what was skipped and why (guarded, ambiguous, out of scope, needs discussion), and that the PR is ready for another review pass. Re-run this skill on the same PR later to pick up whatever the next round of comments leaves.

## Resources

- `scripts/pr_feedback.py`: deterministic `gh`-backed helper with `fetch`, `reply`, `resolve`, and `comment` subcommands. Run `python scripts/pr_feedback.py --help` or `python scripts/pr_feedback.py <subcommand> --help` for full argument lists; `--body-file` is available anywhere `--body` is, for long comment text.
