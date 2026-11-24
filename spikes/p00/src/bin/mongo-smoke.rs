//! Run only against a unique disposable database. No URI or driver errors are printed.
use mongodb::{
    Client, ClientSession,
    bson::{Document, doc},
    options::{ClientOptions, ReadConcern, WriteConcern},
};
use std::{
    error::Error,
    time::{Duration, Instant},
};
type Result<T> = std::result::Result<T, Box<dyn Error + Send + Sync>>;

async fn commit(session: &mut ClientSession) -> Result<()> {
    let start = Instant::now();
    loop {
        match session.commit_transaction().await {
            Ok(()) => return Ok(()),
            Err(e)
                if e.contains_label("UnknownTransactionCommitResult")
                    && start.elapsed() < Duration::from_secs(30) =>
            {
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
            // Caller reports unverified on error; no definite failed-commit claim.
            Err(e) => return Err(e.into()),
        }
    }
}

async fn run(stage: &mut &'static str) -> Result<()> {
    let uri = std::env::var("SWAGGEN_MONGO_URI").map_err(|_| "SWAGGEN_MONGO_URI missing")?;
    let mut options = ClientOptions::parse(&uri).await?;
    options.server_selection_timeout = Some(Duration::from_secs(8));
    options.connect_timeout = Some(Duration::from_secs(5));
    options.app_name = Some("swaggen-p00-isolated-smoke".into());
    let client = Client::with_options(options)?;
    let admin = client.database("admin");
    *stage = "topology";
    let hello = admin.run_command(doc! { "hello": 1 }).await?;
    if !hello.get_bool("isWritablePrimary").unwrap_or(false)
        || hello.get_str("setName").is_err()
        || !hello.contains_key("logicalSessionTimeoutMinutes")
    {
        return Err("writable replica set primary with sessions required".into());
    }
    *stage = "server_version";
    let build = admin.run_command(doc! {"buildInfo": 1}).await?;
    *stage = "fcv";
    let fcv = admin
        .run_command(doc! {"getParameter": 1, "featureCompatibilityVersion": 1})
        .await?;
    // Only allowlisted topology fields, never the entire hello/command/URI.
    println!(
        "{}",
        serde_json::json!({"check":"topology", "status":"PASS",
        "server_version":build.get_str("version").unwrap_or("unknown"),
        "fcv":fcv.get_document("featureCompatibilityVersion").ok().and_then(|d|d.get_str("version").ok()),
        "replica_set":true,"writable_primary":true})
    );
    let name = format!(
        "swaggen_p00_{}",
        mongodb::bson::oid::ObjectId::new().to_hex()
    );
    let db = client.database(&name);
    // No dropping existing databases, no bootstrap, no rs reconfiguration, no failpoints.
    let result: Result<()> = async {
        *stage = "fixture_setup";
        db.create_collection("entities").await?;
        db.create_collection("revisions").await?;
        let entities = db.collection::<Document>("entities");
        let revisions = db.collection::<Document>("revisions");
        *stage = "commit_and_invisibility";
        let mut session = client.start_session().await?;
        session
            .start_transaction()
            .read_concern(ReadConcern::snapshot())
            .write_concern(WriteConcern::majority())
            .await?;
        entities
            .insert_one(doc! {"_id":"commit", "revision":1})
            .session(&mut session)
            .await?;
        revisions
            .insert_one(doc! {"_id":"commit-revision", "entity":"commit"})
            .session(&mut session)
            .await?;
        // Independent observer must not see uncommitted state.
        assert_eq!(entities.count_documents(doc! {}).await?, 0);
        assert_eq!(revisions.count_documents(doc! {}).await?, 0);
        commit(&mut session).await?;
        assert_eq!(entities.count_documents(doc! {}).await?, 1);
        assert_eq!(revisions.count_documents(doc! {}).await?, 1);
        println!("{{\"check\":\"multi_document_commit_and_invisibility\",\"status\":\"PASS\"}}");
        *stage = "explicit_rollback";
        session
            .start_transaction()
            .read_concern(ReadConcern::snapshot())
            .write_concern(WriteConcern::majority())
            .await?;
        entities
            .insert_one(doc! {"_id":"abort"})
            .session(&mut session)
            .await?;
        revisions
            .insert_one(doc! {"_id":"abort-revision"})
            .session(&mut session)
            .await?;
        session.abort_transaction().await?;
        assert_eq!(entities.count_documents(doc! {}).await?, 1);
        assert_eq!(revisions.count_documents(doc! {}).await?, 1);
        println!("{{\"check\":\"explicit_multi_document_rollback\",\"status\":\"PASS\"}}");
        *stage = "error_rollback";
        // Actual server error after a successful write must not leave partial state.
        session.start_transaction().await?;
        entities
            .insert_one(doc! {"_id":"error-rollback"})
            .session(&mut session)
            .await?;
        assert!(
            revisions
                .insert_one(doc! {"_id":"commit-revision"})
                .session(&mut session)
                .await
                .is_err()
        );
        let _ = session.abort_transaction().await;
        assert_eq!(entities.count_documents(doc! {}).await?, 1);
        assert_eq!(revisions.count_documents(doc! {}).await?, 1);
        println!("{{\"check\":\"duplicate_key_error_rollback\",\"status\":\"PASS\"}}");
        Ok(())
    }
    .await;
    // Only this process's random ObjectId-named fixture database.
    let operation_stage = *stage;
    *stage = "fixture_cleanup";
    let cleanup = db.drop().await;
    client.shutdown().await;
    if let Err(error) = result {
        *stage = operation_stage;
        return Err(error);
    }
    cleanup?;
    println!("{{\"check\":\"isolated_fixture_cleanup\",\"status\":\"PASS\"}}");
    Ok(())
}

#[tokio::main]
async fn main() {
    let mut stage = "configuration";
    if let Err(error) = run(&mut stage).await {
        let diagnosis = swaggen_p00::mongo_diagnostics::classify(
            error.downcast_ref::<mongodb::error::Error>(),
            stage,
        );
        eprintln!(
            "{}",
            serde_json::json!({"status":diagnosis.status,"stage":stage,"category":diagnosis.category,"detail":"Mongo probe did not complete; no transaction acceptance. Original driver error bodies are suppressed."})
        );
        std::process::exit(2);
    }
}
