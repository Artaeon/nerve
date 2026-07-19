# Nerve memory DB — build spec (draft, 2026-07-19)

## Why (the north star, in the user's words)
Nerve must not forget, be context-efficient, log/document everything, hold principles + the plan, and DO it — with a real database. Storage never failed us; retrieval did. This spec is written to be executed AFTER the read-path fix (always_on_context in headless) is deployed and measured.

## Decision already made (2026-07-17, held)
- **SQLite, embedded, one file, OUTSIDE the repo**: `~/.nerve/projects/<workspace-hash>/memory.db`.
  - Outside the tree ⇒ `git clean`/`reset --hard` can never wipe it (kills bug class #22/#27 by construction).
  - Embedded ⇒ no server dependency; nerve stays a binary + daemon. Postgres is the wrong trade for a single-writer sequential worker.
- **Keep the split**: curated law (`memory.md`, `design.md`, `brief.md`) stays as tracked, human-editable markdown IN the repo (review artifact, travels with branches). The DB holds the APPEND-ONLY machine record + link graph + FTS index.
- `rusqlite` (bundled feature) becomes a new dependency.

## Schema (v1)
```sql
CREATE TABLE schema_version (version INTEGER NOT NULL);
CREATE TABLE entries (
  id INTEGER PRIMARY KEY,
  kind TEXT NOT NULL CHECK (kind IN ('principle','decision','architecture','activity','journal','fact','task')),
  title TEXT NOT NULL,
  body TEXT NOT NULL DEFAULT '',
  created_at TEXT NOT NULL,          -- RFC3339
  source TEXT NOT NULL DEFAULT ''    -- e.g. job id, 'tui', file of origin
);
CREATE TABLE links (
  from_id INTEGER NOT NULL REFERENCES entries(id),
  to_id   INTEGER NOT NULL REFERENCES entries(id),
  rel     TEXT NOT NULL,             -- 'follows', 'changes', 'caused-by', ...
  PRIMARY KEY (from_id, to_id, rel)
);
CREATE VIRTUAL TABLE entries_fts USING fts5(title, body, content='entries', content_rowid='id');
-- plus the two triggers keeping entries_fts in sync on INSERT/UPDATE/DELETE
```

## The sequencing that makes it SAFE (do not reorder)
`project_context::build(store, opts)` is already the single context path both TUI and worker use (unified 2026-07-16, commit 586794a era). Therefore the DB is a **substrate swap behind ProjectStore**, invisible to every caller:
1. Step 1 — storage module: `src/memory_db.rs`, open/create + schema + version table, pure functions, full tests (tempdir). No callers yet? NO — see the clippy rule: dead code fails nerve's own gate. So step 1 must land WITH step 2's first caller.
2. Step 2 — dual-write: `ProjectStore::record_activity_full` / `record_decision` / journal writes also insert into the DB (JSONL stays for one release as fallback). First caller arrives with the module.
3. Step 3 — import-on-first-open: migrate existing activity.jsonl / journal.jsonl / decisions.jsonl into entries (idempotent — track imported file offsets or hash).
4. Step 4 — read path: `memory_recall::collect_entries` gains an FTS-backed source (replacing/augmenting the bullet-only markdown scan — FTS indexes PROSE, which the markdown scan structurally cannot). BM25 ranking stays; FTS candidates feed it.
5. Step 5 — links: decisions link to the principles they follow; activity links to the tasks it advanced. Recall follows one hop when relevance is high.
6. Step 6 — measure, then retire JSONL writes.

## Job decomposition (each step = one --decompose-friendly unit)
Every step: "change only what is needed; do not rewrite files; keep the diff small; add tests; clippy+fmt clean" (the gate enforces it).

## Acceptance test (do NOT declare victory without it)
- `recall` calls per job > 0 (was 0/2,362).
- read_file+search_code per job DOWN from the 797/352-per-3-days baseline.
- Kill a job mid-run, `git clean -fdx` the repo: the DB still holds the record.
- A prose paragraph written to memory.md is findable via recall (FTS), not just bullets.
