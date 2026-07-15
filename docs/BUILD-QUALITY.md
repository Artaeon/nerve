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
| Features merged into vollgebucht | **15** |
| Bugs nerve fixed in its OWN source | **4** (2 HIGH, 2 MEDIUM — all correct) |
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

## The hardest test: nerve fixing nerve

We pointed nerve at **its own source** — a Rust codebase — and asked it to fix two
**HIGH-severity** bugs found by an independent deep review. Its verify gate ran
`cargo check` **and nerve's own 2,028-test suite**.

| Bug | Result | Iterations |
|---|---|---|
| A failed `checkout -b` silently committed jobs to `main` | ✅ **Fixed correctly** | 16 |
| The decompose fallback re-ran a step the child had already executed | ✅ **Fixed correctly** | 16 |

Both merged after human review; both compile clean, pass 2,028 tests and clippy.

This is the strongest evidence in this document, because the work is unforgiving:
- For the branch bug it did exactly the right thing — replaced the inference
  ("`checkout -b` failed ⇒ branch exists") with an explicit
  `rev-parse --verify`, **preserved** the subtle requeue-keeps-commits behaviour,
  and added a hard `bail!` if HEAD isn't on the job branch.
- For the double-run bug it drew the correct boundary — a `StepExec` enum
  separating *never started* (safe to retry in-process) from *started and may
  have already edited the repo* (must **not** retry) — and added `kill_on_drop`
  so an abandoned child can't keep writing.

Its comments explain **why**, referencing the real failure mode, in the file's
existing voice. This is senior-level work on a codebase it had never "seen".

**But the same experiment exposed two real bugs in nerve that only this could
find** — because the gate ran nerve's tests on the machine hosting a live daemon
and a root shell:

1. **The test suite decapitated the running daemon.** `daemon.rs` tests called
   `remove_file(socket_path())` and `stop_daemon()` — on the **real**
   `~/.nerve/nerve.sock`. Every self-job's verify gate killed the very daemon
   running it; the process stayed up but was unreachable and the queue stranded.
   *Fixed (hermetic `*_at(path)` cores + tempdir sockets) and **proven**: the
   suite now runs on the server and the daemon survives.*
2. **A "security" test really ran `sudo apt-get install malware`** — as root, on
   every `cargo test`. It was named `..._sudo_blocked`, asserted **nothing**
   (`let _ = result;`), and its own comment admitted the command wasn't in the
   denylist. *Fixed: privileged system-wide installs are now denied (project-local
   `npm install` still allowed), and the test asserts the refusal.*

Neither is code nerve wrote — they're **latent bugs in nerve that dogfooding
surfaced**. That is the strongest argument for this practice.

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

## The most consistent failure — and it's cheap to plan around

Across **four** prescriptive single-file bug-fix jobs on nerve's own source, the
pattern was identical every time:

> **The code was correct. The tests were missing.** Each job burned its 40
> iterations getting the fix right, then stopped before writing the unit tests it
> was explicitly asked for.

The INCOMPLETE flag caught all four. The fix is not more prompting — it's
**budgeting**: give the code and the tests **separate jobs**, or accept that a
human writes the tests. (Which is arguably the right split anyway: a test written
by the same agent that wrote the code shares its blind spots. Every regression
test in this session was verified by *reverting the fix and watching it go red* —
a discipline the agent never performed on itself.)

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
