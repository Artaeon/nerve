# How well does nerve actually build? — the measured record

An honest, evidence-based answer to "is the code nerve writes any good?", drawn
from driving it to build a real production app (**vollgebucht**, a German booking
SaaS) and, latterly, to fix **its own source**.

No vibes. Every row below is a job that ran, was reviewed by a human, and was
either merged or rejected. Pairs with [TEST-STATUS.md](TEST-STATUS.md) (what's
verified) and [DETERMINISM.md](DETERMINISM.md) (how reproducible it is).

**Method.** Each task is submitted to the 24/7 server, runs on its own
`nerve/job-<id>` branch, and is gated by the project's own type-check **and test
suite**. A human then reviews the branch, runs `tsc` + the tests independently,
and merges or rejects. Nothing below is "it looked right".

---

## The headline numbers (2026-07-14 → 15)

| | |
|---|---|
| Features merged into vollgebucht | **13** |
| Rejected / failed jobs | **4** (all caught before merge) |
| vollgebucht test suite | 191 → **222 passing** |
| Typical clean job | **3–24 iterations** |
| Human interventions on merged code | **1** (a nav line re-added by hand) |

---

## What predicts success — the single strongest finding

The task's **shape** predicts the outcome far better than its size.

| Task shape | Outcome | Iterations |
|---|---|---|
| **Additive / pure / new-file** (a pure function + tests, a new page) | ✅ Clean **first try**, every time | **3–21** |
| **Single-file prescriptive edit** (add a field, render a banner) | ✅ Reliable | 8–24 |
| **Mechanical repetition** (same edit across 10 files) | ✅ Reliable | ~19 |
| **Cross-cutting edit-existing, handed over whole** | ❌ **Failed** — stub or nothing | hit the 40 cap |
| **…the same task, decomposed** | ✅ **Succeeded** | 3–14 |

The decisive experiment: **DSGVO consent** and **customer↔record linking** each
failed *twice* as monolithic multi-file jobs (thrashing to the iteration cap).
Decomposed — consent collapsed to **one file** once we noticed the plumbing
already existed; linking split into a pure matcher + a thin wiring job — **both
succeeded immediately** (14, 3, and 13 iterations).

**The rule:** a task that fails whole almost always succeeds decomposed. Check
first whether the types/plumbing already exist — they often do, and the job
collapses to a single file.

---

## Is the code actually good? — yes, and specifically so

Not "compiles and passes tests" good. The details it gets right are the ones that
trip humans:

- **`summarizeRevenue`** — half-open month interval, null prices treated as 0, a
  zero-denominator guard on the no-show rate, stable top-5 sort. Pure, `now`
  injected, deterministic.
- **`summarizeCustomers`** — fills missing contact fields *without overwriting*
  existing ones; sorts recent-visit-first with no-visit customers last.
- **Reschedule action** — converts a `datetime-local` in the **studio's**
  timezone (via the existing `instantAt` helper, not naive `new Date()`),
  translates engine errors to German, redirects on conflict.
- **Customer find-or-create** — wrapped in try/catch so it *can never fail a
  booking*, falling back cleanly.
- **The Remotion commercial** — 370 lines, five scenes, and it honored the design
  law unprompted: **zero gradients, zero shadows**, only the brand palette.

It also matches the surrounding style — idiom, comment density, naming. It reads
like the codebase, not like generated code.

---

## Where it fails — honestly

1. **Cross-cutting edit-existing work done whole.** The one real weakness. It
   thrashes to the cap and commits a stub or nothing. *Mitigation: decompose
   (now automatable via `--decompose`).*
2. **It half-acts and stops.** A reschedule job once imported the function and
   defined the helpers but **rendered no form and called nothing** — and it
   type-checked, so the gate passed. *Mitigation: the INCOMPLETE flag, and a
   human who reads the diff.*
3. **It once committed a failing test** it had written. The gate was
   type-check-only at the time. *Fixed: the gate now runs the test suite too.*
4. **It cannot see.** No visual feedback loop — CSS/markup is verified by reading
   code, never by looking at the page. A human must eyeball the result.
5. **A green gate is not a finished feature.** The gate proves *correctness*, not
   *completeness*. Every one of the 4 failures above was caught by human review,
   not by the machine.

---

## What made the difference (in order of impact)

1. **Prescriptive specs.** Naming the exact file, the exact signature, the exact
   JSX, and an explicit acceptance criterion ("X must actually be CALLED") turned
   a stubbed job into a working one on re-issue. This is the highest-leverage
   lever by far.
2. **Decomposition.** Turned two repeat failures into successes.
3. **Tests in the verify gate.** Closed the "type-checks but is wrong" hole.
4. **Reading the code before believing the story.** The "wedge" that shaped weeks
   of architecture was a one-line bug (an unreset counter) — and the agent's
   supposed "confabulated tool limit" was it *accurately reporting nerve's own
   error message*. See TEST-STATUS.md.

---

## The efficiency footnote

Early jobs burned **29–40 iterations** on work needing ~5 — and because every
iteration re-sends the growing context, cost is roughly quadratic. It was enough
to **exhaust a claude.ai session quota mid-batch**. After the explore-nudge (act
after 8 tool-iterations with no edit) plus prescriptive specs: **3–24
iterations**. The context *system* was always lean; the *loop* was the waste.

---

## Verdict

**nerve writes production-grade code within a well-understood envelope**:
additive, pure, prescriptive, decomposed. It is genuinely reliable there — 13
merged features, 222 green tests, one hand-edit. Outside that envelope it fails
in *legible* ways (stubs, empty diffs) that a review catches.

Two disciplines are non-negotiable, and neither is a nerve limitation so much as
how you drive it:

- **Decompose cross-cutting work.**
- **A human reviews and runs the tests before merging.**

Held to those, it built a real, growing SaaS with very little waste.
