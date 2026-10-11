# Source Policy

## Two standards for a second source

Cross-checking is not one rule. Applying the wrong one lets a single vendor's
two pages pass as independent corroboration.

| Claim type | Standard | Why |
|---|---|---|
| Currency ("this is the current version") | Two independent **artifacts**, normally both published by the project | Only the project can say what its own current release is. A third party repeating it adds nothing |
| Everything else consequential | Two independent **origins** | Here a single origin is the risk, and a vendor page quoted back by a blog is one origin, not two |

Do not use the currency standard to justify treating two pages from one vendor
as corroboration for a non-currency claim.


Which source is authoritative for which question, and in what order to try them.

This file names **where to look**, including canonical entry points, because a
source you cannot find is not a source. It deliberately stops short of API
paths, parameters, rate limits, and quotas, which decay fast enough that a stale
one is worse than none.

The dividing line: a project's home is stable for years, its API contract is
not. So the domains below are safe to record; confirm the exact request shape at
use time from the source's own documentation, or let a tool that tracks it
(`researchctl`, where available) own that contract for you.

## Source hierarchy

Strongest to weakest. Prefer the strongest source that actually answers the
question rather than the strongest source available.

1. **The artifact itself.** The specification, the source code, the API
   response, the commit, the tag.
2. **The project's own documentation**, at the version in question.
3. **The vendor's advisory or release notes.**
4. **A standards body or government source** (NIST, CISA, a CERT).
5. **A structured database** that aggregates the above (OSV, an advisory
   database), which is authoritative for aggregation but should be followed
   through to the underlying advisory for consequential claims.
6. **Well-sourced secondary analysis** that cites primary sources you can check.
7. **Everything else**, which is discovery material, not evidence.

## Never cite as evidence

- Search result snippets, including AI-generated search summaries.
- Undated blog posts, and dated ones where the date is the only claim to
  currency.
- Content farms, SEO listicles, and tutorial aggregators.
- Forum and Q&A answers without a version, date, and a link to something real.
- Another model's output, including your own earlier turns.
- Documentation for a different major version than the one asked about.

Any of these may legitimately point you at a real source. Follow the pointer,
then cite what it pointed at.

## Routing by domain

### Vulnerabilities and packages

Query the databases directly rather than searching prose about them.

| Need | Source | Entry point |
|---|---|---|
| Is this package version affected | OSV, aggregating across ecosystems | `osv.dev` |
| CVE detail and modification history | NVD | `nvd.nist.gov` |
| Ecosystem-reviewed advisories, richer context | GitHub Security Advisories | `github.com/advisories` |
| Is it actively exploited in the wild | CISA KEV | `cisa.gov`, known exploited vulnerabilities catalog |
| Likelihood of exploitation | EPSS, from FIRST | `first.org/epss` |
| Authoritative fix and affected range | The vendor's or maintainer's own advisory | The project itself |

**OSV and NVD are not interchangeable.** OSV is the right first stop for "is
this package at this version affected", because it maps advisories onto
ecosystems and version ranges. NVD is authoritative for CVE detail itself:
the record, its scoring, and its modification history. Asking either one the
other's question produces a confidently incomplete answer.

Order: identify the package and exact version, query OSV, follow through to the
underlying advisory, then check KEV for exploitation status. Use search only for
context, mitigations, and analysis after the facts are established.

A CVE identifier without an affected-version range is not an answer. "Is it
vulnerable" is always version-specific.

### Standards and frameworks

The hard part is usually resolving **which artifact** is meant, not finding a
version.

1. Resolve the specific project. "OWASP" is a foundation with many independently
   versioned projects (Top 10, API Security Top 10, GenAI/LLM, ASVS, MASVS, SAMM,
   Cheat Sheet Series). "The latest OWASP" is not a well-formed question.
2. Find that project's own current release, from the project, not from an
   article about it.
3. Determine release status explicitly. Many of these publish a working draft
   in the open alongside the current release.
4. Report the normative release, and the draft separately if one exists.

The same shape applies to SLSA (an approved specification and a working draft
coexist), NIST SSDF, and CIS Benchmarks.

**Never report a draft as "the latest" without naming it a draft.**

Beware a platform's own labels. A releases page may mark a rolling preview as
"latest" while the project's text says the previous stable release is still
normative. The project's word wins over the platform's badge.

Worked example: "What is the latest OWASP guidance on authentication?"

- **Wrong:** name a Top 10 edition from memory and summarise it.
- **Right:** OWASP is not one versioned artifact. Resolve which project governs
  the question, retrieve *that* project's current release and its status, and
  report its version rather than an "OWASP version" that does not exist. Report
  any newer draft separately.

### Cloud and infrastructure

Prefer vendor release notes and change feeds over articles. AWS, Azure, and
Google Cloud each publish structured change feeds. Google Cloud additionally
exposes release notes as a public dataset, which is queried with SQL rather than
read as a feed, so it is worth using for historical or cross-service questions
and not for a quick lookup.

For Kubernetes, use the release record and the project's own CVE feed rather
than distribution blogs.

For end-of-life and support windows, prefer the vendor's own support policy.
`endoflife.date` is the de facto aggregator and is genuinely useful for breadth
and for working out what to check. It is community-maintained, carries no
guarantee, and derives much of its data from vendor pages, so treat it as a
pointer and confirm anything consequential against the vendor. Naming it here is
deliberate: an agent told only to "find an EOL aggregator" will search and land
in SEO content.

### Library, framework, and API behavior

1. A documentation retrieval tool such as Context7, which returns
   version-specific documentation rather than whatever the model absorbed.
2. The project's official documentation for the exact version in question.
3. The repository: releases, changelog, tags, and the source itself.

Version-pin everything. "How do I configure X" has a different answer per major
version, and the most confident wrong answers in this domain come from applying
current syntax to an older release or the reverse.

For "when did this change" or "is this fixed in version N", go to the repository.
Releases, changelogs, merge commits, and issues answer that precisely; articles
do not.

### Misbehaving software

When the question is "why is this software misbehaving", search for an existing
diagnosis first. Check the project's issue tracker and the web *before*
investigating, not after testing several hypotheses. Measuring your way to a
conclusion someone already published is competent and wasted. A closed issue is
still a finding: "closed as not planned" means known, unfixed upstream, and to
be designed around.

## When a fetch is not a read

A fetch that returned bytes has not necessarily returned the source. Treat each
of these as `unretrievable`, never as evidence:

- A bot challenge or interstitial ("checking your browser", "attention
  required"), which often arrives with a success status.
- A login wall, paywall, or cookie gate standing in for the content.
- A JavaScript shell whose body is empty until a script runs.
- A body that is empty or visibly truncated before the part you need.
- A search engine's snippet or summary returned in place of the page.

Before recording it as `unretrievable`, try **one** other canonical route to the
same artifact: the raw file in the repository, the repository or vendor API,
the release record, or a PDF or plain-text rendition the project publishes. Do
not substitute an archived snapshot as evidence of what is current: an archive
proves what a page once said, not what it says now.

An empty search is a different case. It shows only that these queries on these
surfaces returned nothing. Say which queries and surfaces, rather than claiming
nothing exists.

## Following through

For a consequential claim, an aggregator is a starting point and not the end.
OSV tells you an advisory exists; the advisory tells you what was actually
affected and fixed. Context7 tells you what the documentation says; the
documentation at that version is what you cite.

The rule of thumb: cite the layer that would change if the fact changed.
