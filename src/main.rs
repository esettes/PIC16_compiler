use pic16cc::{cli::CliOptions, execute};

/// Runs the CLI compiler entrypoint and reports fatal parse or compile failures.
fn main() {
    let options = match CliOptions::parse(std::env::args().collect()) {
        Ok(options) => options,
        Err(message) => {
            eprintln!("{message}");
            std::process::exit(2);
        }
    };

    match execute(options) {
        Ok(_) => {}
        Err(diagnostics) => {
            eprintln!("{diagnostics}");
            std::process::exit(1);
        }
    }
}
