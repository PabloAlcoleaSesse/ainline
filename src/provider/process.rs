//! Bounded, synchronous process supervision. Workers only perform blocking pipe I/O;
//! the supervisor remains responsive to deadlines and cancellation under backpressure.
use super::ProviderError;
use std::{
    io::{self, Read, Write},
    process::{Command, ExitStatus, Stdio},
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc::{self, SyncSender, TrySendError},
    },
    time::{Duration, Instant},
};
#[cfg(unix)]
#[path = "process/unix.rs"]
mod platform;
#[cfg(windows)]
#[path = "process/windows.rs"]
mod platform;

pub static INTERRUPTED: AtomicBool = AtomicBool::new(false);
pub fn install_signal_handler() -> io::Result<()> {
    platform::install_signal_handler()
}

#[derive(Clone, Copy, Debug)]
pub struct Limits {
    pub timeout: Duration,
    pub max_output_bytes: u64,
}
impl Default for Limits {
    fn default() -> Self {
        Self {
            timeout: Duration::from_secs(300),
            max_output_bytes: 16 * 1024 * 1024,
        }
    }
}
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Stream {
    Buffered,
    Text,
    Claude,
}
pub struct Output {
    pub stream_result: Result<(), ProviderError>,
    pub status: ExitStatus,
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
    pub written: io::Result<()>,
}
enum Event {
    Data(bool, Vec<u8>),
    Eof,
    ReadError(io::Error),
    Written(io::Result<()>),
    Delivered(io::Result<()>),
}
enum Delivery {
    Data(bool, Vec<u8>),
    Finish,
}

fn check(start: Instant, limits: Limits) -> Result<(), ProviderError> {
    if INTERRUPTED.load(Ordering::Relaxed) {
        return Err(ProviderError::Interrupted);
    }
    if start.elapsed() >= limits.timeout {
        return Err(ProviderError::Timeout);
    }
    Ok(())
}
fn reader(mut pipe: impl Read + Send + 'static, stdout: bool, tx: SyncSender<Event>) {
    std::thread::spawn(move || {
        let mut buffer = [0; 8192];
        loop {
            match pipe.read(&mut buffer) {
                Ok(0) => {
                    let _ = tx.send(Event::Eof);
                    break;
                }
                Ok(n) => {
                    if tx.send(Event::Data(stdout, buffer[..n].to_vec())).is_err() {
                        break;
                    }
                }
                Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
                Err(e) => {
                    let _ = tx.send(Event::ReadError(e));
                    break;
                }
            }
        }
    });
}
fn deliver(
    tx: &SyncSender<Delivery>,
    mut item: Delivery,
    start: Instant,
    limits: Limits,
) -> Result<(), ProviderError> {
    loop {
        check(start, limits)?;
        match tx.try_send(item) {
            Ok(()) => return Ok(()),
            Err(TrySendError::Full(back)) => {
                item = back;
                std::thread::sleep(Duration::from_millis(2));
            }
            // The worker reports the actual I/O error through its completion event.
            Err(TrySendError::Disconnected(_)) => return Ok(()),
        }
    }
}

/// Capture diagnostics without forwarding potentially sensitive stderr.
pub fn capture(command: Command, limits: Limits) -> Result<Output, ProviderError> {
    run_inner(command, Vec::new(), limits, Stream::Buffered, true)
}

pub fn run(
    command: Command,
    input: Vec<u8>,
    limits: Limits,
    stream: Stream,
) -> Result<Output, ProviderError> {
    run_inner(command, input, limits, stream, false)
}

