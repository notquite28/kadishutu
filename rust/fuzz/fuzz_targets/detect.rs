#![no_main]

use kadishutu::{format::detect::validate_bytes, io::MAX_INPUT_LENGTH};
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let _ = validate_bytes(data);
    if data.first().is_some_and(|value| value & 1 != 0) {
        let mut exact_length = vec![0_u8; MAX_INPUT_LENGTH];
        let prefix_length = data.len().min(MAX_INPUT_LENGTH);
        exact_length[..prefix_length].copy_from_slice(&data[..prefix_length]);
        let _ = validate_bytes(&exact_length);
    }
});
