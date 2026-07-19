# Job Sizing

Guidance for scoping tasks handed to the headless agent, drawn from actual
job outcomes on this repo — not generic project-management advice.

## What succeeds reliably

Prescriptive, single-target, ≤2-file jobs succeed cheaply. Name the exact
file(s) and the exact change up front — e.g. "in `docs/FEATURES.md` around
line 53, update the job lifecycle list to match `src/queue.rs`'s
`JobStatus` enum." The agent doesn't have to search for scope; it can go
straight to editing and verifying.

## What fails

Open-ended "find a good place to…" tasks burn the whole iteration budget on
exploration instead of editing. Worst case, the agent explores, changes
nothing, and the job resolves to `JobStatus::NoChanges` — decided by the
ground-truth diff, not by whatever the agent claims it did (see
`docs/DETERMINISM.md`). A job that "sounds done" in the transcript but
touched zero files is a failure, not a success.

## Watch for base drift on hot files

`src/agent/headless.rs` is this repo's most-edited file. If two jobs both
touch it, one can fork from an older base and silently revert the other's
change. Don't queue multiple jobs against the same heavily-edited file in
parallel; serialize them, or scope each job to a different file.

## What to do with a bigger task

Decompose it yourself before queuing, into a list of prescriptive,
single-target jobs — don't hand the agent one big vague job and hope it
self-decomposes. Each sub-job should read like the "what succeeds" example
above: one file (or two closely related ones), one named change, and enough
context (the current wording/line, the correct wording/enum, etc.) that the
agent can verify its own edit against a fact rather than a hunch.

## The gate checks compilation, not judgment

Even a well-scoped job should be checked by a human (or a second job) when
it touches prose, markdown, or anything the `cargo test` / `cargo clippy`
gate can't see — the gate proves the tree still builds, not that the
wording or scope decision was right.