# Real-server end-to-end results

Two debug server binaries, built from `origin/main` `d1e957217` (base) and from the PR branch (fix), each started on free ports with a fresh data directory on disk and driven by psycopg2 over pgwire. Each statement is the SQL in `repro/`, sent one statement at a time. The raw reports are `results/e2e-report.json` and `results/e2e-b-report.json`.

## Statements

| Scenario | Base | Fix | Rows after, with the fix |
|---|---|---|---|
| A1 block DELETE | ok; ok; ERR internal error: calvin bulk predicate write reached overlay staging without ollp_predicted_surrogates for collection 'a1'; a C | ok; ok; ok | d0 t0, d1 t1, d2 t2 |
| A2 driver-managed block | ok; ok; commit ERR internal error: calvin bulk predicate write reached overlay staging without ollp_predicted_surrogates for collection 'a | ok; ok; commit ok | d0 t0, d1 t1, d2 drv, d3 t3 |
| B1 SYNC AFTER trigger body | ok; ERR internal error: calvin bulk predicate write reached overlay staging without ollp_predicted_surrogates for collection 'b_sync_g | ok; ok | d0 t0, d1 t1, d2 trig, d3 t3 |
| B2 BEFORE trigger body | ok; ERR internal error: calvin bulk predicate write reached overlay staging without ollp_predicted_surrogates for collection 'b_before | ok; ok | d0 t0, d1 t1, d2 trig, d3 t3 |
| B3 SYNC AFTER, DELETE body trigger body | ok; ERR internal error: calvin bulk predicate write reached overlay staging without ollp_predicted_surrogates for collection 'b_syncde | ok; ok | d0 t0, d1 t1, d2 t2 |

## GRAPH RAG FUSION

Hit in no edge. Base `node_id`s: `['a', '19', 'b']`. Fix: `['a', 'lonely', 'b']`.

## Durability (fix)

A predicate `DELETE` in a block on an edge-bearing collection commits, then the server restarts gracefully, then a second block commits, then the server is killed with `SIGKILL` and started again.

- After the graceful restart: `['d0 t0', 'd1 t1', 'd2 t2']`
- After the `SIGKILL`: `['d0 t0', 'd1 t1']` (the second delete survived too)
- A plain single-shard block, same run: `['d0 t0', 'd1 plain-commit', 'd2 before-kill', 'd3 t3']`

## Cost of the statement-time read

A 3,000-row `UPDATE` in a block on a plain collection, three repetitions each, debug binaries.

| | Statement, s | Commit, s |
|---|---|---|
| Base | [0.206, 0.203, 0.203] (median 0.203) | [14.011, 13.353, 13.731] (median 13.731) |
| Fix | [0.25, 0.251, 0.248] (median 0.25) | [13.211, 12.582, 12.459] (median 12.582) |

The fix adds about 47 ms to the statement. The commit times differ by less than the spread inside each run.
