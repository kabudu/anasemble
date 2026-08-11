use std::path::Path;
use std::process::ExitCode;

fn main() -> ExitCode {
    let command = std::env::args().nth(1).unwrap_or_else(|| "check".into());
    let result = match command.as_str() {
        "generate" => anasemble::brand::generate(Path::new(".")),
        "check" => anasemble::brand::validate(Path::new(".")),
        _ => Err("usage: brand_assets [generate|check]".into()),
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("brand asset validation failed: {error}");
            ExitCode::FAILURE
        }
    }
}
