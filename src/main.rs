#![forbid(unsafe_code)]

use std::process::exit;

fn main() {
    let mut stdout = std::io::stdout();
    let mut stderr = std::io::stderr();
    let code = serde_saphyr::cli::run(std::env::args().skip(1), &mut stdout, &mut stderr);
    exit(code);
}
