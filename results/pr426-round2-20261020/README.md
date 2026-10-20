# PR #426, second review round and upstream #429 (2026-10-20)

Runs on the 11-commit series (`ea23abef0`, on `main` `8b1318b80`) after the maintainer's second review, and the check of that series against upstream PR #429.

## Timeline

| When (UTC) | Event |
|---|---|
| 2026-10-19 06:27 | First review: five findings, rebuild of the series requested |
| 2026-10-19 19:48 | Second review: four findings resolved, one partial (a predicate after a point write in one request) |
| 2026-10-19 23:38 | Third review: all five findings resolved by static review, no new finding |
| 2026-10-20 02:30 | Upstream PR #429 merged. It closes #424 and #425 and settles #428 by a different design. 22 files overlap the series. Ten of the eleven commits conflict on the new `main` |

## Files

| File | Content |
|---|---|
| `ci-mirror-series-head.txt` | The steps of the CI Test workflow run locally on the series head: format, gates, `cargo deny`, clippy, the workspace tests and the three-node tests |
| `vector-timeout-on-upstream.txt` | The one timed-out test of that run, run alone on the series head and on upstream `main` |
| `batch-fixes-red-green.txt` | Red and green runs of the tests added for a request of several statements |
| `upstream-429-overlap-survey.txt` | Embedding survey of the hunks of #429 against the findings and commits of the series, cosine scores |

## Results

| Step | Result |
|---|---|
| Format, ten gate scripts, `cargo deny` | Pass |
| Workspace tests (22,778) | 22,777 pass. One test times out at the 120 s limit, `nodedb-vector` `search_allocations_do_not_scale_with_distance_count`. It times out on upstream `main` too, and the series changes no file of that crate |
| Three-node tests (523) | 523 pass. One passes on its second try |
| Clippy with `-D warnings`, workspace | Fails on an unused import in `nodedb-sql` (`search_cells.rs:32`, in `main` since 2026-09-18), and two `nonminimal_bool` findings in files the series does not touch |

The run is on x86_64 with rustc 1.96.1. The GitHub workflow runs on ARM64. A failure that exists only on ARM64 does not show here.

## State

The series is being rebuilt on the new `main`. Parts that #429 covers are dropped, and the gaps it leaves are kept with their tests. This directory records the run on the previous head.
