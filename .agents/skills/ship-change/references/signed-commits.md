# Signed commits at push time

Read this at step 6 before the first push of a proposal branch.

1. Read the base branch's rules: `gh api repos/OWNER/REPO/rules/branches/BASE`. An empty list
   means no ruleset applies. An error is unknown, not absent: use the signing path if there is
   one, otherwise return `incomplete` with the error. This endpoint lists rulesets only; classic
   branch protection's signature setting is not readable by an App token, so a requirement set
   only there surfaces later as a blocked pull request, handled like item 4.
2. If the rules do not include `required_signatures`, or every commit is already signed locally,
   push as step 6 describes.
3. Otherwise a plain `git push` publishes commits that block the pull request, squash merge
   included, and only a force-push can replace them. Push through the environment's signing path
   instead: for the SPEC-0003 worker App, `agent-push`, which re-creates each commit through the
   GitHub API so GitHub signs it, then moves the local branch onto the signed commits. When BASE
   is not the default branch, pass `--base origin/BASE`; its closing signature check still starts
   at the default branch, so unsigned commits already on BASE end in a refusal.
4. If there is no signing path, or it refuses, report `push: blocked` with its reason,
   `pr: blocked` and status `incomplete`, and stop before the pull request. Never fall back to
   `git push`, sign by rewriting history, or force. If it exits non-zero after sending commits,
   also report the remote branch head.
5. A remote branch left by an earlier `agent-push` run is expected work, not a conflict: running
   `agent-push` again resumes from the signed copies it already pushed. A remote branch holding
   unsigned commits is `incomplete`: replacing them needs a force-push, which is the owner's
   decision.
6. After a signing push, the head SHA is the remote signed commit, not the local one. Resolve it
   again before steps 7 to 13.

| Rationalization | Required response |
| --- | --- |
| "Signatures only matter at merge, so git push is fine for now" | An unsigned commit pushed now can only be replaced by a force-push. Use the signing path or report `blocked`. |
| "The squash commit is signed, so the branch commits do not matter" | GitHub tests mergeability against the head commit; unsigned head commits block a squash merge too. |
| "The rules call failed, so there are no rules" | An error is unknown. Use the signing path, or return `incomplete`. |
