# PR #426 after upstream #429 (2026-10-20)

Upstream PR #429 (merged 2026-10-20 02:30 UTC) closes #424 and #425 with a different design. The 11-commit series of the earlier round was checked against it and cut to three commits.

## Method

The tests of the earlier series ran on upstream `main` after #429 with none of the series' source. A test that passes there is covered by #429, and its fix was dropped. A test that fails there marks a gap.

## Files

| File | Content |
|---|---|
| `series-tests-on-main-after-429.txt` | The series' tests on `main` (215b7505e): in-process 18 of 18, three-node 13 of 13, wire 39 pass and 10 fail |
| `ci-mirror-fdbe1c4ac.txt` | The CI Test workflow run locally on the three-commit head: fmt, gates, deny pass; workspace tests 22,783 of 22,784; cluster tests 515 of 515; clippy fails on one upstream unused import |
| `per-commit-checks.txt` | Each of the three commits checked out alone: fmt, `cargo check --tests`, the tests it adds, and the three `nodedb-client` clippy commands |

## Result

| Gap on `main` after #429 | Commit |
|---|---|
| A predicate write outside a block on a multi-shard collection answers `XX000`; a predicate after another write in one request misses the earlier write | run a request in which a predicate write follows another write as one implicit transaction |
| Two writes of one request that move one sum balance are not refused | refuse several writes of one sum balance before a route is chosen |
| A hybrid score alias named `doc_id` answers rows with an empty id | refuse a hybrid score alias named `doc_id` |

The one timed-out test, `nodedb-vector` `search_allocations_do_not_scale_with_distance_count`, also times out on upstream `main`. See the previous folder for that run. The runs are on x86_64. The GitHub workflow runs on ARM64, and its `Test Suite` job is skipped on the PR without the `run-ci` label.
