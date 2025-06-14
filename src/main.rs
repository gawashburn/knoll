#![cfg_attr(all(coverage_nightly, test), feature(coverage_attribute))]
// Objective-C is needed by the core_graphics module.
// The core_graphics module also uses static_assertions to validate
// some data structures at compile time.
extern crate static_assertions;

mod config;
mod core_graphics;
mod displays;
mod fake_displays;
pub mod indirect_logger;
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
    let args: Vec<String> = std::env::args().into_iter().collect();
    match knoll::run::<RealDisplayState, std::io::Stdin, std::io::Stdout, std::io::Stderr>(
        &args,
        std::io::stdin(),
        std::io::stdout(),
        std::io::stderr(),
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
