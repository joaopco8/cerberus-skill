//! Terminal banner for cerberus CLI.
//!
//! Prints a block-style "CERBERUS" header with a smooth horizontal
//! purple→violet→cyan gradient computed per column position across the
//! full line width, using owo-colors 256-colour xterm escape codes.

use owo_colors::{OwoColorize, XtermColors};

/// Gradient anchor points in xterm 6×6×6 cube coordinates (r, g, b ∈ 0–5).
/// Cube index = 16 + r*36 + g*6 + b.  Value map: 0→0, 1→95, 2→135, 3→175, 4→215, 5→255.
///
/// Path: 93 (deep purple) → 135 (violet) → 51 (bright cyan), with 8 intermediate
/// shades so the gradient field is continuous rather than hard-banded.
const GRAD: &[(u8, u8, u8)] = &[
    (2, 0, 5), // 93  deep purple
    (2, 1, 5), // 99  blue-purple
    (2, 2, 5), // 105 periwinkle
    (3, 1, 5), // 135 violet
    (3, 2, 5), // 141 soft violet
    (3, 3, 5), // 147 lavender
    (3, 4, 5), // 153 ice blue
    (3, 5, 5), // 159 pale cyan
    (2, 5, 5), // 123 medium cyan
    (1, 5, 5), // 87  bright cyan
    (0, 5, 5), // 51  pure cyan
];

/// Prints the Cerberus block-style gradient banner to stdout.
///
/// The gradient is computed per-column against the widest line so the
/// purple→violet→cyan transition is a single consistent field across all
/// 6 rows, not 6 independent per-line gradients restarting at column 0.
pub fn print_banner() {
    let lines = [
        " ██████╗███████╗██████╗ ██████╗ ███████╗██████╗ ██╗   ██╗███████╗",
        "██╔════╝██╔════╝██╔══██╗██╔══██╗██╔════╝██╔══██╗██║   ██║██╔════╝",
        "██║     █████╗  ██████╔╝██████╔╝█████╗  ██████╔╝██║   ██║███████╗",
        "██║     ██╔══╝  ██╔══██╗██╔══██╗██╔══╝  ██╔══██╗██║   ██║╚════██║",
        "╚██████╗███████╗██║  ██║██████╔╝███████╗██║  ██║╚██████╔╝███████║",
        " ╚═════╝╚══════╝╚═╝  ╚═╝╚═════╝ ╚══════╝╚═╝  ╚═╝ ╚═════╝ ╚══════╝",
    ];

    let max_width = lines.iter().map(|l| l.chars().count()).max().unwrap_or(1);
    // total is the denominator for t = col / total → [0.0, 1.0]
    let total = (max_width.saturating_sub(1)).max(1) as f32;
    let steps = (GRAD.len() - 1) as f32;

    for line in &lines {
        for (col, ch) in line.chars().enumerate() {
            if ch == ' ' {
                print!(" ");
            } else {
                // Map column position to a 0.0–1.0 gradient position shared
                // across all lines (uses max_width, not this line's width).
                let t = col as f32 / total;
                let fi = t * steps;
                let lo = fi.floor() as usize;
                let hi = (lo + 1).min(GRAD.len() - 1);
                let frac = fi - fi.floor();

                let (r0, g0, b0) = GRAD[lo];
                let (r1, g1, b1) = GRAD[hi];

                // Interpolate in cube-coord space, then map to xterm index.
                // This avoids the wrong-color problem of lerping raw indices.
                let r = (r0 as f32 * (1.0 - frac) + r1 as f32 * frac).round() as u8;
                let g = (g0 as f32 * (1.0 - frac) + g1 as f32 * frac).round() as u8;
                let b = (b0 as f32 * (1.0 - frac) + b1 as f32 * frac).round() as u8;
                let color_idx: u8 = 16 + r * 36 + g * 6 + b;

                print!("{}", ch.color(XtermColors::from(color_idx)).bold());
            }
        }
        println!();
    }
    println!(
        "{}",
        "        on-chain governed spending limits for AI agents".dimmed()
    );
    println!();
}
