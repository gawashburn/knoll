extern crate knoll;

use knoll::knoll::run;
use knoll::real_displays::*;
use tempfile;

#[test]
fn tmp_test() {
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
