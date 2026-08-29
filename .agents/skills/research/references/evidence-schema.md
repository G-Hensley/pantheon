# Evidence Schema

What to capture per claim, and how to render it.

The point of a schema is that provenance becomes a field rather than a writing
habit. Fields get filled in or visibly do not. Prose can imply currency it does
not have.

## How much of this to actually do

**One record per load-bearing conclusion, not per sentence.**

A load-bearing conclusion is one the reader would act on, or one that would
change the recommendation if it were wrong. Supporting details group underneath
it and share its provenance; they do not each get their own record.

Full ceremony on every incidental claim is how this gets abandoned entirely, and
an abandoned schema protects nothing. If a claim does not feel worth a record,
either it is not load-bearing, or it is and you have found the reason to check it
properly.

The fields must appear in the answer. Provenance recorded privately and
summarised as "verified" is not provenance.

**Factor out what repeats.** State the retrieval date and the queries once for
the whole answer, then let each record carry only what differs. A reader should
not have to parse a mini audit log to reach a two-sentence answer. The JSON
below is the field contract, not a required output format; a compact table with
shared fields hoisted out is usually the better rendering.

## Fields

### Core, always

Six fields. If this feels like a lot for a claim, the claim is probably not
load-bearing and does not need a record at all.

| Field | Meaning |
|---|---|
| `claim` | One statement, small enough to be true or false on its own |
| `source_url` | The canonical location of the artifact actually read |
| `status` | Lifecycle position, from the closed set below |
| `published` | Publication or last-modified date of the source, or `unknown` |
| `retrieved` | When you fetched it, in this session |
| `confidence` | `high`, `medium`, `low`, justified by evidence quality |

### Extended, for consequential or currency claims only

| Field | When |
|---|---|
| `status_native` | The project's own word, verbatim, whenever it differs from the normalised value |
| `currency_pointer` | Any currency claim: the version index, release list, or current-version banner |
| `source_type` | `spec`, `docs`, `repo`, `advisory`, `api`, `release-notes`, `analysis` |
| `version` | The version, tag, or commit the claim is scoped to |
| `query` | The exact search, API call, or command used |
| `corroboration` | Independent evidence supporting the same claim |
| `conflicts` | Sources that disagree, and how |

`published: unknown` is a legitimate value and more honest than a guess. An
undated source is weaker evidence, and recording that is the point.

### Emitted records, when the set is written to a file

Everything above describes a record as working material behind an answer. A
record set **written to a file** is a different artifact with a stricter
contract, because something mechanical reads it: a delegating lead re-opening a
worker's sources, or `researchctl verify`.

Six additions. They are what separate evidence from prose, and a worker's
summary satisfies none of them.

| Field | Meaning |
|---|---|
| `lane_id` | The bounded assignment this record came from. Name your own when you did not delegate |
| `outcome` | `evidence`, `no-evidence-found`, `unretrievable`, or `out-of-scope` |
| `quote` | A verbatim span from the source, 25 to 300 characters, that the claim rests on |
| `observation` | What the source says |
| `inference` | What you concluded from it, which the source does not itself state |
| `coverage` | The surfaces actually reached. Required when the outcome is `no-evidence-found` |
| `retrieval_method` | The exact tool call, query, or URL sequence, so the lane is reproducible |

`outcome` is a closed set because silence is the failure a lead cannot detect.
A lane that found nothing and a lane that fell over read identically in prose
and are entirely different facts. A record whose outcome is `unretrievable` is
a useful record; it is not required to carry a claim or a quote.

`observation` and `inference` are separate **fields**, not separate prose
voices. A stylistic distinction does not survive a delegation boundary.

`quote` is the single most valuable addition. It makes re-opening cheap, since
the lead searches the page for the span rather than reading the document again,
and it is what makes a mechanical check possible at all.

Do not apply this section to a record rendered inline in an answer. Full
ceremony on every incidental claim is how a schema gets abandoned, and an
abandoned schema protects nothing.

### The `status` set

Five values, chosen to answer the question the reader actually has, which is
"should I use this one?" Earlier drafts listed `released`, `approved`, `GA`, and
`stable` side by side; those are not synonyms and are not even the same kind of
label, so the set below records **lifecycle position** and leaves the project's
own vocabulary to `status_native`.

