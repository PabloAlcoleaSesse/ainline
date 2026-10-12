use std::{fs, io::{self, Read, Write}, process::{Command, Stdio}, thread, time::Duration};
fn main() {
    let args: Vec<_> = std::env::args().collect();
    if args.iter().any(|arg| arg == "stream-json") {
        let mut input = String::new();
        io::stdin().read_to_string(&mut input).unwrap();
        println!("{{\"type\":\"stream_event\",\"event\":{{\"type\":\"content_block_delta\",\"delta\":{{\"type\":\"text_delta\",\"text\":\"first\"}}}}}}");
        io::stdout().flush().unwrap();
        fs::write("ready", "ready").unwrap();
        while !std::path::Path::new("continue").exists() { thread::sleep(Duration::from_millis(10)); }
        println!("{{\"type\":\"stream_event\",\"event\":{{\"type\":\"content_block_delta\",\"delta\":{{\"type\":\"text_delta\",\"text\":\"second\"}}}}}}");
        println!("{{\"type\":\"result\",\"is_error\":false,\"result\":\"firstsecond\"}}");
        return;
    }
    let mode = args.get(2).map(String::as_str).unwrap_or("");
    if mode == "heartbeat" {
        let mut tick = 0u64;
        loop {
            fs::write("heartbeat", tick.to_string()).unwrap();
            tick += 1;
            thread::sleep(Duration::from_millis(10));
        }
    }
    if mode == "hang" || mode == "exit-with-child" {
        let _child = Command::new(std::env::current_exe().unwrap()).args(["run", "heartbeat"])
            .stdin(Stdio::null()).spawn().unwrap();
        while !std::path::Path::new("heartbeat").exists() { thread::sleep(Duration::from_millis(10)); }
        fs::write("ready", "ready").unwrap();
        if mode == "exit-with-child" { print!("done"); return; }
        loop { thread::sleep(Duration::from_secs(1)); }
    }
    let mut input = String::new();
    io::stdin().read_to_string(&mut input).unwrap();
    match mode {
        "large-result" => io::stdout().write_all(&vec![b'x'; 1_000_000]).unwrap(),
        "slow" => {
            print!("first"); io::stdout().flush().unwrap();
            fs::write("ready", "ready").unwrap();
            while !std::path::Path::new("continue").exists() { thread::sleep(Duration::from_millis(10)); }
            print!("second");
        }
        "flood" => loop { io::stdout().write_all(&[b'x'; 8192]).unwrap(); },
        "stderr-flood" => loop { io::stderr().write_all(&[b'x'; 8192]).unwrap(); },
        "fail" => { print!("partial"); eprint!("fixture error"); std::process::exit(42); }
        _ => print!("{input}"),
    }
}
