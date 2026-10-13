mod cli;
mod color;
mod error;
mod render;

fn main() {
    if let Err(err) = cli::execute() {
        eprintln!("error: {err}");
        std::process::exit(1);
    }
}
