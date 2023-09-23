// Objective-C is needed by the core_graphics module.
// The core_graphics module also uses static_assertions to validate
// some data structures at compile time.
#[macro_use]
extern crate objc;
extern crate static_assertions;

mod config;
mod core_graphics;
mod displays;
mod fake_displays;
mod knoll;
mod real_displays;
mod serde;
mod valid_config;

use std::io::Write;

use real_displays::RealDisplayState;

/// Main entry point for the knoll command-line tool.  
/// Most everything happens in the knoll module, as ii has been
/// parameterized to make testing easier.
pub fn main() {
    // Dispatch to the run function.  As this is entry point to the real
    // program, we use the actual stdin, stdout and RealDisplayState.
    let mut stdin = std::io::stdin();
    let mut stdout = std::io::stdout();
    let args: Vec<String> = std::env::args().into_iter().collect();
    match knoll::run::<RealDisplayState, std::io::Stdin, std::io::Stdout>(
        &args,
        &mut stdin,
        &mut stdout,
    ) {
        // Hit an error, print it to stderr.
        Err(e) => {
            write!(std::io::stderr(), "{}", e).unwrap();
            std::process::exit(1);
        }
        // Everything went as expected.
        Ok(_) => {
            std::process::exit(0);
        }
    }
}
