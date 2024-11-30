#![cfg_attr(all(coverage_nightly, test), feature(coverage_attribute))]
extern crate knoll;

use coverage_helper::test;
use knoll::knoll::run;
use knoll::real_displays::*;
use tempfile;

#[cfg_attr(all(coverage_nightly, test), coverage(off))]
fn run_knoll(args: Vec<&str>) -> (String, String) {
    let mut vec_out: Vec<u8> = Vec::new();
    let file_err = tempfile::tempfile().expect("Failed to create temporary file.");
    let _ = run::<RealDisplayState, std::io::Stdin, &mut Vec<u8>, std::fs::File>(
        &args.into_iter().map(|s| String::from(s)).collect(),
        std::io::stdin(),
        &mut vec_out,
        file_err.try_clone().expect("Clone failed"),
    );

    (
        String::from_utf8(vec_out).unwrap(),
        String::from("".to_string()),
    )
}
#[test]
fn test_default() {
    run_knoll(vec!["knoll"]);
}

#[test]
fn test_help() {
    run_knoll(vec!["knoll", "help"]);
}

#[test]
fn test_list() {
    run_knoll(vec!["knoll", "list"]);
}
