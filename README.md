# NodeDB: evidence for two defects on `origin/main` `d1e957217`

Evidence for two defects in [NodeDB-Lab/nodedb](https://github.com/NodeDB-Lab/nodedb) and for the PR that fixes them.

- Issue [#424](https://github.com/NodeDB-Lab/nodedb/issues/424): both defects.
- Issue [#425](https://github.com/NodeDB-Lab/nodedb/issues/425): the Calvin refusal is not logged on the node (see the last section).
- Draft PR [#426](https://github.com/NodeDB-Lab/nodedb/pull/426): the fixes and the tests named below. It holds the SQL that triggers each defect, the probe tests that map the sibling shapes, and the raw test output from `main` and from the fix.

## The defects

| | Defect | Wrong artifact |
|---|---|---|
| A | A transaction block, or a trigger body, that holds a predicate `UPDATE` or `DELETE` cannot commit through Calvin | `SQLSTATE XX000`: `calvin bulk predicate write reached overlay staging without ollp_predicted_surrogates` |
| B | `GRAPH RAG FUSION` names a hit that is in no edge by its surrogate number | `"node_id":"1"` where the row's primary key is `lonely` |

**A.** The Calvin stage runs a bulk plan over the surrogates the plan names and refuses a plan that names none. The autocommit path reads that set at recon and injects it. A transaction block buffers the plan with none. A write to an edge-bearing collection always plans as a predicate write, so a `DELETE` of any row there reaches the refusal alone, and an `UPDATE` reaches it beside an edge write. A plain autocommit `INSERT` fails the same way when its `BEFORE` or `SYNC AFTER` trigger body holds a predicate write. Nothing is applied.

**B.** The Data Plane never reads the catalog, so a fusion hit bound to no graph node falls back to its surrogate's decimal. `SEARCH ... USING VECTOR`, full-text and hybrid search translate that surrogate to the primary key. Fusion skips the step.

## Files

| Path | Content |
|---|---|
| `repro/txn-block-predicate-write.sql` | Defect A in a transaction block |
| `repro/trigger-body-predicate-write.sql` | Defect A through a trigger body |
| `repro/fusion-node-id.sql` | Defect B |
| `probes/txn_bulk_survey.rs` | 13 probe tests for sibling shapes. Each prints `PROBE <name>: <outcome>` and asserts nothing |
| `results/*-base.txt` | Test output on `origin/main` plus only the new tests |
| `results/*-fix.txt` | Test output with the fix |
| `results/calvin-refusal-log-excerpt.txt` | Server log of one failing `COMMIT` on `main`, with counts per level. See the next section |

## Results

Base is `origin/main` `d1e957217` with only the new tests. Fix is the PR branch.

| Test | Base | Fix |
|---|---|---|
| In-process `txn_bulk_prediction`, 9 tests that read the committed rows back | 8 fail, 1 passes (a plain collection, the control) | 9 pass |
| Wire `graph_rag_fusion`, 11 tests | 2 fail | 11 pass |

The base error in every failing in-process test is the participant's own, since a stage refusal now reaches the client:

```text
ERROR: internal error: calvin bulk predicate write reached overlay staging without ollp_predicted_surrogates for collection 'g_del'; a Calvin bulk plan must be recon-injected before dispatch (SQLSTATE XX000)
```

The failing fusion test on the base:

```text
no result named lonely; got: {"results":[{"node_id":"a",...},{"node_id":"1","rrf_score":0.0163...,"vector_rank":0,"vector_distance":0.0,"hop_distance":null},{"node_id":"b",...}], ...}
```

### Sibling probes

| Probe | Shape | Base | Fix |
|---|---|---|---|
| E1 | Predicate `UPDATE` in a block with an edge write | `XX000` | commits |
| E2 | Predicate `DELETE` in a block | `XX000` | commits |
| E8 | `UPDATE ... RETURNING` in a block | `XX000` | commits |
| E9 | Non-key `UPDATE` on two collections in a block | `XX000` | commits |
| I2 | Autocommit `INSERT`, `SYNC AFTER` trigger body with a predicate `UPDATE` | `XX000` | commits |
| I3 | Autocommit `INSERT`, `SYNC AFTER` trigger body with a predicate `DELETE` | `XX000` | commits |
| I4 | Autocommit `INSERT`, `BEFORE` trigger body with a predicate `UPDATE` | `XX000` | commits |
| E3 | `UPDATE ... FROM` in a block | commits | commits |
| E4, E5 | `MERGE` matched `UPDATE` and `DELETE` in a block | commit | commit |
| I1 | Autocommit `MERGE ... DELETE` into an edge-bearing collection | commits | commits |
| E6 | `INSERT ... SELECT` in a block | inconclusive | inconclusive |
| E7 | `DELETE ... IN (SELECT ...)` in a block | inconclusive | inconclusive |

E6 and E7 are probe errors: a duplicate key and unsupported syntax. They say nothing about the defect.

Every `E` probe adds an edge insert to the block, so the commit takes the Calvin path. The `I` probes run as autocommit statements.

A probe reports whether the statement commits. It does not read the rows back. The in-process tests do.

## What is not covered

- A concurrent change between a statement and `COMMIT`.
- Hand-run `psql` sessions. The SQL in `repro/` runs through the in-process pgwire test server, and the statements match the tests.
- The probe results come from an earlier head of the PR branch. Its library code is identical to the final head, which only adds tests.
- A release build. All runs use the debug test profile.

## Run the probes

Copy `probes/txn_bulk_survey.rs` to `nodedb/tests/inproc/cases/`, add `mod txn_bulk_survey;` to `nodedb/tests/inproc/cases/mod.rs`, and run:

```sh
cargo test -p nodedb --test inproc -- txn_bulk_survey --nocapture --test-threads=1
```

The in-process and wire tests named above are in the PR.

## Calvin refusal log

The server logs nothing about the refusal in defect A. `results/calvin-refusal-log-excerpt.txt` holds the log of one failing `COMMIT` on `origin/main` `d1e957217`.

- The run has 4,645 lines: 3,166 `INFO`, 1,360 `DEBUG`, 77 `TRACE`, 5 `WARN`.
- The five `WARN` lines are the cluster transport without authentication, the authorization lease at startup, and three shutdown drain warnings. None names the commit.
- `DEBUG` shows the stage start on vShards 543 and 607, then `discarding staged commit` for both, then a completion ack. No line says which vShard refused or why.
- The error text appears once, in the test's own panic line.

Capture method: the in-process test server installs a subscriber only when a test calls `nodedb_test_support::test_tracing::init()` and `RUST_LOG` is set. The capture adds that one call to the failing test and runs it with `RUST_LOG=trace` and `--nocapture`.

```sh
RUST_LOG=trace cargo test -p nodedb --test inproc -- a_delete_of_a_row_without_edges_in_an_edge_bearing_collection_commits --nocapture
```