| Value | Meaning |
|---|---|
| `normative` | The current authoritative version. This is the one to build against |
| `prerelease` | Draft, working draft, release candidate, preview, beta, bleeding edge. Real, published, and not normative |
| `superseded` | Was normative, no longer is |
| `deprecated` | Still present, actively discouraged |
| `eol` | Unsupported |

Keeping both matters. SLSA v1.2 is `normative` with `status_native: "Approved"`,
and its working draft is `prerelease` with `status_native: "working draft"`.
Flattening those into one word is how the two get confused.

## Record

The full field contract, for a consequential claim that warrants the extended
fields:

```json
{
  "claim": "Package <name> version <x.y.z> is affected by <advisory-id>.",
  "source_url": "https://<advisory-canonical-url>",
  "source_type": "advisory",
  "status": "normative",
  "status_native": "Published",
  "published": "2026-07-14",
  "retrieved": "2026-08-10",
  "currency_pointer": "https://<advisory-index-or-release-list>",
  "version": "<x.y.z>",
  "query": "osv query package=<name> version=<x.y.z>",
  "corroboration": ["https://<vendor-advisory-url>"],
  "conflicts": [],
  "confidence": "high"
}
```

This is the contract, not a required output format. For a routine claim, six
core fields rendered as one line is the whole record.

## Rendering into an answer

The record is working material. The reader gets prose with the load-bearing
fields visible inline.

**Good:**

> All retrieved 2026-08-10. Queries: `site:example.invalid/spec versions`.
>
> The current specification is **v1.2** (`normative`, project's own word
> "Approved", published 2025-11-24, [spec](https://example.invalid/spec),
> [version index](https://example.invalid/versions)). A working draft proposes
> two further tracks (`prerelease`, [draft](https://example.invalid/draft)) and
> is not normative. Checked: version index, default branch, releases.
>
> Recommend building against v1.2.

Shared fields are stated once. Status is named and separated from the project's
own vocabulary. The draft is present but visibly not the answer. What was
checked is stated, so the search is auditable. The recommendation is visibly a
recommendation.

**Bad:**

> The latest version is 1.2, though there is newer work adding more tracks.

No status, no dates, no links, and "newer work" quietly implies the draft is the
direction to follow.

## Checking a set of records

`researchctl verify <records.json>` runs the checks that do not need judgement:

- required fields present for the outcome the record claims;
- `published` present or explicitly `unknown`;
- `retrieved` inside the run window;
- `coverage` present whenever the outcome is `no-evidence-found`, which is what
  makes a negative claim auditable;
- `corroboration` entries differing in origin from the source and each other,
  because two pages on one host is one source with two URLs;
- a `normative` status on a date-versioned URL carrying a `currency_pointer` to
  a **different** document.

With `--fetch` it also opens each `source_url` and confirms the `quote` occurs
in it. Offline by default, so a clean result never comes from a run that
quietly skipped half its work.

The last check exists because of a defect this system produced about itself: a
research pass cited a specification at a dated URL as `normative` while the
project's own versioning page named a newer version. The rule that would have
caught it was loaded at the time and was not applied, which is the argument for
a check that runs over a rule that must be remembered.

**A pass is not a verification.** Confirming that a span occurs in a document
confirms it was copied rather than invented. It says nothing about whether the
span supports the claim beside it, and nothing about whether the claim is true.

## Confidence

Set from the evidence, not from familiarity.

| Level | Warrants it |
|---|---|
| `high` | Primary source, dated, version-scoped, and corroborated where consequential |
| `medium` | Primary source, but undated, indirect, or uncorroborated on a claim that deserves corroboration |
| `low` | Only secondary sources, or sources that disagree, or a gap you could not close |

A `low` confidence answer is a valid deliverable when it says what is missing and
what would resolve it. A `high` confidence answer that skipped retrieval is not
an answer at all.

## Conflicts

When sources disagree, record both and report both. State what each says, which
is more authoritative and why, and what would settle it.

Do not resolve a conflict by omission. A vendor's documentation contradicted by
an open issue in the vendor's own repository is a genuinely useful finding, and
usually more useful than whichever side you would have picked.
