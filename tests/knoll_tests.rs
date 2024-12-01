#![cfg_attr(all(coverage_nightly, test), feature(coverage_attribute))]
extern crate knoll;

use coverage_helper::test;
use knoll::displays::DisplayState;
use knoll::fake_displays::FakeDisplayState;
use knoll::knoll::{run, Error};
use knoll::real_displays::*;
use std::io::{Read, Write};
use tempfile::tempdir;

#[cfg_attr(all(coverage_nightly, test), coverage(off))]
/// Run the knoll command with the given arguments an optional input and
/// returns whether the invoked resulted in an error and the text output
/// to stdout and stderr.
fn run_knoll<DS: DisplayState>(
    args: Vec<&str>,
    input: Option<String>,
) -> (Option<Error>, String, String) {
    let mut vec_out: Vec<u8> = Vec::new();
    let dir = tempdir().expect("Failed to create temporary directory.");
    let err_path = dir.path().join("stderr");
    let file_err =
        std::fs::File::create(err_path.clone()).expect("Failed to open temporary file for stderr.");
    let file_err_clone = file_err.try_clone().expect("Cloning stderr file failed");
    let res = if let Some(input_str) = input {
        let in_path = dir.path().join("stdin");
        let mut file_in = std::fs::File::create(in_path.clone())
            .expect("Failed to open temporary file for stdin.");
        file_in
            .write(input_str.as_bytes())
            .expect("Failed to write to file.");
        file_in.flush().expect("Failed to flush file.");
        // Need to reopen the file for knoll to read from it.
        let file_in =
            std::fs::File::open(in_path).expect("Failed to open temporary file for stdin.");
        run::<DS, std::fs::File, &mut Vec<u8>, std::fs::File>(
            &args.into_iter().map(|s| String::from(s)).collect(),
            file_in,
            &mut vec_out,
            file_err_clone,
        )
    } else {
        run::<DS, std::io::Stdin, &mut Vec<u8>, std::fs::File>(
            &args.into_iter().map(|s| String::from(s)).collect(),
            std::io::stdin(),
            &mut vec_out,
            file_err_clone,
        )
    };
    // Convert the result to an option
    let opt_error = match res {
        Ok(_) => None,
        Err(e) => Some(e),
    };

    let mut file_err =
        std::fs::File::open(err_path).expect("Failed to open temporary file for stderr.");
    let mut string_err = String::new();
    file_err.read_to_string(&mut string_err).unwrap();

    (opt_error, String::from_utf8(vec_out).unwrap(), string_err)
}

#[cfg_attr(all(coverage_nightly, test), coverage(off))]
/// Run the knoll command with the given arguments using real displays.
fn run_knoll_real(args: Vec<&str>, input: Option<String>) -> (Option<Error>, String, String) {
    run_knoll::<RealDisplayState>(args, input)
}

#[cfg_attr(all(coverage_nightly, test), coverage(off))]
/// Run the knoll command with the given arguments using fake displays.
fn run_knoll_fake(args: Vec<&str>, input: Option<String>) -> (Option<Error>, String, String) {
    run_knoll::<FakeDisplayState>(args, input)
}

#[test]
/// Test the knoll --help command
fn test_help() {
    let (opt_err, _, _) = run_knoll_real(vec!["knoll", "--help"], None);
    // Verify that a help error was produced.
    match opt_err {
        Some(Error::Argument(e)) => assert_eq!(e.kind(), clap::error::ErrorKind::DisplayHelp),
        _ => panic!("Unexpected error: {:?}", opt_err),
    }
}

#[test]
/// Test the knoll --version command
fn test_version() {
    let (opt_err, _, _) = run_knoll_real(vec!["knoll", "--version"], None);
    // Verify that a version error was produced.
    match opt_err {
        Some(Error::Argument(e)) => assert_eq!(e.kind(), clap::error::ErrorKind::DisplayVersion),
        _ => panic!("Unexpected error: {:?}", opt_err),
    }
}

#[test]
/// Test the default knoll command behavior with real displays.
fn test_real_default() {
    let (opt_err, stdout, stderr) = run_knoll_real(vec!["knoll", "-vvv"], None);
    // Verify that no error occurred.
    assert!(opt_err.is_none());
    // Verify that no display configuration took place.
    assert!(!stderr.contains("Configuration complete."));
    // Run idempotency test.
    let (opt_err, stdout_new, stderr) = run_knoll_real(vec!["knoll", "-vvv"], Some(stdout.clone()));
    // Verify that no error occurred.
    assert!(opt_err.is_none());
    // Verify that display configuration did happen.
    assert!(stderr.contains("Configuration complete."));
    // Results should be unchanged.
    assert_eq!(stdout, stdout_new);
}

#[test]
/// Test the default knoll list command behavior with real displays.
fn test_real_list() {
    run_knoll_real(vec!["knoll", "list"], None);
}

#[test]
/// Test the default knoll command behavior with fake displays.
fn test_fake_default() {
    run_knoll_fake(vec!["knoll", "-vvv"], None);
}

#[test]
/// Test the default knoll list command behavior with fake displays.
fn test_fake_list() {
    run_knoll_fake(vec!["knoll", "list"], None);
}
