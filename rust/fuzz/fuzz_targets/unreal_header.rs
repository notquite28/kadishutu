#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let _ = kadishutu::format::unreal::parse_header(data);
});
