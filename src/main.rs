use std::env;
use std::process;

fn main() {
    let args: Vec<String> = env::args().skip(1).collect();
    let code = qcc::cli::run(&args);
    process::exit(code.as_i32());
}
