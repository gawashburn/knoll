extern crate knoll;

use knoll::knoll::run;
use knoll::real_displays::*;

#[test]
fn tmp_test() {
    let mut vecout: Vec<u8> = Vec::new();
    let args = vec![];
    let _ =
        run::<RealDisplayState, std::io::Stdin, Vec<u8>>(&args, &mut std::io::stdin(), &mut vecout);
}
