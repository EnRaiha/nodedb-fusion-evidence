// Adversarial probes for the predicate-write fix. Each prints `ADV <label>: ...` and asserts nothing.
// Not for commit.

use std::time::{Duration, Instant};

use nodedb_test_support::pgwire_harness::TestServer;

async fn seed(server: &TestServer, name: &str, edge: bool) {
    server.exec(&format!("CREATE COLLECTION {name}")).await.expect("create");
    for i in 0..4 {
        server
            .exec(&format!("UPSERT INTO {name} {{ id: 'd{i}', title: 't{i}' }}"))
            .await
            .expect("seed");
    }
    if edge {
        server
            .exec(&format!("GRAPH INSERT EDGE IN '{name}' FROM 'd0' TO 'd1' TYPE 'l'"))
            .await
            .expect("edge");
    }
}

async fn rows(server: &TestServer, name: &str) -> String {
    let mut r = server
        .query_rows(&format!("SELECT id, title FROM {name}"))
        .await
        .unwrap_or_default();
    r.sort();
    r.iter().map(|x| x.join("=")).collect::<Vec<_>>().join(" ")
}

fn res<T: std::fmt::Debug, E: std::fmt::Debug>(r: Result<T, E>) -> String {
    match r {
        Ok(_) => "ok".to_owned(),
        Err(e) => format!("ERR {e:?}").chars().take(220).collect(),
    }
}

/// B changes a row A's statement matched, between A's statement and A's COMMIT.
async fn stale(edge: bool, label: &str) {
    let server = TestServer::start().await;
    let x = if edge { "g" } else { "p" };
    seed(&server, x, edge).await;
    let (b, _h) = server.connect_as("nodedb", "").await.expect("second session");
    server.exec("BEGIN").await.unwrap();
    server
        .exec(&format!("UPDATE {x} SET title = 'A' WHERE title = 't2'"))
        .await
        .unwrap();
    if edge {
        server
            .exec(&format!("GRAPH INSERT EDGE IN '{x}' FROM 'd2' TO 'd3' TYPE 'l'"))
            .await
            .unwrap();
    }
    let b_res = tokio::time::timeout(
        Duration::from_secs(8),
        b.simple_query(&format!("UPDATE {x} SET title = 'B' WHERE id = 'd2'")),
    )
    .await
    .map(res)
    .unwrap_or_else(|_| "TIMEOUT (B waits on A)".to_owned());
    let commit = server.exec("COMMIT").await;
    println!("ADV {label}: B update {b_res}; A COMMIT {}; rows {}", res(commit), rows(&server, x).await);
}

/// B makes a new row match A's predicate, between A's statement and A's COMMIT.
async fn phantom(edge: bool, label: &str) {
    let server = TestServer::start().await;
    let x = if edge { "g" } else { "p" };
    seed(&server, x, edge).await;
    let (b, _h) = server.connect_as("nodedb", "").await.expect("second session");
    server.exec("BEGIN").await.unwrap();
    server
        .exec(&format!("UPDATE {x} SET title = 'A' WHERE title = 't2'"))
        .await
        .unwrap();
    if edge {
        server
            .exec(&format!("GRAPH INSERT EDGE IN '{x}' FROM 'd2' TO 'd3' TYPE 'l'"))
            .await
            .unwrap();
    }
    let b_res = tokio::time::timeout(
        Duration::from_secs(8),
        b.simple_query(&format!("UPDATE {x} SET title = 't2' WHERE id = 'd1'")),
    )
    .await
    .map(res)
    .unwrap_or_else(|_| "TIMEOUT (B waits on A)".to_owned());
    let commit = server.exec("COMMIT").await;
    println!("ADV {label}: B update {b_res}; A COMMIT {}; rows {}", res(commit), rows(&server, x).await);
}

#[tokio::test]
async fn s1_stale_set_calvin() {
    stale(true, "S1 stale set, Calvin commit").await;
}

#[tokio::test]
async fn s2_stale_set_single_shard() {
    stale(false, "S2 stale set, single-shard commit (control)").await;
}

#[tokio::test]
async fn s3_phantom_calvin() {
    phantom(true, "S3 new matching row, Calvin commit").await;
}

#[tokio::test]
async fn s4_phantom_single_shard() {
    phantom(false, "S4 new matching row, single-shard commit (control)").await;
}

/// Three predicate writes in one block, each seeing the one before.
#[tokio::test]
async fn c1_chain_in_one_block() {
    let server = TestServer::start().await;
    seed(&server, "g", true).await;
    server.exec("BEGIN").await.unwrap();
    server
        .exec("GRAPH INSERT EDGE IN 'g' FROM 'd1' TO 'd3' TYPE 'l'")
        .await
        .unwrap();
    let mut steps = Vec::new();
    for sql in [
        "UPDATE g SET title = 'x' WHERE title = 't1'",
        "UPDATE g SET title = 'y' WHERE title = 'x'",
        "DELETE FROM g WHERE title = 't2'",
    ] {
        steps.push(res(server.exec(sql).await));
    }
    let commit = server.exec("COMMIT").await;
    println!("ADV C1 chain: steps {steps:?}; COMMIT {}; rows {}", res(commit), rows(&server, "g").await);
}

/// An empty match set beside an edge write.
#[tokio::test]
async fn c2_empty_set() {
    let server = TestServer::start().await;
    seed(&server, "g", true).await;
    server.exec("BEGIN").await.unwrap();
    server.exec("UPDATE g SET title = 'z' WHERE title = 'none'").await.unwrap();
    server
        .exec("GRAPH INSERT EDGE IN 'g' FROM 'd2' TO 'd3' TYPE 'l'")
        .await
        .unwrap();
    println!("ADV C2 empty set: COMMIT {}; rows {}", res(server.exec("COMMIT").await), rows(&server, "g").await);
}

async fn big(edge: bool, n: usize) {
    let server = TestServer::start().await;
    let x = if edge { "gb" } else { "pb" };
    server.exec(&format!("CREATE COLLECTION {x}")).await.unwrap();
    for i in 0..n {
        server
            .exec(&format!("UPSERT INTO {x} {{ id: 'r{i}', title: 'old' }}"))
            .await
            .unwrap();
    }
    if edge {
        server
            .exec(&format!("GRAPH INSERT EDGE IN '{x}' FROM 'r0' TO 'r1' TYPE 'l'"))
            .await
            .unwrap();
    }
    server.exec("BEGIN").await.unwrap();
    let t = Instant::now();
    let upd = server
        .exec(&format!("UPDATE {x} SET title = 'new' WHERE title = 'old'"))
        .await;
    let t_upd = t.elapsed();
    if edge {
        server
            .exec(&format!("GRAPH INSERT EDGE IN '{x}' FROM 'r2' TO 'r3' TYPE 'l'"))
            .await
            .unwrap();
    }
    let t = Instant::now();
    let commit = server.exec("COMMIT").await;
    let t_commit = t.elapsed();
    let changed = server
        .query_text(&format!("SELECT id FROM {x} WHERE title = 'new'"))
        .await
        .map(|v| v.len())
        .unwrap_or(0);
    println!(
        "ADV L big n={n} edge={edge}: UPDATE {} in {} ms; COMMIT {} in {} ms; rows now 'new': {changed}",
        res(upd),
        t_upd.as_millis(),
        res(commit),
        t_commit.as_millis()
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn l1_large_set_calvin() {
    big(true, 3000).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn l2_large_set_single_shard() {
    big(false, 3000).await;
}
