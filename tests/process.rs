use std::{
    fs,
    io::{Read, Write},
    path::Path,
    process::{Child, Command, Stdio},
    sync::OnceLock,
    thread,
    time::{Duration, Instant},
};

fn fixture() -> &'static Path {
    static FIXTURE: OnceLock<tempfile::TempDir> = OnceLock::new();
    let dir = FIXTURE.get_or_init(|| {
        let dir = tempfile::tempdir().unwrap();
        let status = Command::new("rustc")
            .arg("--edition=2024")
            .arg(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/process_provider.rs"))
            .arg("-o")
            .arg(dir.path().join(if cfg!(windows) {
                "provider.exe"
            } else {
                "provider"
            }))
            .status()
            .unwrap();
        assert!(status.success());
        dir
    });
    dir.path()
}
fn ai(dir: &Path, mode: &str) -> Command {
    let mut config = ainline::config::Config::default();
    config.models.get_mut("ollama").unwrap().binary = fixture()
        .join(if cfg!(windows) {
            "provider.exe"
        } else {
            "provider"
        })
        .to_string_lossy()
        .into_owned();
    fs::write(
        dir.join("config.yaml"),
        serde_yaml_ng::to_string(&config).unwrap(),
    )
    .unwrap();
    let mut c = Command::new(env!("CARGO_BIN_EXE_ai"));
    c.current_dir(dir)
        .env("AI_CONFIG_FILE", dir.join("config.yaml"))
        .args(["ol", "--no-context", "--model", mode])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    c
}
fn wait(child: &mut Child) -> std::process::ExitStatus {
    let start = Instant::now();
    loop {
        if let Some(status) = child.try_wait().unwrap() {
            return status;
        }
        if start.elapsed() > Duration::from_secs(8) {
            let _ = child.kill();
            panic!("supervisor hung");
        }
        thread::sleep(Duration::from_millis(10));
    }
}
fn ready(dir: &Path, child: &mut Child) {
    let start = Instant::now();
    while !dir.join("ready").exists() {
        assert!(
            child.try_wait().unwrap().is_none(),
            "child exited before readiness"
        );
        if start.elapsed() > Duration::from_secs(5) {
            let _ = child.kill();
            panic!("fixture did not start");
        }
        thread::sleep(Duration::from_millis(10));
    }
}
fn assert_descendant_stopped(dir: &Path) {
    thread::sleep(Duration::from_millis(100));
    let first = fs::read(dir.join("heartbeat")).unwrap();
    thread::sleep(Duration::from_millis(150));
    assert_eq!(
        first,
        fs::read(dir.join("heartbeat")).unwrap(),
        "descendant survived cleanup"
    );
}
#[test]
fn timeout_cleans_descendants_and_a_blocked_stdin_writer() {
    let dir = tempfile::tempdir().unwrap();
    let mut child = ai(dir.path(), "hang")
        .args(["--timeout", "1", "hello"])
        .stdin(Stdio::piped())
        .spawn()
        .unwrap();
    let mut stdin = child.stdin.take().unwrap();
    let writer = thread::spawn(move || {
        let _ = stdin.write_all(&vec![b'x'; 900_000]);
    });
    ready(dir.path(), &mut child);
    assert_eq!(wait(&mut child).code(), Some(124));
    writer.join().unwrap();
    assert_descendant_stopped(dir.path());
}
#[test]
fn normal_exit_also_cleans_descendants_holding_pipes() {
    let dir = tempfile::tempdir().unwrap();
    let mut child = ai(dir.path(), "exit-with-child")
        .args(["--timeout", "3", "hello"])
        .spawn()
        .unwrap();
    assert!(wait(&mut child).success());
    assert_descendant_stopped(dir.path());
}
#[test]
fn output_limits_cover_stdout_and_stderr() {
    for mode in ["flood", "stderr-flood"] {
        let dir = tempfile::tempdir().unwrap();
        let out = ai(dir.path(), mode)
            .args(["--max-output-bytes", "1024", "hello"])
            .output()
            .unwrap();
        assert_eq!(out.status.code(), Some(1));
        assert!(String::from_utf8_lossy(&out.stderr).contains("output exceeded 1024 bytes"));
        assert!(out.stdout.len() <= 1024);
    }
}
#[test]
fn streaming_delivers_before_child_completion_and_buffering_preserves_failure() {
    let dir = tempfile::tempdir().unwrap();
    let mut child = ai(dir.path(), "slow")
        .args(["--timeout", "5", "hello"])
        .spawn()
        .unwrap();
    let mut pipe = child.stdout.take().unwrap();
    let (tx, rx) = std::sync::mpsc::channel();
    let reader = thread::spawn(move || {
        let mut first = [0; 5];
        pipe.read_exact(&mut first).unwrap();
        tx.send(first).unwrap();
        let mut rest = String::new();
        pipe.read_to_string(&mut rest).unwrap();
        rest
    });
    assert_eq!(rx.recv_timeout(Duration::from_secs(3)).unwrap(), *b"first");
    assert!(child.try_wait().unwrap().is_none());
    fs::write(dir.path().join("continue"), "go").unwrap();
    assert!(wait(&mut child).success());
    assert_eq!(reader.join().unwrap(), "second");
    let buffered = ai(dir.path(), "fail")
        .args(["--no-stream", "hello"])
        .output()
        .unwrap();
    assert_eq!(buffered.status.code(), Some(42));
    assert!(buffered.stdout.is_empty());
    let streamed = ai(dir.path(), "fail").arg("hello").output().unwrap();
    assert_eq!(streamed.status.code(), Some(42));
    assert_eq!(streamed.stdout, b"partial");
}
#[test]
fn stalled_downstream_and_closed_pipe_do_not_hang_supervisor() {
    let dir = tempfile::tempdir().unwrap();
    let mut child = ai(dir.path(), "flood")
        .args([
            "--timeout",
            "1",
            "--max-output-bytes",
            "1073741824",
            "hello",
        ])
        .spawn()
        .unwrap();
    assert_eq!(wait(&mut child).code(), Some(124));
    let mut child = ai(dir.path(), "flood")
        .args(["--timeout", "3", "hello"])
        .spawn()
        .unwrap();
    drop(child.stdout.take());
    assert!(wait(&mut child).success());
}
#[cfg(unix)]
#[test]
fn interrupt_cleans_the_process_group() {
    let dir = tempfile::tempdir().unwrap();
    let mut child = ai(dir.path(), "hang")
        .args(["--timeout", "5", "hello"])
        .spawn()
        .unwrap();
    ready(dir.path(), &mut child);
    unsafe {
        libc::kill(child.id() as i32, libc::SIGINT);
    }
    assert_eq!(wait(&mut child).code(), Some(130));
    assert_descendant_stopped(dir.path());
}

