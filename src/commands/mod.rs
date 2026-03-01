pub mod auth;
pub mod payload;
pub mod runs;
pub mod secrets;
pub mod tasks;

use std::io::{self, Write};

use crate::errors::CliError;

pub fn prompt_confirm(message: &str) -> Result<bool, CliError> {
    print!("{message} [y/N]: ");
    io::stdout().flush()?;

    let mut input = String::new();
    io::stdin().read_line(&mut input)?;
    let normalized = input.trim().to_ascii_lowercase();

    Ok(normalized == "y" || normalized == "yes")
}
