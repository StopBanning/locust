//! The standalone Locust command. The window and engine stay in their own process.
fn main() {
    let args: Vec<_> = std::env::args().skip(1).collect();
    std::process::exit(locust_core::cli_client::run_process(&args));
}
