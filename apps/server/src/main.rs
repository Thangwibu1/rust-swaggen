//! Swaggen collaboration server binary.
use std::{net::SocketAddr, sync::Arc};
use swaggen_mongo_store::{
    ServerStore, memory_backend::InMemoryStore, mongo_backend::MongoStore,
};

#[tokio::main]
async fn main() {
    let args: Vec<String> = std::env::args().collect();
    if let Err(code) = run(&args).await {
        std::process::exit(code);
    }
}

async fn run(args: &[String]) -> Result<(), i32> {
    if args.get(1).map(String::as_str) == Some("export-openapi") {
        let out_path = args
            .get(2)
            .cloned()
            .unwrap_or_else(|| "07-operations/server-openapi.json".into());
        let json = serde_json::to_string_pretty(&swaggen_server::build_server_openapi_json())
            .map_err(|_| 2)?;
        std::fs::write(&out_path, format!("{json}\n")).map_err(|_| 2)?;
        println!("Exported server OpenAPI 3.1 contract to {out_path}");
        return Ok(());
    }

    let bootstrap_secret = std::env::var("SWAGGEN_BOOTSTRAP_TOKEN").ok().filter(|s| !s.is_empty());

    if args.get(1).map(String::as_str) == Some("backup") {
        let out = args
            .get(2)
            .cloned()
            .unwrap_or_else(|| "swaggen-backup.json".into());
        let Ok(uri) = std::env::var("SWAGGEN_MONGO_URI") else {
            eprintln!("BLOCKED: SWAGGEN_MONGO_URI is required for backup");
            return Err(2);
        };
        let db_name = std::env::var("SWAGGEN_MONGO_DB").unwrap_or_else(|_| "swaggen".into());
        let mongo = MongoStore::connect(&uri, &db_name, bootstrap_secret)
            .await
            .map_err(|_| 2)?;
        let bundle = swaggen_mongo_store::backup::export_mongo_backup(
            mongo.database(),
            swaggen_mongo_store::mongo_backend::SCHEMA_MIGRATION_VERSION,
            swaggen_mongo_store::repo::now_unix(),
        )
        .await
        .map_err(|_| 2)?;
        let json = serde_json::to_string_pretty(&bundle).map_err(|_| 2)?;
        std::fs::write(&out, format!("{json}\n")).map_err(|_| 2)?;
        println!(
            "BACKUP PASS: {} collections sha256={}",
            bundle.manifest.collection_counts.len(),
            bundle.manifest.payload_sha256
        );
        return Ok(());
    }

    if args.get(1).map(String::as_str) == Some("restore") {
        let in_path = args
            .get(2)
            .cloned()
            .unwrap_or_else(|| "swaggen-backup.json".into());
        let raw = std::fs::read_to_string(&in_path).map_err(|_| {
            eprintln!("Restore failed: backup file missing");
            2
        })?;
        let bundle: swaggen_mongo_store::backup::BackupBundle =
            serde_json::from_str(&raw).map_err(|_| {
                eprintln!("Restore failed: corrupt backup JSON");
                2
            })?;
        if let Err(err) = bundle.verify_integrity() {
            eprintln!("Restore blocked: {err}");
            return Err(3);
        }
        let Ok(uri) = std::env::var("SWAGGEN_MONGO_URI") else {
            eprintln!("Checksum OK, live Mongo restore BLOCKED: SWAGGEN_MONGO_URI is not set");
            return Err(2);
        };
        let db_name = std::env::var("SWAGGEN_MONGO_DB").unwrap_or_else(|_| "swaggen".into());
        let mongo = MongoStore::connect(&uri, &db_name, bootstrap_secret)
            .await
            .map_err(|_| 2)?;
        if let Err(err) = swaggen_mongo_store::backup::restore_mongo_backup(mongo.database(), &bundle).await {
            eprintln!("Restore failed: {err}");
            return Err(3);
        }
        println!("RESTORE PASS: verified all counts and payload checksum");
        return Ok(());
    }

    if args.get(1).map(String::as_str) == Some("doctor-mongo") {
        let Ok(uri) = std::env::var("SWAGGEN_MONGO_URI") else {
            eprintln!("BLOCKED: SWAGGEN_MONGO_URI is not set");
            return Err(2);
        };
        let db = std::env::var("SWAGGEN_MONGO_DB").unwrap_or_else(|_| "swaggen".into());
        let store = match MongoStore::connect(&uri, &db, bootstrap_secret).await {
            Ok(m) => ServerStore::Mongo(m),
            Err(_) => {
                eprintln!("FAIL: MongoDB connection, replica-set check, or migrations failed");
                return Err(2);
            }
        };
        if store.run_doctor_probe().await.is_err() {
            eprintln!("FAIL: MongoDB transaction/rollback doctor probe failed");
            return Err(2);
        }
        println!("PASS: MongoDB replica-set topology, migrations, and transaction rollback verified");
        return Ok(());
    }

    let in_memory_dev = args.iter().any(|a| a == "--in-memory-dev");
    let store = if in_memory_dev {
        Arc::new(ServerStore::Memory(InMemoryStore::new(bootstrap_secret)))
    } else {
        let Ok(uri) = std::env::var("SWAGGEN_MONGO_URI") else {
            eprintln!("Server startup aborted: SWAGGEN_MONGO_URI is required (pass --in-memory-dev only for disposable local testing)");
            return Err(2);
        };
        let db = std::env::var("SWAGGEN_MONGO_DB").unwrap_or_else(|_| "swaggen".into());
        let mongo = MongoStore::connect(&uri, &db, bootstrap_secret)
            .await
            .map_err(|_| {
                eprintln!("Server startup failed: MongoDB replica set unreachable or migrations failed");
                2
            })?;
        Arc::new(ServerStore::Mongo(mongo))
    };

    let address: SocketAddr = std::env::var("SWAGGEN_BIND")
        .unwrap_or_else(|_| "127.0.0.1:8080".into())
        .parse()
        .map_err(|_| {
            eprintln!("Invalid SWAGGEN_BIND socket address");
            2
        })?;
    let listener = tokio::net::TcpListener::bind(address).await.map_err(|_| {
        eprintln!("Failed to bind SWAGGEN_BIND listener");
        2
    })?;
    axum::serve(
        listener,
        swaggen_server::router(store).into_make_service_with_connect_info::<SocketAddr>(),
    )
    .with_graceful_shutdown(async {
        let _ = tokio::signal::ctrl_c().await;
    })
    .await
    .map_err(|_| {
        eprintln!("HTTP server loop exited with error");
        2
    })?;
    Ok(())
}
