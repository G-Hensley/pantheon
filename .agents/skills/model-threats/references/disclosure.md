# Disclosure

Two different questions hide under the word, and separating them is what makes either answerable.

- **Artifact handling** is where the threat model document lives, and how much of it sits in a
  publishable tree. A standing policy, set once per repository.
- **Vulnerability disclosure** is whether a specific finding is told to the people it affects, and
  in how much detail. A decision per finding.

**Neither is derived from a finding's treatment.** This is the rule that matters most, and the
tempting shortcut is exactly the dangerous one: `mitigated` does not mean the detail is harmless,
because a control can be incomplete, misconfigured, or bypassable in a way the write-up reveals.
`transferred` leaves the vulnerability present and merely assigns the obligation elsewhere.
`avoided` closes one path and says nothing about the others the write-up maps.

## Artifact handling

A repository that is or may become public states its option in `docs/security/README.md`:

| Option | What it means |
| --- | --- |
| Private analysis | The threat model stays out of the publishable repository; only the structural description in `docs/architecture/security.md` is committed there |
| Reduced analysis | The model is committed with findings reduced to a class and an identifier, holding the exploitable detail privately |
| Reviewed publication | The full model is published after an explicit disclosure review, recorded with who approved it and what they considered |

Reviewed publication is the only option that permits exploit detail in a public repository, and it
requires a decision by a person, not a value in a table.

A repository that has not declared an option uses private analysis, because that is the only one
that is safe while undecided.

## Per finding

Where a finding is an actual vulnerability rather than a modelled risk, the question is how much to
tell whom. Four options:

| Option | What it means |
| --- | --- |
| No disclosure | Nothing is said outside the people already handling it |
| Private disclosure | Told only to the parties who have to act |
| Limited disclosure | Told publicly while withholding proof of concept code or other technical details |
| Full disclosure | Told publicly, with the technical detail |

Limited disclosure is the one most often misread. It buys time, not safety: withholding detail helps
for a few days at most, because the fix itself is a map for anyone reading the diff. Choose it to
stagger a rollout, never as a substitute for the fix.

## Silence is a decision, and it has a cost

The default is to hold detail, and a default is not a policy: a repository that never revisits it
ends up withholding from the people who most need to act. Disclose, at minimum privately to those
affected, when any of these is true:

- **Users have to do something.** Where the fix is not entirely in your hands, a user who does not
  know cannot act. The test is who must act: publication is unnecessary when whoever had to fix the
  vulnerable system has already fixed it, and necessary the moment somebody else has to.
- **It is already public, or being exploited.** Withholding then protects nobody and only delays the
  response. This forces a disclosure review and an incident response. It does not automatically
  publish the whole model, which is a separate and larger decision.
- **It will not be fixed.** If an issue is unresolvable, it is better that users know than not know.
  Security through obscurity is a weak defense.

One conflict in the sources is worth carrying rather than resolving, because the two views belong to
different roles. A system owner typically issues an advisory once a remediation has been developed
and deployed. A coordinator discloses after a fixed window regardless of whether a patch exists,
where the vendor does not engage. A repository here is the owner, so the first is the working
assumption, and the second is the reason an owner who stalls stops controlling the timeline.

## Where an undisclosed finding lives

Artifact handling decides where the document sits. This decides where the work sits, and it is the
rule most often broken by accident rather than by choice.

**An unfixed, undisclosed finding's exploitable detail never goes anywhere reachable from a public
repository: not on a branch, not in history, not in an issue or a pull request.** A deleted file is
still in the history, an unmerged branch is still public, and a pull request that explains the hole
is the disclosure.

The rule is platform-neutral and the mechanism is not. On GitHub the implementation is a private
security advisory to hold the record and a temporary private fork to hold the fix, merged when the
advisory publishes and not before. On any other host the requirement is the same shape: a private
place for the record, a private place for the fix, and neither reachable by a reader of the public
repository.
