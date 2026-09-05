mod cli;
fn main() {
    if let Err(error) = cli::run() {
        eprintln!("phasecraft: {error}");
        std::process::exit(1);
    }
}
