-- A plain autocommit INSERT fails because its trigger body holds a predicate UPDATE.
-- Build: origin/main d1e957217. Same collection g as in txn-block-predicate-write.sql.
CREATE COLLECTION g;
UPSERT INTO g { id: 'd0', title: 't0' };
UPSERT INTO g { id: 'd1', title: 't1' };
UPSERT INTO g { id: 'd2', title: 't2' };
UPSERT INTO g { id: 'd3', title: 't3' };
GRAPH INSERT EDGE IN 'g' FROM 'd0' TO 'd1' TYPE 'l';

CREATE COLLECTION c (id TEXT PRIMARY KEY, v INT) WITH (engine='document_strict');
CREATE SYNC TRIGGER bump AFTER INSERT ON c FOR EACH ROW
  BEGIN UPDATE g SET title = 'trig' WHERE title = 't2'; END;

INSERT INTO c (id, v) VALUES ('c1', 1);
-- Actual: the same XX000 error as in txn-block-predicate-write.sql.
-- Expected: the INSERT commits and row d2 of g has title 'trig'.
-- A BEFORE INSERT trigger with the same body fails the same way.
