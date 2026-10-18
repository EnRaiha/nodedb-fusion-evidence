-- GRAPH RAG FUSION returns the surrogate number as node_id for a hit that is in no edge.
-- Build: origin/main d1e957217. Single node, default database.
CREATE COLLECTION ragf_names (id TEXT PRIMARY KEY, name TEXT);
CREATE VECTOR INDEX idx_ragf_names ON ragf_names (embedding) METRIC cosine DIM 2;
INSERT INTO ragf_names (id, embedding) VALUES ('lonely', ARRAY[1.0, 0.0]);  -- nearest the query, in no edge
INSERT INTO ragf_names (id, embedding) VALUES ('a', ARRAY[0.0, 1.0]);
INSERT INTO ragf_names (id, embedding) VALUES ('b', ARRAY[-1.0, 0.0]);
GRAPH INSERT EDGE IN 'ragf_names' FROM 'a' TO 'b' TYPE 'hop';

GRAPH RAG FUSION ON ragf_names QUERY ARRAY[1.0, 0.0] VECTOR_FIELD 'embedding'
  VECTOR_TOP_K 2 EXPANSION_DEPTH 1 EDGE_LABEL 'hop' FINAL_TOP_K 10;
-- Actual (the second row is the hit in no edge):
--   {"results":[
--     {"node_id":"a","rrf_score":0.0325,"vector_rank":1,"vector_distance":1.0,"hop_distance":0},
--     {"node_id":"1","rrf_score":0.0164,"vector_rank":0,"vector_distance":0.0,"hop_distance":null},
--     {"node_id":"b","rrf_score":0.0161,"vector_rank":null,"vector_distance":null,"hop_distance":1}], ...}
-- Expected: node_id "lonely" in place of "1", as SEARCH ... USING VECTOR names the same row.
