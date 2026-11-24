//! Headless Swaggen CLI entrypoint (`swaggen-cli run` / `swaggen-cli snippet`) (MK05, MK06).
use std::{path::PathBuf, process::ExitCode};
use swaggen_cli::{CliRunArgs, EXIT_INFRA_OR_USAGE, execute_cli_run, generate_cli_snippet};

fn usage() {
    eprintln!(
        "Swaggen CLI\n\
         Usage:\n  swaggen-cli run --workspace <file.json> [--env <env.json>] [--dataset <csv|json>] [--spec <openapi>] [--report-json <out.json>] [--junit <out.xml>] [--secrets-root <dir>] [--workspace-id <id>] [--iterations <n>] [--delay-ms <ms>] [--stop-on-fail] [--validate-contract]\n  swaggen-cli snippet --workspace <file.json> [--index <n>] [--target <curl|rust|js|dart|go>]"
    );
}

#[tokio::main]
async fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.is_empty() || args[0] == "--help" || args[0] == "-h" {
        usage();
        return ExitCode::from(0);
    }
    match args[0].as_str() {
        "snippet" => {
            let mut ws = None;
            let mut idx = 0usize;
            let mut target = String::from("curl");
            let mut i = 1;
            while i < args.len() {
                match args[i].as_str() {
                    "--workspace" => {
                        i += 1;
                        ws = args.get(i).map(PathBuf::from);
                    }
                    "--index" => {
                        i += 1;
                        idx = args.get(i).and_then(|s| s.parse().ok()).unwrap_or(0);
                    }
                    "--target" => {
                        i += 1;
                        if let Some(t) = args.get(i) {
                            target = t.clone();
                        }
                    }
                    _ => {}
                }
                i += 1;
            }
            let Some(ws_path) = ws else {
                usage();
                return ExitCode::from(EXIT_INFRA_OR_USAGE as u8);
            };
            match generate_cli_snippet(&ws_path, idx, &target) {
                Ok(out) => {
                    println!("{out}");
                    ExitCode::from(0)
                }
                Err(e) => {
                    eprintln!("Error: {e}");
                    ExitCode::from(EXIT_INFRA_OR_USAGE as u8)
                }
            }
        }
        "run" => {
            let mut ws = None;
            let mut run_args = CliRunArgs {
                workspace_path: PathBuf::new(),
                env_path: None,
                dataset_path: None,
                spec_paths: Vec::new(),
                report_json_path: None,
                junit_path: None,
                secrets_root: None,
                workspace_id: "personal".into(),
                stop_on_failure: false,
                validate_contract: false,
                iterations: 1,
                delay_ms: 0,
                extra_redact_fields: Vec::new(),
            };
            let mut i = 1;
            while i < args.len() {
                match args[i].as_str() {
                    "--workspace" => {
                        i += 1;
                        ws = args.get(i).map(PathBuf::from);
                    }
                    "--env" => {
                        i += 1;
                        run_args.env_path = args.get(i).map(PathBuf::from);
                    }
                    "--dataset" => {
                        i += 1;
                        run_args.dataset_path = args.get(i).map(PathBuf::from);
                    }
                    "--spec" => {
                        i += 1;
                        if let Some(p) = args.get(i) {
                            run_args.spec_paths.push(PathBuf::from(p));
                        }
                    }
                    "--report-json" => {
                        i += 1;
                        run_args.report_json_path = args.get(i).map(PathBuf::from);
                    }
                    "--junit" => {
                        i += 1;
                        run_args.junit_path = args.get(i).map(PathBuf::from);
                    }
                    "--secrets-root" => {
                        i += 1;
                        run_args.secrets_root = args.get(i).map(PathBuf::from);
                    }
                    "--workspace-id" => {
                        i += 1;
                        if let Some(v) = args.get(i) {
                            run_args.workspace_id = v.clone();
                        }
                    }
                    "--iterations" => {
                        i += 1;
                        if let Some(v) = args.get(i).and_then(|s| s.parse().ok()) {
                            run_args.iterations = v;
                        }
                    }
                    "--delay-ms" => {
                        i += 1;
                        if let Some(v) = args.get(i).and_then(|s| s.parse().ok()) {
                            run_args.delay_ms = v;
                        }
                    }
                    "--stop-on-fail" => run_args.stop_on_failure = true,
                    "--validate-contract" => run_args.validate_contract = true,
                    other => {
                        eprintln!("Unknown argument: {other}");
                        return ExitCode::from(EXIT_INFRA_OR_USAGE as u8);
                    }
                }
                i += 1;
            }
            let Some(ws_path) = ws else {
                usage();
                return ExitCode::from(EXIT_INFRA_OR_USAGE as u8);
            };
            run_args.workspace_path = ws_path;
            match execute_cli_run(&run_args).await {
                Ok((rep, code)) => {
                    println!(
                        "Run {} finished: passed={} failed_assertions={} failed_contract={} skipped={} infra_errors={}",
                        rep.manifest.content_sha256,
                        rep.passed,
                        rep.failed_assertions,
                        rep.failed_contract,
                        rep.skipped,
                        rep.infra_errors
                    );
                    ExitCode::from(code as u8)
                }
                Err(e) => {
                    eprintln!("Error: {e}");
                    ExitCode::from(EXIT_INFRA_OR_USAGE as u8)
                }
            }
        }
        _ => {
            usage();
            ExitCode::from(EXIT_INFRA_OR_USAGE as u8)
        }
    }
}
