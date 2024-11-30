#![cfg_attr(all(coverage_nightly, test), feature(coverage_attribute))]
extern crate knoll;

use coverage_helper::test;
use knoll::knoll::run;
use knoll::real_displays::*;
use tempfile;

#[test]
fn test_help() {
    let mut vec_out: Vec<u8> = Vec::new();
    let file_err = tempfile::tempfile().expect("Failed to create temporary file.");
    let args = vec!["knoll", "help"];
    let _ = run::<RealDisplayState, std::io::Stdin, &mut Vec<u8>, std::fs::File>(
        &args.into_iter().map(|s| String::from(s)).collect(),
        std::io::stdin(),
        &mut vec_out,
        file_err.try_clone().expect("Clone failed"),
    );
}

#[test]
fn test_list() {
    let mut vec_out: Vec<u8> = Vec::new();
    let file_err = tempfile::tempfile().expect("Failed to create temporary file.");
    let args = vec!["knoll", "list"];
    let _ = run::<RealDisplayState, std::io::Stdin, &mut Vec<u8>, std::fs::File>(
        &args.into_iter().map(|s| String::from(s)).collect(),
        std::io::stdin(),
        &mut vec_out,
        file_err.try_clone().expect("Clone failed"),
    );
}
