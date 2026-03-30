#[expect(clippy::print_stderr)]
fn main() {
    handsome_logger::init().unwrap_or_default();
    if let Err(error) = space_finder::run() {
        log::error!("Failed to start Space Finder: {error}");
        eprintln!("Failed to start Space Finder: {error}");
    }
}
