// Standalone, cross-platform native executable. No shell or provider account required.
use std::{fs, io::{self, Read}};
fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    fs::write("argv", args.join("\0")).unwrap();
    let mut input = String::new();
    io::stdin().read_to_string(&mut input).unwrap();
    fs::write("stdin", input).unwrap();
    if std::env::var_os("CONTRACT_FAIL").is_some() { eprint!("contract failure"); std::process::exit(42); }
    if args.iter().any(|a| a == "--input-format") {
        println!("{{\"event\":\"result\",\"result\":{{\"status\":\"SUCCESS\",\"response\":\"OK\"}}}}");
    } else if args.iter().any(|a| a == "stream-json") {
        println!("{{\"type\":\"result\",\"is_error\":false,\"result\":\"OK\"}}");
    } else { print!("OK"); }
}
