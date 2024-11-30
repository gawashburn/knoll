#![cfg_attr(all(coverage_nightly, test), feature(coverage_attribute))]
extern crate knoll;

use coverage_helper::test;
use knoll::displays::DisplayState;
use knoll::fake_displays::FakeDisplayState;
use knoll::knoll::run;
use knoll::real_displays::*;
use std::io::Read;
use tempfile::tempdir;

#[cfg_attr(all(coverage_nightly, test), coverage(off))]
/// Run the knoll command with the given arguments and return the output to
/// stdout and stderr.
/// The only caveat is that it is currently not possible to override output
/// destinations for the clap argument parser when using `--help` or
/// `--version`.  So at present we cannot capture the output of these options.
fn run_knoll<DS: DisplayState>(args: Vec<&str>) -> (String, String) {
    let mut vec_out: Vec<u8> = Vec::new();
    let dir = tempdir().expect("Failed to create temporary directory.");
    let path = dir.path().join("stderr");
    let file_err = std::fs::File::create(path.clone()).expect("Failed to open temporary file.");
    let _ = run::<DS, std::io::Stdin, &mut Vec<u8>, std::fs::File>(
        &args.into_iter().map(|s| String::from(s)).collect(),
        std::io::stdin(),
        &mut vec_out,
        file_err.try_clone().expect("Clone failed"),
    );

    let mut file_err = std::fs::File::open(path).expect("Failed to open temporary file.");
    let mut string_err = String::new();
    file_err.read_to_string(&mut string_err).unwrap();

    (String::from_utf8(vec_out).unwrap(), string_err)
}

#[cfg_attr(all(coverage_nightly, test), coverage(off))]
/// Run the knoll commmand with the given arguments using real displays.
fn run_knoll_real(args: Vec<&str>) -> (String, String) {
    run_knoll::<RealDisplayState>(args)
}

#[cfg_attr(all(coverage_nightly, test), coverage(off))]
/// Run the knoll commmand with the given arguments using fake displays.
fn run_knoll_fake(args: Vec<&str>) -> (String, String) {
    run_knoll::<FakeDisplayState>(args)
}

#[test]
/// Test the knoll --help command
fn test_help() {
    run_knoll_real(vec!["knoll", "--help"]);
}

#[test]
/// Test the knoll --version command
fn test_version() {
    run_knoll_real(vec!["knoll", "--version"]);
}

#[test]
/// Test the default knoll command behavior with real displays.
fn test_real_default() {
    run_knoll_real(vec!["knoll", "-vvv"]);
}

#[test]
/// Test the default knoll list command behavior with real displays.
fn test_real_list() {
    run_knoll_real(vec!["knoll", "list"]);
}

#[test]
/// Test the default knoll command behavior with fake displays.
fn test_fake_default() {
    run_knoll_fake(vec!["knoll", "-vvv"]);
}

#[test]
/// Test the default knoll list command behavior with fake displays.
fn test_fake_list() {
    run_knoll_fake(vec!["knoll", "list"]);
}
