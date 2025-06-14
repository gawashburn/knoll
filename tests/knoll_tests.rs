#![cfg_attr(
    all(coverage_nightly, test),
    feature(coverage_attribute),
    coverage(off)
)]
extern crate knoll;

use knoll::config::ConfigGroups;
use knoll::displays::DisplayState;
use knoll::displays::Point;
use knoll::fake_displays::FakeDisplayState;
use knoll::knoll::{Error, run};
use knoll::real_displays::*;
use ron::{
    de::from_str,
    ser::{PrettyConfig, to_string_pretty},
};
use std::io::{Read, Write};
use std::sync::{LazyLock, Mutex};
use tempfile::tempdir;

// Mutex to prevent tests from running concurrently, as they can interfere
// with each other in various ways.
static MUTEX: LazyLock<Mutex<()>> = LazyLock::new(|| Mutex::new(()));

/// Run the knoll command with the given arguments an optional input and
/// returns whether the invoked resulted in an error and the text output
/// to stdout and stderr.
fn run_knoll<DS: DisplayState>(
    args: Vec<&str>,
    input: Option<&[u8]>,
) -> (Option<Error>, String, String) {
    // Obtain the lock before proceeding.
    let _guard = MUTEX.lock().unwrap();

    let mut vec_out: Vec<u8> = Vec::new();
    let dir = tempdir().expect("Failed to create temporary directory.");
    let err_path = dir.path().join("stderr");
    let file_err =
        std::fs::File::create(err_path.clone()).expect("Failed to open temporary file for stderr.");
    let file_err_clone = file_err.try_clone().expect("Cloning stderr file failed");
    let res = if let Some(input_slice) = input {
        let in_path = dir.path().join("stdin");
        let mut file_in = std::fs::File::create(in_path.clone())
            .expect("Failed to open temporary file for stdin.");
        file_in
            .write(input_slice)
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

/// Run the knoll command with the given arguments using real displays.
fn run_knoll_real(args: Vec<&str>, input: Option<&[u8]>) -> (Option<Error>, String, String) {
    run_knoll::<RealDisplayState>(args, input)
}

/// Run the knoll command with the given arguments using fake displays.
fn run_knoll_fake(args: Vec<&str>, input: Option<&[u8]>) -> (Option<Error>, String, String) {
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
    assert!(
        !stderr.contains("Configuration complete."),
        "Expected no configuration message in stderr, got:\n {}",
        stderr
    );
    // Run idempotency test.
    let (opt_err, stdout_new, stderr) =
        run_knoll_real(vec!["knoll", "-vvv"], Some(stdout.as_bytes()));
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

#[test]
/// Test knoll daemon mode will work with the identity configuration.
fn test_real_daemon() {
    let (opt_err, stdout, stderr) = run_knoll_real(vec!["knoll", "-vvv"], None);
    // Verify that no error occurred.
    assert!(opt_err.is_none());
    // Verify that no display configuration took place.
    assert!(
        !stderr.contains("Configuration complete."),
        "Expected no configuration message in stderr, got:\n {}",
        stderr
    );
    // Now run in daemon mod with the given configuration.
    let (opt_err, _stdout_new, stderr) = run_knoll_real(
        vec!["knoll", "-vvv", "daemon", "-e", "-w", "1s"],
        Some(stdout.as_bytes()),
    );
    // Verify that no error occurred.
    assert!(opt_err.is_none());
    // Verify that display configuration did happen.
    assert!(
        stderr.contains("Daemon mode selected") && stderr.contains("Reconfiguration successful"),
        "Expected messages in stderr, got:\n {}",
        stderr
    );
}

#[test]
/// Test pipeline mode with a non-existent display UUID to trigger a
/// NoMatchingConfigGroup error.
fn test_unknown_uuid() {
    // Construct a config for a display UUID that is unlikely to exist.
    let uuid = "00000000000000000000000000000000";
    let config = format!("[ [ ( uuid: \"{}\" ) ] ]", uuid);
    let (opt_err, _stdout, _stderr) =
        run_knoll_real(vec!["knoll", "--format=ron"], Some(config.as_bytes()));
    match opt_err {
        Some(Error::NoMatchingConfigGroup(uuids)) => {
            // Verify that the UUID is the one we expected.
            assert!(!uuids.contains(&uuid.to_string()));
        }
        _ => panic!("Unexpected error: {:?}", opt_err),
    }
}

#[test]
/// Test pipeline mode with duplicate display entries triggers a
/// DuplicateDisplays config error.
fn test_duplicate_config_entries() {
    // Construct a config with the same UUID twice to cause DuplicateDisplays.
    let uuid = "00000000000000000000000000000000";
    let entry = format!("( uuid: \"{}\" )", uuid);
    let config = format!("[ [ {}, {} ] ]", entry, entry);
    let (opt_err, _stdout, _stderr) =
        run_knoll_real(vec!["knoll", "--format=ron"], Some(config.as_bytes()));
    match opt_err {
        Some(Error::Config(knoll::valid_config::Error::DuplicateDisplays(dups))) => {
            assert!(dups.contains(uuid));
        }
        _ => panic!("Expected DuplicateDisplays config error, got {:?}", opt_err),
    }
}

#[test]
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
        run_knoll_real(vec!["knoll", "--format=ron"], Some(modified.as_bytes()));
    match opt_err2 {
        Some(Error::NoMatchingDisplayMode(_uuid, msg)) => {
            assert!(msg.contains(size.to_string().as_str()));
        }
        _ => panic!("Expected NoMatchingDisplayMode error, got {:?}", opt_err2),
    }
}

#[test]
/// Test that we will correctly detect duplicate configuration groups.
fn test_duplicate_group() {
    // Obtain current configuration in RON format.
    let (opt_err, stdout, _stderr) = run_knoll_real(vec!["knoll", "--format=ron"], None);
    assert!(opt_err.is_none());
    // Deserialize the configuration for editing.
    let mut groups: ConfigGroups = from_str(&stdout).expect("Invalid RON output");
    // Duplicate a configuration group.
    if let Some(group) = groups.groups.first().cloned() {
        groups.groups.push(group);
    } else {
        panic!("No groups found in the configuration");
    }
    // Serialize back to RON text.
    let modified =
        to_string_pretty(&groups, PrettyConfig::default()).expect("RON serialization failed");

    // Run knoll with the ambiguous configuration
    let (opt_err, _stdout, _stderr) =
        run_knoll_fake(vec!["knoll", "--format=ron"], Some(modified.as_bytes()));

    // Verify that an AmbiguousConfigGroup error was returned
    assert!(
        matches!(
            &opt_err,
            Some(Error::Config(knoll::valid_config::Error::DuplicateGroups(
                _
            )))
        ),
        "Expected DuplicateGroups error"
    );
}

#[test]
/// Test pipeline mode with invalid RON formatting triggers a deserialization error.
fn test_invalid_ron_format() {
    let bad = "invalid ron config";
    let (opt_err, _stdout, _stderr) =
        run_knoll_real(vec!["knoll", "--format=ron"], Some(bad.as_bytes()));
    assert!(
        matches!(opt_err, Some(Error::Serde(knoll::serde::Error::DeRon(_)))),
        "Expected RON deserialization error."
    );
}

#[test]
/// Test pipeline mode with invalid JSON formatting triggers a deserialization error.
fn test_invalid_json_format() {
    let bad = "{ invalid json }";
    let (opt_err, _stdout, _stderr) =
        run_knoll_real(vec!["knoll", "--format=json"], Some(bad.as_bytes()));
    assert!(
        matches!(opt_err, Some(Error::Serde(knoll::serde::Error::DeJson(_)))),
        "Expected JSON deserialization error."
    )
}

#[test]
/// Test that specifying a non-existent input file triggers an IO error.
fn test_input_file_not_found() {
    // Use a path that should not exist.
    // Create a random file suffix.
    use rand::Rng;
    let random_suffix: String = rand::rng()
        .sample_iter(&rand::distr::Alphanumeric)
        .take(5)
        .map(char::from)
        .collect();
    // Construct a non-existent file path.

    let non_existent = format!("/tmp/does_not_exist_{random_suffix}.ron");
    let (opt_err, _stdout, _stderr) =
        run_knoll_real(vec!["knoll", "--input", non_existent.as_str()], None);
    assert!(
        matches!(opt_err, Some(Error::Io(e)) if e.kind() == std::io::ErrorKind::NotFound),
        "Expected IO error for non-existent input file."
    );
}

#[test]
/// Test parsing of a valid wait time argument for daemon command.
fn test_wait_arg_parsing_valid() {
    let (opt_err, _stdout, _stderr) =
        run_knoll_real(vec!["knoll", "daemon", "--wait", "5 blips"], None);
    assert!(
        matches!(opt_err, Some(Error::Duration(_))),
        "Expected Duration error for invalid wait time."
    );
}

#[test]
/// Test that a RON input configuration containing invalid UTF-8 data is properly handled.
fn test_invalid_utf8_in_ron() {
    // Create a Vec<u8> containing valid start, invalid UTF-8 bytes, and valid end.
    let mut bytes = "(groups: [".as_bytes().to_vec();
    bytes.extend_from_slice(&[0xFF, 0xFE, 0xFD, 0xFC]); // Invalid UTF-8 bytes
    bytes.extend_from_slice("])".as_bytes());

    // Run knoll with the file path as input
    let (opt_err, _stdout, _stderr) =
        run_knoll_real(vec!["knoll", "--format=ron"], Some(bytes.as_slice()));

    // Verify that a deserialization error occurred (either Serde error or IO error with invalid data)
    assert!(
        matches!(&opt_err, Some(Error::Utf8(_))),
        "Expected error due to invalid UTF-8 data."
    );
}

#[test]
/// Test that passing an invalid argument to knoll results in an Argument error
fn test_invalid_argument() {
    let (opt_err, _stdout, _stderr) = run_knoll_real(vec!["knoll", "--bogus-argument"], None);

    assert!(
        matches!(&opt_err, Some(Error::Argument(arg)) if arg.kind() == clap::error::ErrorKind::UnknownArgument),
        "Expected an invalid argument error"
    );
}
