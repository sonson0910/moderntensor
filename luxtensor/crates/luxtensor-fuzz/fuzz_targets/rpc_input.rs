#![no_main]

use libfuzzer_sys::fuzz_target;
use luxtensor_tests::fuzz_targets::fuzz_rpc_json;

fuzz_target!(|data: &[u8]| {
    let _ = fuzz_rpc_json(data);
});
