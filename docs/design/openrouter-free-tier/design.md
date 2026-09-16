# OpenRouter free tier

Owned by #53. That issue is authoritative for scope, acceptance and status; this document is
the reasoning it links to rather than restates.

Why detecting a free model by zero pricing is wrong, why the quota is account-wide rather than per session, and what the launch guard already enforces.

Moved here from `BACKLOG.md` on 2026-09-16 under `orion:SPEC-0001` FR-009, which reduces that file
to one line per item. The text below is unchanged from the section titled "Guardrail: OpenCode sessions must stay on free OpenRouter models", so an issue
citing that section by name is citing this. It is a live design argument, not an archived one: the
work it describes has not been done.

---

## Guardrail: OpenCode sessions must stay on free OpenRouter models

OpenRouter is configured with a real account, so an OpenCode pane can select a
paid model and silently spend money. Pantheon now refuses a paid opencode model
at launch (`is_paid_openrouter_model`, `src-tauri/src/lib.rs:173-184`, enforced
at 1014-1024), but it still cannot see what a pane costs and the account-side
budget remains unenforced.

The naive implementation is wrong in a specific way worth writing down:

**Do not detect "free" by checking that prompt and completion pricing are
zero.** `MODEL-GUIDE.md` records that a model can price prompt and completion at
zero while still charging per request, per generated image, or per audio clip.
The documented signal is the `:free` suffix on the model id, plus the
`openrouter/free` meta-id which selects from the live free pool.

Also account for:

- **Free quota is account-wide**, not per session: 20 requests/minute, and 50
  requests/day until $10 of lifetime credit has been purchased, 1,000/day after.
  A fan-out across several OpenCode panes shares one budget and can exhaust the
  daily allowance quickly. An orchestrator that spawns sessions needs to know
  this before it spawns them.
- **`openrouter/free` does not promise a stable model identity.** It selects a
  compatible model per request, so two calls can land on different models. Pin
  an explicit `:free` id where reproducibility matters.
- **Free is explicitly not a reliability tier.** The guide notes free
  availability and latency vary. This is almost certainly why dispatches to
  OpenCode panes ran long often enough to expose the discarded-result bug fixed
  in `fix/late-task-completion`; the two issues share a root cause in tier
  choice.
- **It is still hosted inference.** Free does not mean private. Do not route
  work over private code or credentials to this tier without checking the
  selected provider's data policy.

Enforcement point is undecided and matters: Pantheon can only realistically
constrain what it launches, so this may belong in OpenCode's own config
(generated from `.agents/`, per the toolkit's sync) rather than in Pantheon. If
Pantheon enforces it, it needs a way to observe the model actually in use, which
it does not have today.

**Partly done, 2026-08-11.** `~/.config/opencode/opencode.json` now pins both
`model` and `small_model` to `openrouter/openrouter/free`, the Free Models
Router, with `max_price` zeros and `allow_fallbacks: false`.

The router is a better guard than pinning one `:free` model, for a reason worth
keeping: it cannot drift to a paid model, and it survives a model leaving the
free pool. That is not hypothetical, `inclusionai/ling-3.0-flash:free` had
already disappeared between the MODEL-GUIDE snapshot and the live catalogue.
Its tradeoff is no stable model identity between requests.

Care is needed with the sibling routers. `openrouter/free` prices prompt and
completion at 0, but `openrouter/auto`, `openrouter/fusion`, and
`openrouter/pareto-code` all report `-1`, meaning variable and billable. Pinning
the wrong router looks equally tidy and spends money.

**Launch-time enforcement, 2026-09-04.** Pantheon now refuses to start an
opencode pane without an explicit model, and refuses any `openrouter/*` id that
is not `openrouter/free`, `openrouter/openrouter/free`, or a `:free` id, compared
case-insensitively (`opencode_model_guard` in `src-tauri/src/lib.rs`, with the
launcher marking the field required). The missing-model refusal is the point:
without `-m`, opencode falls back to its config's `model` and then to whatever
model it used last, and Pantheon can see neither. The 2026-08-11 config pin is
no longer in force on the development machine (the global config carries no
top-level `model` as of this date), which is exactly the case the launch guard
covers.

Still outstanding, and still the only real guarantee: the account-side state
(zero balance, auto top-up off, payment method removed, no BYOK keys). Config
is declared intent; the balance is the enforcement.
