#![no_main]

use libfuzzer_sys::fuzz_target;
use luxtensor_tests::fuzz_targets::fuzz_consensus_message;

fuzz_target!(|data: &[u8]| {
    let _ = fuzz_consensus_message(data);
});
