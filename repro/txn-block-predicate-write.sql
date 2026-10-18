-- A transaction block whose COMMIT takes the Calvin path fails on a predicate DELETE.
-- Build: origin/main d1e957217. Single node, default database.
-- The collection holds one edge, so every write to it plans as a predicate write.
CREATE COLLECTION g;
UPSERT INTO g { id: 'd0', title: 't0' };
UPSERT INTO g { id: 'd1', title: 't1' };
UPSERT INTO g { id: 'd2', title: 't2' };
UPSERT INTO g { id: 'd3', title: 't3' };
GRAPH INSERT EDGE IN 'g' FROM 'd0' TO 'd1' TYPE 'l';

BEGIN;
DELETE FROM g WHERE id = 'd3';
COMMIT;
-- Actual:
--   ERROR: internal error: calvin bulk predicate write reached overlay staging without
--   ollp_predicted_surrogates for collection 'g'; a Calvin bulk plan must be recon-injected
--   before dispatch (SQLSTATE XX000)
-- Expected: COMMIT succeeds. Row d3 is gone and the edge d0 -> d1 stays.
