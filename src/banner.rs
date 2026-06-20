//! Terminal banner for cerberus-skill CLI examples.
//!
//! Prints a colour-gradient "CERBERUS" header using 256-colour xterm codes
//! via the [`owo-colors`](https://docs.rs/owo-colors) crate.
//! Falls back gracefully if the terminal does not support colour.

use owo_colors::{OwoColorize, XtermColors};

/// Prints the Cerberus gradient banner to stdout.
///
/// Uses xterm 256-colour escape codes. Safe to call in any terminal; if
/// the terminal does not support colour the text prints without formatting.
pub fn print_banner() {
    // "CERBERUS" split into characters, each coloured across a
    // purple→violet→cyan gradient (xterm indices 93→99→45).
    // Indices map roughly to: 93=purple, 99=blue-violet, 45=cyan.
    let chars = ['C', 'E', 'R', 'B', 'E', 'R', 'U', 'S'];
    let colors: [u8; 8] = [93, 135, 141, 147, 153, 117, 81, 45];

    print!("  ");
    for (ch, color) in chars.iter().zip(colors.iter()) {
        print!(
            "{}",
            format!(" {ch} ").color(XtermColors::from(*color)).bold()
        );
    }
    println!();
    println!(
        "{}",
        "        on-chain governed spending limits for AI agents".dimmed()
    );
    println!();
}
