# Reckon inputs

A separate report shape from the reconciliation report in the main workflow.
Use it when a session is closing an increment and needs the supporting facts
a Reckon record cites, per SPEC-0010 section 3.3 (Restock, Run, Reckon).

## The boundary

**This shape stops at inputs.** It does not write the Reckon record and does
not produce any of the eight Reckon judgment fields: confirmed assumptions,
disproven assumptions, newly discovered constraints, new risks, unexpected
opportunities, whether the outcome was achieved, implications, or a
disposition (`continue` / `remediate` / `reshape` / `investigate` / `accept`
/ `defer` / `stop`). All eight are judgment, decided at M3 close on
lexicon#140. An agent holding the gathered facts and drafting the record
anyway is the failure this shape exists to prevent; do not do that, and do
not summarize the facts in a way that reads as a disposition.

The record itself goes in a comment headed `### Reckon record` on the
tracking issue. This shape does not write there, or anywhere: it reads and
reports.

## The five facts

Gather all five. Report "unknown" or "unavailable" for one you cannot
establish rather than omitting it silently.

1. **Board counts per Status column, per repository.** First establish
   whether the repository is on a board at all. Read `projectItems` for the
   repository's open issues, every page of them:

   ```sh
   gh api graphql --paginate -F owner=OWNER -F name=REPO -f query='
     query($owner: String!, $name: String!, $endCursor: String) {
       repository(owner: $owner, name: $name) {
         issues(first: 100, states: OPEN, after: $endCursor) {
           pageInfo { hasNextPage endCursor }
           nodes { number projectItems(first: 5) { nodes { project { url } } } }
         }
       }
     }'
   ```

   Read the same `projectItems` for the tracking issue, open or closed, and
   for the issues closed since the previous Reckon record (fact 3), each with
   `issue(number: N)`, so a repository whose tracked work has just closed is
   not mistaken for one on no board. When every read succeeds and no issue
   is an item on any Project, the repository is **board-less**
   (SPEC-0010 section 2): report this fact as **no board**, not as zero
   items per column, and do not go looking for the repository on some other
   board. When the read fails (a token without the `read:project` scope
   errors rather than returning an empty list), the fact is **unknown**, not
   "no board".

   Otherwise read the Project's item list directly. `flow_report.py`'s
   `aging()` measure only covers items currently in `Doing` or `Review`; it
   does not report `Ready`, no-Status, or done items, so it cannot supply
   this fact on its own.

2. **The interval's flow figures.** These come from `flow_report.py`
   (lexicon#141), which ships in a Lexicon checkout, not in this skill.
   Resolve the checkout from exactly one of these, in order, and stop at
   the first that applies:
   - the current repository, when it is Lexicon itself;
   - the `LEXICON_CATALOG_ROOT` environment variable, the same setting
     that points the `lexicon` command at a checkout;
   - a checkout path the caller names for this run.

   Then run
   `python3 <checkout>/integration-library/github-tracking/flow_report.py
   --repo OWNER/REPO --project OWNER/NUMBER --window-days N --json` and
   report its `lead_time`, `throughput`, and `aging` measures as given. Do
   not reimplement its GraphQL or recompute a figure it already reports;
   cite its own `status` field (`known` / `unknown` / `empty`) rather than
   silently treating an unknown measure as zero or absent.

   When none applies, or the resolved checkout has no `flow_report.py`,
   report the flow figures as **unavailable** and name which it was. Do
   not search the filesystem for a checkout or guess a path.

   For a board-less repository, `lead_time` and `throughput` come from the
   repository's own issues and stand as reported, but `--project` is still
   required: pass the Project the caller names, and when the caller names
   none, report the flow figures as unavailable for that reason. Report
   `aging` as **no board**: the tool reports `empty` because no item from
   the repository is on the Project, which reads as "nothing in `Doing` or
   `Review`" and is not. Fact 1's read, not this measure, establishes that
   there is no board. If the tool instead reports `unknown`, report no board
   and give its reason beside it: the gap concerns an item the tool could not
   read, which may belong to another repository.

3. **PRs merged and issues closed since the previous Reckon record.** The
   caller names which previous record (by date or by comment); read PRs
   merged and issues closed in that repository since that boundary. If no
   previous record exists, say so; do not guess a window.

4. **One `hold` / `not hold` / `unknown` line per acceptance criterion in
   scope.** Take the criteria from the tracking issue's `### Done when`
   section (the work-item contract in
   `integration-library/github-tracking/work_item_check.py`). For each one,
   apply this skill's own evidence-authority and implementation-state
   classification (Workflow steps 1 to 3) and collapse the result to one of
   three words:
   - **hold:** the criterion's claim is still true on the evidence
     inspected (implemented on the integration branch, and any validation
     the criterion itself names has been observed, not merely present).
   - **not hold:** the evidence contradicts the claim (not built in
     inspected revisions, or built only on another branch).
   - **unknown:** the evidence is insufficient either way. Name the missing
     evidence, the same way Workflow step 3's "Unknown or partial" state
     requires.

5. **Staleness flags.** Three kinds, each checked explicitly rather than
   left to fall out of the board read:
   - An item carrying no Status field at all.
   - An issue sitting in `Review` whose linked pull request has already
     merged.
   - A `Ready` item with no assignee.

   For a board-less repository all three are **not applicable: no board**,
   stated as such; none of them can be checked, and an empty list would
   read as "checked and clean".

   These two specific patterns (a merged-but-open Review item, an
   unassigned Ready item) are what this shape was filed to catch: iteration
   1's Review column sat at 10, above the SPEC-0008 ceiling of 5 per
   repository, because issues whose pull requests had already merged were
   never moved out of it, and iteration 2 had Ready items with no assignee.
   Flag every instance found, not just one example of each kind.

## Report shape

Five labeled sections, one per fact above, each citing its source (the
board read, the `flow_report.py` output or the reason it was unavailable,
the merged/closed read, the tracking issue's Done when section, and the
board read again for staleness). No sixth section. No narrative recommending what to do about
any of it: that framing belongs to the Reckon record, which is not this
shape's output.
