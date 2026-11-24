//! Real process termination at deterministic compaction boundaries.
//! Fixtures stay in a unique project-local run directory for evidence inspection.
use std::{
    fs,
    io::{self, BufRead, Write},
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    sync::mpsc,
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use swaggen_p00::journal::{Batch, CompactionStage, Journal};

const CASES: [&str; 6] = [
    "created", "partial", "written", "synced", "replaced", "acked",
];

fn batch(value: u8) -> Batch {
    Batch {
        draft: vec![value; 43],
        outbox: vec![value; 29],
    }
}

fn label(stage: CompactionStage) -> &'static str {
    match stage {
        CompactionStage::Created => "created",
        CompactionStage::PartialWrite => "partial",
        CompactionStage::Written => "written",
        CompactionStage::Synced => "synced",
        CompactionStage::Replaced => "replaced",
    }
}

fn checkpoint() {
    println!("READY");
    io::stdout()
        .flush()
        .expect("parent must observe the exact crash boundary");
    loop {
        std::thread::park();
    }
}

fn child(target: &str, path: &Path) -> io::Result<()> {
    if !CASES.contains(&target) {
        return Err(io::Error::other("unknown crash case"));
    }
    let (mut journal, recovered) = Journal::open(path)?;
    assert_eq!(recovered, vec![batch(1), batch(2)]);
    journal.compact_with_probe(&batch(2), |stage| {
        if target == label(stage) {
            checkpoint();
        }
    })?;
    if target == "acked" {
        checkpoint();
    }
    Err(io::Error::other("requested crash boundary was not reached"))
}

struct RunningChild(Child);
impl Drop for RunningChild {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

fn one_case(folder: &Path, case: &str) -> io::Result<(usize, usize)> {
    let path = folder.join(format!("{case}.journal"));
    let (mut journal, initial) = Journal::open(&path)?;
    assert!(initial.is_empty());
    journal.append(&batch(1))?;
    journal.append(&batch(2))?;
    drop(journal);
    let process = Command::new(std::env::current_exe()?)
        .arg("child")
        .arg(case)
        .arg(&path)
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()?;
    let mut process = RunningChild(process);
    let stdout = process.0.stdout.take().expect("piped stdout");
    let (sender, receiver) = mpsc::channel();
    let reader = std::thread::spawn(move || {
        let mut line = String::new();
        let result = io::BufReader::new(stdout)
            .read_line(&mut line)
            .map(|_| line);
        let _ = sender.send(result);
    });
    let line = receiver
        .recv_timeout(Duration::from_secs(10))
        .map_err(|_| {
            io::Error::new(
                io::ErrorKind::TimedOut,
                "child did not reach crash boundary",
            )
        })??;
    if line.trim() != "READY" {
        return Err(io::Error::other("child boundary protocol mismatch"));
    }
    assert!(
        Journal::open(&path).is_err(),
        "writer fence must be held by child at {case}"
    );
    process.0.kill()?;
    assert!(!process.0.wait()?.success());
    reader
        .join()
        .expect("boundary reader must finish after child termination");
    let (mut journal, recovered) = Journal::open(&path)?;
    let after_replace = matches!(case, "replaced" | "acked");
    let expected = if after_replace {
        vec![batch(2)]
    } else {
        vec![batch(1), batch(2)]
    };
    assert_eq!(
        recovered, expected,
        "all acknowledged draft+outbox data must survive {case}"
    );
    let count = recovered.len();
    assert_eq!(journal.append(&batch(3))?, count as u64 + 1);
    drop(journal);
    let (journal, appended) = Journal::open(&path)?;
    assert_eq!(appended.last(), Some(&batch(3)));
    assert_eq!(appended.len(), count + 1);
    drop(journal);
    let prefix = format!("{case}.journal.compact-");
    let orphans = fs::read_dir(folder)?
        .filter_map(Result::ok)
        .filter(|entry| entry.file_name().to_string_lossy().starts_with(&prefix))
        .count();
    assert_eq!(
        orphans,
        usize::from(!after_replace),
        "prepared files are never replayed as committed state"
    );
    Ok((count, orphans))
}

fn parent() -> io::Result<()> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../.local/p00-crash-matrix");
    fs::create_dir_all(&root)?;
    let folder = root.join(format!(
        "run-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(io::Error::other)?
            .as_nanos()
    ));
    fs::create_dir(&folder)?;
    let mut evidence = String::from("case,recovered_batches,orphan_files,status\n");
    for case in CASES {
        let (count, orphans) = one_case(&folder, case)?;
        evidence.push_str(&format!("{case},{count},{orphans},PASS\n"));
        println!(
            "PASS: compaction crash at {case}; recovered={count}, uncommitted_orphans={orphans}"
        );
    }
    let report = folder.join("results.csv");
    fs::write(&report, evidence)?;
    println!("Evidence: {}", report.display());
    println!(
        "LIMIT: forced process termination only; no disk power-loss, fsync-directory or physical disk-full acceptance"
    );
    Ok(())
}

fn main() -> io::Result<()> {
    let args: Vec<_> = std::env::args_os().collect();
    if args.len() == 4 && args[1] == "child" {
        return child(&args[2].to_string_lossy(), &PathBuf::from(&args[3]));
    }
    if args.len() != 1 {
        return Err(io::Error::other("unexpected probe arguments"));
    }
    parent()
}
