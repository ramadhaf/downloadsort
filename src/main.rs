use std::process::ExitCode;

fn main() -> ExitCode {
    match downsort::run_cli(std::env::args().skip(1)) {
        Ok(output) => {
            if !output.is_empty() {
                print!("{output}");
            }
            ExitCode::SUCCESS
        }
        Err(err) => {
            eprintln!("Error: {err}");
            ExitCode::FAILURE
        }
    }
}
