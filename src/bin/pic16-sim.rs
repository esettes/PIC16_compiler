// SPDX-License-Identifier: GPL-3.0-or-later

use pic16cc::sim_cli::{SimCliError, SimCliOptions, run};

fn main() {
    let options = match SimCliOptions::parse(std::env::args().collect()) {
        Ok(options) => options,
        Err(error) => exit_with_error(error, 2),
    };

    match run(options) {
        Ok(output) => print!("{output}"),
        Err(error) => exit_with_error(error, 1),
    }
}

fn exit_with_error(error: SimCliError, code: i32) -> ! {
    eprintln!("pic16-sim: error: {error}");
    std::process::exit(code);
}