fn run_inner(
    mut command: Command,
    input: Vec<u8>,
    limits: Limits,
    stream: Stream,
    quiet: bool,
) -> Result<Output, ProviderError> {
    check(Instant::now(), limits)?;
    command
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut tree = platform::Tree::spawn(&mut command)?;
    let start = Instant::now();
    let (tx, rx) = mpsc::sync_channel(16);
    reader(
        tree.child.stdout.take().expect("piped stdout"),
        true,
        tx.clone(),
    );
    reader(
        tree.child.stderr.take().expect("piped stderr"),
        false,
        tx.clone(),
    );
    let mut stdin = tree.child.stdin.take().expect("piped stdin");
    let writer_tx = tx.clone();
    std::thread::spawn(move || {
        let result = stdin.write_all(&input);
        drop(stdin);
        let _ = writer_tx.send(Event::Written(result));
    });
    let (delivery_tx, delivery_rx) = mpsc::sync_channel(8);
    std::thread::spawn(move || {
        let result = (|| -> io::Result<()> {
            while let Ok(item) = delivery_rx.recv() {
                match item {
                    Delivery::Data(stdout, bytes) => {
                        if stdout {
                            let mut output = io::stdout().lock();
                            output.write_all(&bytes)?;
                            output.flush()?;
                        } else {
                            let mut output = io::stderr().lock();
                            output.write_all(&bytes)?;
                            output.flush()?;
                        }
                    }
                    Delivery::Finish => break,
                }
            }
            Ok(())
        })();
        drop(delivery_rx);
        let _ = tx.send(Event::Delivered(result));
    });
    let mut decoder = super::claude::StreamDecoder::default();
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    let mut count = 0u64;
    let mut eof = 0;
    let mut status = None;
    let mut written = None;
    let mut delivery_done = false;
    let mut finishing = false;
    loop {
        check(start, limits)?;
        if status.is_none() {
            status = tree.child.try_wait()?;
            if status.is_some() {
                tree.terminate();
            }
        }
        if status.is_some() && eof == 2 && written.is_some() && !finishing {
            deliver(&delivery_tx, Delivery::Finish, start, limits)?;
            finishing = true;
        }
        if finishing && delivery_done {
            break;
        }
        match rx.recv_timeout(Duration::from_millis(5)) {
            Ok(Event::Data(is_stdout, bytes)) => {
                count = count.saturating_add(bytes.len() as u64);
                if count > limits.max_output_bytes {
                    return Err(ProviderError::OutputLimit(limits.max_output_bytes));
                }
                if is_stdout && stream == Stream::Buffered {
                    stdout.extend_from_slice(&bytes);
                }
                if !is_stdout {
                    stderr.extend_from_slice(&bytes);
                }
                if is_stdout && stream == Stream::Claude {
                    for chunk in decoder.push(&bytes)? {
                        deliver(&delivery_tx, Delivery::Data(true, chunk), start, limits)?;
                    }
                } else if (!is_stdout && !quiet) || stream == Stream::Text {
                    deliver(
                        &delivery_tx,
                        Delivery::Data(is_stdout, bytes),
                        start,
                        limits,
                    )?;
                }
            }
            Ok(Event::Eof) => eof += 1,
            Ok(Event::Written(result)) => written = Some(result),
            Ok(Event::Delivered(result)) => {
                result?;
                delivery_done = true;
            }
            Ok(Event::ReadError(e)) => return Err(e.into()),
            Err(mpsc::RecvTimeoutError::Timeout) => (),
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                return Err(ProviderError::Response(
                    "process workers disconnected".into(),
                ));
            }
        }
    }
    Ok(Output {
        stream_result: if stream == Stream::Claude {
            decoder.finish()
        } else {
            Ok(())
        },
        status: status.expect("finished process"),
        stdout,
        stderr,
        written: written.expect("finished writer"),
    })
}

/// Deliver a validated/buffered response under the original request deadline too.
pub fn write_response(bytes: Vec<u8>, start: Instant, limits: Limits) -> Result<(), ProviderError> {
    if bytes.is_empty() {
        return Ok(());
    }
    let (tx, rx) = mpsc::sync_channel(1);
    std::thread::spawn(move || {
        let mut stdout = io::stdout().lock();
        let result = stdout.write_all(&bytes).and_then(|()| stdout.flush());
        let _ = tx.send(result);
    });
    loop {
        check(start, limits)?;
        match rx.recv_timeout(Duration::from_millis(5)) {
            Ok(result) => return result.map_err(ProviderError::from),
            Err(mpsc::RecvTimeoutError::Timeout) => (),
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                return Err(ProviderError::Response("stdout worker disconnected".into()));
            }
        }
    }
}

/// A stalled stderr consumer must not prevent the CLI from exiting after cleanup.
pub fn report_error(message: String) {
    let (tx, rx) = mpsc::sync_channel(1);
    std::thread::spawn(move || {
        let _ = io::stderr().write_all(message.as_bytes());
        let _ = tx.send(());
    });
    let _ = rx.recv_timeout(Duration::from_millis(250));
}
