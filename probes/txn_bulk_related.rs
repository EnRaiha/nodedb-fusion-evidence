// Related-area probes: predicate writes in other engines, procedures and an async trigger body,
// in blocks that also write an edge so the commit takes the Calvin path. Print only. Not for commit.

use std::time::{Duration, Instant};

use nodedb_test_support::pgwire_harness::TestServer;

async fn edge_bearing_g(server: &TestServer) {
    server.exec("CREATE COLLECTION g").await.expect("create g");
    for i in 0..4 {
        server
            .exec(&format!("UPSERT INTO g {{ id: 'd{i}', title: 't{i}' }}"))
            .await
            .expect("seed g");
    }
    server
        .exec("GRAPH INSERT EDGE IN 'g' FROM 'd0' TO 'd1' TYPE 'l'")
        .await
        .expect("seed edge");
}

fn res<T: std::fmt::Debug, E: std::fmt::Debug>(r: Result<T, E>) -> String {
    match r {
        Ok(_) => "ok".to_owned(),
        Err(e) => format!("ERR {e:?}").chars().take(230).collect(),
    }
}

async fn show(server: &TestServer, sql: &str) -> String {
    match server.query_rows(sql).await {
        Ok(mut r) => {
            r.sort();
            r.iter().map(|x| x.join("=")).collect::<Vec<_>>().join(" ")
        }
        Err(e) => format!("READ-ERR {}", e.chars().take(120).collect::<String>()),
    }
}

/// `setup` statements, then a block: edge insert, `stmt`, COMMIT. Prints the outcome and `view`.
async fn in_block(label: &str, setup: &[&str], stmt: &str, view: &str) {
    let server = TestServer::start().await;
    edge_bearing_g(&server).await;
    for s in setup {
        server.exec(s).await.unwrap_or_else(|e| panic!("setup {s}: {e:?}"));
    }
    server.exec("BEGIN").await.unwrap();
    server
        .exec("GRAPH INSERT EDGE IN 'g' FROM 'd2' TO 'd3' TYPE 'l'")
        .await
        .unwrap();
    let st = res(server.exec(stmt).await);
    let commit = res(server.exec("COMMIT").await);
    println!("REL {label}: stmt {st}; COMMIT {commit}; {view} -> {}", show(&server, view).await);
}

const KV: &str = "CREATE COLLECTION kv1 (key TEXT PRIMARY KEY, n INT) WITH (engine='kv')";
const KV_ROWS: &str = "INSERT INTO kv1 (key, n) VALUES ('a', 1), ('b', 2), ('c', 1)";
const COL: &str = "CREATE COLLECTION col1 COLUMNS (id TEXT PRIMARY KEY, v INTEGER) WITH (engine='columnar')";
const COL_ROWS: &str = "INSERT INTO col1 (id, v) VALUES ('a', 1), ('b', 2), ('c', 1)";
const VP: &str = "CREATE COLLECTION vp (id STRING PRIMARY KEY, vec VECTOR(3), owner STRING) WITH (engine='vector', primary='vector', vector_field='vec', dim=3, payload_indexes=['owner'])";
const VP_ROWS: [&str; 3] = [
    "INSERT INTO vp (id, vec, owner) VALUES ('v1', ARRAY[1.0, 0.0, 0.0], 'alice')",
    "INSERT INTO vp (id, vec, owner) VALUES ('v2', ARRAY[0.0, 1.0, 0.0], 'bob')",
    "INSERT INTO vp (id, vec, owner) VALUES ('v3', ARRAY[0.0, 0.0, 1.0], 'bob')",
];

#[tokio::test]
async fn r1_kv_predicate_update() {
    in_block("R1 KV predicate UPDATE", &[KV, KV_ROWS], "UPDATE kv1 SET n = 9 WHERE n = 1", "SELECT key, n FROM kv1").await;
}

#[tokio::test]
async fn r2_kv_predicate_delete() {
    in_block("R2 KV predicate DELETE", &[KV, KV_ROWS], "DELETE FROM kv1 WHERE n = 1", "SELECT key, n FROM kv1").await;
}

#[tokio::test]
async fn r3_columnar_predicate_update() {
    in_block("R3 columnar predicate UPDATE", &[COL, COL_ROWS], "UPDATE col1 SET v = 9 WHERE v = 1", "SELECT id, v FROM col1").await;
}

#[tokio::test]
async fn r4_columnar_predicate_delete() {
    in_block("R4 columnar predicate DELETE", &[COL, COL_ROWS], "DELETE FROM col1 WHERE v = 1", "SELECT id, v FROM col1").await;
}

#[tokio::test]
async fn r5_vector_primary_predicate_update() {
    in_block(
        "R5 vector-primary predicate UPDATE",
        &[VP, VP_ROWS[0], VP_ROWS[1], VP_ROWS[2]],
        "UPDATE vp SET owner = 'dave' WHERE owner = 'bob'",
        "SELECT id, owner FROM vp",
    )
    .await;
}

#[tokio::test]
async fn r6_vector_primary_predicate_delete() {
    in_block(
        "R6 vector-primary predicate DELETE",
        &[VP, VP_ROWS[0], VP_ROWS[1], VP_ROWS[2]],
        "DELETE FROM vp WHERE owner = 'bob'",
        "SELECT id, owner FROM vp",
    )
    .await;
}

/// A procedure body holds a predicate write on the edge-bearing collection.
async fn procedure(label: &str, body: &str) {
    let server = TestServer::start().await;
    edge_bearing_g(&server).await;
    let created = res(server.exec(&format!("CREATE PROCEDURE pr() AS BEGIN {body} END")).await);
    let called = res(server.exec("CALL pr()").await);
    println!("REL {label}: CREATE {created}; CALL {called}; g -> {}", show(&server, "SELECT id, title FROM g").await);
}

#[tokio::test]
async fn r7_procedure_predicate_delete() {
    procedure("R7 procedure: predicate DELETE", "DELETE FROM g WHERE title = 't3';").await;
}

#[tokio::test]
async fn r8_procedure_two_predicate_writes() {
    procedure(
        "R8 procedure: predicate UPDATE then DELETE",
        "UPDATE g SET title = 'p' WHERE title = 't2'; DELETE FROM g WHERE title = 't3';",
    )
    .await;
}

/// An ASYNC AFTER trigger runs its body on the Event Plane after the client's INSERT commits.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn r9_async_trigger_predicate_update() {
    let server = TestServer::start().await;
    edge_bearing_g(&server).await;
    server
        .exec("CREATE COLLECTION c (id TEXT PRIMARY KEY, v INT) WITH (engine='document_strict')")
        .await
        .unwrap();
    server
        .exec("CREATE TRIGGER async_bump AFTER INSERT ON c FOR EACH ROW BEGIN UPDATE g SET title = 'async' WHERE title = 't2'; END")
        .await
        .unwrap();
    let insert = res(server.exec("INSERT INTO c (id, v) VALUES ('c1', 1)").await);
    let t = Instant::now();
    let mut seen = false;
    while t.elapsed() < Duration::from_secs(10) {
        if show(&server, "SELECT id, title FROM g").await.contains("d2=async") {
            seen = true;
            break;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    println!(
        "REL R9 async trigger body: INSERT {insert}; body applied {} ({} ms); g -> {}",
        seen,
        t.elapsed().as_millis(),
        show(&server, "SELECT id, title FROM g").await
    );
}
