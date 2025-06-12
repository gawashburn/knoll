#![cfg_attr(all(coverage_nightly, test), feature(coverage_attribute))]
extern crate knoll;

use coverage_helper::test;
use knoll::config::ConfigGroups;
use knoll::displays::DisplayState;
use knoll::displays::Point;
use knoll::fake_displays::FakeDisplayState;
use knoll::knoll::{run, Error};
use knoll::real_displays::*;
use ron::{
    de::from_str,
    ser::{to_string_pretty, PrettyConfig},
};
use serial_test::serial;
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
    file_err
        .read_to_string(&mut string_err)
        .expect("Failed to read stderr file.");

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
#[serial]
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
#[serial]
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
#[serial]
/// Test the default knoll command behavior with real displays.
fn test_real_default() {
    let (opt_err, stdout, stderr) = run_knoll_real(vec!["knoll", "-vvv"], None);
    // Verify that no error occurred.
    assert!(opt_err.is_none());
    // Verify that no display configuration took place.
    assert!(
        !stderr.contains("Configuration complete."),
        "Expected no configuration message in stderr, got:\n {}",
        stderr
    );
    // Run idempotency test.
    let (opt_err, stdout_new, stderr) = run_knoll_real(vec!["knoll", "-vvv"], Some(stdout.clone()));
    // Verify that no error occurred.
    assert!(opt_err.is_none());
    // Verify that display configuration did happen.
    assert!(
        stderr.contains("Configuration complete."),
        "Expected configuration message in stderr, got:\n {}",
        stderr
    );
    // Results should be unchanged.
    assert_eq!(stdout, stdout_new);
}

#[test]
#[serial]
/// Test the default knoll list command behavior with real displays.
fn test_real_list() {
    run_knoll_real(vec!["knoll", "list"], None);
}

#[test]
#[serial]
/// Test the default knoll command behavior with fake displays.
fn test_fake_default() {
    run_knoll_fake(vec!["knoll", "-vvv"], None);
}

#[test]
#[serial]
/// Test the default knoll list command behavior with fake displays.
fn test_fake_list() {
    run_knoll_fake(vec!["knoll", "list"], None);
}

#[test]
#[serial]
/// Test pipeline mode with a non-existent display UUID to trigger a
/// NoMatchingConfigGroup error.
fn test_unknown_uuid() {
    // Construct a config for a display UUID that is unlikely to exist.
    let uuid = "00000000000000000000000000000000";
    let config = format!("[ [ ( uuid: \"{}\" ) ] ]", uuid);
    let (opt_err, _stdout, _stderr) = run_knoll_real(vec!["knoll", "--format=ron"], Some(config));
    match opt_err {
        Some(Error::NoMatchingConfigGroup(uuids)) => {
            // Verify that the UUID is the one we expected.
            assert!(!uuids.contains(&uuid.to_string()));
        }
        _ => panic!("Unexpected error: {:?}", opt_err),
    }
}

#[test]
#[serial]
/// Test pipeline mode with duplicate display entries triggers a
/// DuplicateDisplays config error.
fn test_duplicate_config_entries() {
    // Construct a config with the same UUID twice to cause DuplicateDisplays.
    let uuid = "00000000000000000000000000000000";
    let entry = format!("( uuid: \"{}\" )", uuid);
    let config = format!("[ [ {}, {} ] ]", entry, entry);
    let (opt_err, _stdout, _stderr) = run_knoll_real(vec!["knoll", "--format=ron"], Some(config));
    match opt_err {
        Some(Error::Config(knoll::valid_config::Error::DuplicateDisplays(dups))) => {
            assert!(dups.contains(uuid));
        }
        _ => panic!("Expected DuplicateDisplays config error, got {:?}", opt_err),
    }
}

#[test]
#[serial]
/// Test that modifying extents of display at origin (0,0) to a huge value triggers
/// an error finding a matching mode.
fn test_extents_too_large() {
    // Obtain current configuration in RON format.
    let (opt_err, stdout, _stderr) = run_knoll_real(vec!["knoll", "--format=ron"], None);
    assert!(opt_err.is_none());
    // Deserialize the configuration for editing.
    let mut groups: ConfigGroups = from_str(&stdout).expect("Invalid RON output");
    // Modify extents for any display at origin (0,0).
    let size = 1000000;
    for group in &mut groups.groups {
        for cfg in &mut group.configs {
            if cfg.origin == Some(Point { x: 0, y: 0 }) {
                cfg.extents = Some(Point { x: size, y: size });
            }
        }
    }
    // Serialize back to RON text.
    let modified =
        to_string_pretty(&groups, PrettyConfig::default()).expect("RON serialization failed");
    // Run with modified configuration.
    let (opt_err2, _stdout2, _stderr2) =
        run_knoll_real(vec!["knoll", "--format=ron"], Some(modified));
    match opt_err2 {
        Some(Error::NoMatchingDisplayMode(_uuid, msg)) => {
            assert!(msg.contains(size.to_string().as_str()));
        }
        _ => panic!("Expected NoMatchingDisplayMode error, got {:?}", opt_err2),
    }
}
