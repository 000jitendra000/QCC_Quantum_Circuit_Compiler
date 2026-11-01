use std::env;
use std::process;

fn print_help() {
    println!("QCC — Quantum Circuit Compiler");
    println!();
    println!("Usage: qcc [OPTIONS]");
    println!();
    println!("Options:");
    println!("  -V, --version  Print version information and exit");
    println!("  -h, --help     Print help information and exit");
}

fn main() {
    let args: Vec<String> = env::args().skip(1).collect();

    if args.is_empty() {
        print_help();
        return;
    }

    match args[0].as_str() {
        "-V" | "--version" => {
            println!("{}", qcc::version_string());
            process::exit(0);
        }
        "-h" | "--help" => {
            print_help();
            process::exit(0);
        }
        unknown => {
            eprintln!("error: unrecognized argument `{unknown}`");
            eprintln!("Try `qcc --help` for more information.");
            process::exit(1);
        }
    }
}

