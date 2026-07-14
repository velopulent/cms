mod arch;
mod artifact;
mod cli;
mod linux;
mod macos;
mod model;
mod package;
mod portable;
mod shared;
mod windows;

fn main() {
    if let Err(error) = cli::run() {
        eprintln!("error: {error}");
        std::process::exit(1);
    }
}