#[test]
fn claude_deltas_stream_before_completion_without_protocol_metadata() {
    let dir = tempfile::tempdir().unwrap();
    let _ = ai(dir.path(), "slow");
    let path = dir.path().join("config.yaml");
    let mut config = ainline::config::Config::parse(&fs::read_to_string(&path).unwrap()).unwrap();
    config.models.get_mut("claude").unwrap().binary = config.models["ollama"].binary.clone();
    fs::write(&path, serde_yaml_ng::to_string(&config).unwrap()).unwrap();
    let mut child = Command::new(env!("CARGO_BIN_EXE_ai"))
        .current_dir(dir.path())
        .env("AI_CONFIG_FILE", path)
        .args(["cl", "--no-context", "--timeout", "5", "hello"])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let mut pipe = child.stdout.take().unwrap();
    let (tx, rx) = std::sync::mpsc::channel();
    let reader = thread::spawn(move || {
        let mut first = [0; 5];
        pipe.read_exact(&mut first).unwrap();
        tx.send(first).unwrap();
        let mut rest = String::new();
        pipe.read_to_string(&mut rest).unwrap();
        rest
    });
    assert_eq!(rx.recv_timeout(Duration::from_secs(3)).unwrap(), *b"first");
    assert!(child.try_wait().unwrap().is_none());
    fs::write(dir.path().join("continue"), "go").unwrap();
    assert!(wait(&mut child).success());
    assert_eq!(reader.join().unwrap(), "second");
}

#[test]
fn buffered_stdout_and_stalled_stderr_respect_deadline() {
    let dir = tempfile::tempdir().unwrap();
    let mut child = ai(dir.path(), "large-result")
        .args(["--no-stream", "--timeout", "1", "hello"])
        .spawn()
        .unwrap();
    assert_eq!(wait(&mut child).code(), Some(124));
    let mut child = ai(dir.path(), "stderr-flood")
        .args([
            "--timeout",
            "1",
            "--max-output-bytes",
            "1073741824",
            "hello",
        ])
        .spawn()
        .unwrap();
    assert_eq!(wait(&mut child).code(), Some(124));
}
