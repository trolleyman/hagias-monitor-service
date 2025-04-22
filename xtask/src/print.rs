use std::fmt::Display;

const GREEN_BOLD: &str = "\x1b[1;32m";
const RESET: &str = "\x1b[0m";

/// Print a message in cargo style
pub fn print_cargo_style(action: impl Display, message: impl Display) {
    println!("{}{:>12} {}{}", GREEN_BOLD, action, RESET, message);
}
