#![cfg_attr(all(coverage_nightly, test), feature(coverage_attribute))]
///! Expose the knoll modules as part of a library.  This is only really
/// necessary so that they will be visible in the `test` directory.
#[macro_use]
extern crate objc;
extern crate static_assertions;

pub mod config;
pub mod core_graphics;
pub mod displays;
pub mod fake_displays;
pub mod indirect_logger;
pub mod knoll;
pub mod real_displays;
mod serde;
pub mod valid_config;
