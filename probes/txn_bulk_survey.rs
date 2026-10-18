// Survey probes. Each prints `PROBE <name>: <outcome>` and asserts nothing, so one file reads the
// same on the baseline and on the fix. Not for commit.

use nodedb_test_support::pgwire_harness::TestServer;

const FORCE_CALVIN: &str = "GRAPH INSERT EDGE IN 'a' FROM 'd2' TO 'd3' TYPE 'l'";

async fn edge_bearing(server: &TestServer, name: &str) {
    server.exec(&format!("CREATE COLLECTION {name}")).await.expect("CREATE COLLECTION");
    for i in 0..4 {
        server
            .exec(&format!("UPSERT INTO {name} {{ id: 'd{i}', title: 't{i}' }}"))
            .await
            .expect("seed row");
    }
    server
        .exec(&format!("GRAPH INSERT EDGE IN '{name}' FROM 'd0' TO 'd1' TYPE 'l'"))
        .await
        .expect("seed edge");
}

async fn fresh() -> TestServer {
    let server = TestServer::start().await;
    edge_bearing(&server, "a").await;
    edge_bearing(&server, "b").await;
    server
}

fn outcome(r: Result<impl std::fmt::Debug, impl std::fmt::Debug>) -> String {
    match r {
        Ok(_) => "ok".to_owned(),
        Err(e) => {
            let text = format!("{e:?}");
            if text.contains("cross-shard transaction aborted") {
                "ABORT (cross-shard transaction aborted)".to_owned()
            } else {
                format!("ERR {}", text.chars().take(160).collect::<String>())
            }
        }
    }
}

/// Run `stmts` in a block that also writes an edge, so the commit takes the Calvin path.
async fn in_block(name: &str, setup: &[&str], stmts: &[&str]) {
    let server = fresh().await;
    for s in setup {
        server.exec(s).await.unwrap_or_else(|e| panic!("setup {s}: {e:?}"));
    }
    server.exec("BEGIN").await.expect("BEGIN");
    for s in stmts {
        let r = server.exec(s).await;
        if r.is_err() {
            println!("PROBE {name}: statement `{s}` -> {}", outcome(r));
            let _ = server.exec("ROLLBACK").await;
            return;
        }
    }
    let r = server.exec(FORCE_CALVIN).await;
    if r.is_err() {
        println!("PROBE {name}: edge insert -> {}", outcome(r));
        let _ = server.exec("ROLLBACK").await;
        return;
    }
    println!("PROBE {name}: COMMIT -> {}", outcome(server.exec("COMMIT").await));
}

#[tokio::test]
async fn e1_predicate_update() {
    in_block("E1 predicate UPDATE", &[], &["UPDATE a SET title = 'x' WHERE title = 't2'"]).await;
}

#[tokio::test]
async fn e2_predicate_delete() {
    in_block("E2 predicate DELETE", &[], &["DELETE FROM a WHERE title = 't3'"]).await;
}

#[tokio::test]
async fn e3_update_from_join() {
    in_block(
        "E3 UPDATE ... FROM",
        &[],
        &["UPDATE a SET title = b.title FROM b WHERE a.id = b.id"],
    )
    .await;
}

#[tokio::test]
async fn e4_merge_update() {
    in_block(
        "E4 MERGE matched UPDATE",
        &[],
        &["MERGE INTO a t USING b s ON t.id = s.id WHEN MATCHED THEN UPDATE SET title = s.title"],
    )
    .await;
}

#[tokio::test]
async fn e5_merge_delete() {
    in_block(
        "E5 MERGE matched DELETE",
        &[],
        &["MERGE INTO a t USING b s ON t.id = s.id WHEN MATCHED THEN DELETE"],
    )
    .await;
}

#[tokio::test]
async fn e6_insert_select() {
    in_block(
        "E6 INSERT ... SELECT",
        &[],
        &["INSERT INTO a SELECT id, title FROM b WHERE title = 't2'"],
    )
    .await;
}

#[tokio::test]
async fn e7_delete_in_subquery() {
    in_block(
        "E7 DELETE ... IN (SELECT)",
        &[],
        &["DELETE FROM a WHERE id IN (SELECT id FROM b WHERE title = 't3')"],
    )
    .await;
}

#[tokio::test]
async fn e8_update_returning() {
    in_block(
        "E8 UPDATE ... RETURNING",
        &[],
        &["UPDATE a SET title = 'x' WHERE title = 't2' RETURNING id"],
    )
    .await;
}

#[tokio::test]
async fn e9_two_collections_non_key() {
    in_block(
        "E9 non-key UPDATE in two collections",
        &[],
        &[
            "UPDATE a SET title = 'x' WHERE title = 't2'",
            "UPDATE b SET title = 'y' WHERE title = 't2'",
        ],
    )
    .await;
}

/// Autocommit statements that run in an implicit transaction.
async fn autocommit(name: &str, setup: &[&str], stmt: &str) {
    let server = fresh().await;
    for s in setup {
        server.exec(s).await.unwrap_or_else(|e| panic!("setup {s}: {e:?}"));
    }
    println!("PROBE {name}: {}", outcome(server.exec(stmt).await));
}

#[tokio::test]
async fn i1_merge_delete_autocommit() {
    autocommit(
        "I1 MERGE DELETE into an edge-bearing collection, autocommit",
        &[],
        "MERGE INTO a t USING b s ON t.id = s.id WHEN MATCHED THEN DELETE",
    )
    .await;
}

#[tokio::test]
async fn i2_sync_trigger_predicate_update() {
    autocommit(
        "I2 SYNC trigger body: predicate UPDATE",
        &[
            "CREATE COLLECTION c (id TEXT PRIMARY KEY, v INT) WITH (engine='document_strict')",
            "CREATE SYNC TRIGGER bump AFTER INSERT ON c FOR EACH ROW \
             BEGIN UPDATE a SET title = 'trig' WHERE title = 't2'; END",
        ],
        "INSERT INTO c (id, v) VALUES ('c1', 1)",
    )
    .await;
}

#[tokio::test]
async fn i3_sync_trigger_predicate_delete() {
    autocommit(
        "I3 SYNC trigger body: predicate DELETE",
        &[
            "CREATE COLLECTION c (id TEXT PRIMARY KEY, v INT) WITH (engine='document_strict')",
            "CREATE SYNC TRIGGER purge AFTER INSERT ON c FOR EACH ROW \
             BEGIN DELETE FROM a WHERE title = 't3'; END",
        ],
        "INSERT INTO c (id, v) VALUES ('c1', 1)",
    )
    .await;
}

#[tokio::test]
async fn i4_before_trigger_predicate_update() {
    autocommit(
        "I4 BEFORE trigger body: predicate UPDATE",
        &[
            "CREATE COLLECTION c (id TEXT PRIMARY KEY, v INT) WITH (engine='document_strict')",
            "CREATE TRIGGER pre BEFORE INSERT ON c FOR EACH ROW \
             BEGIN UPDATE a SET title = 'trig' WHERE title = 't2'; END",
        ],
        "INSERT INTO c (id, v) VALUES ('c1', 1)",
    )
    .await;
}
