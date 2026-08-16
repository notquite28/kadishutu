#![no_main]

use kadishutu::{
    integrity,
    mutation::{MutationRequest, OwnedRange, execute_set, verify_diff_ownership},
};
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let split = data.len() / 2;
    let before = &data[..split];
    let after = &data[data.len() - split..];
    let owned = if split > 64 {
        let width = usize::from(data.first().copied().unwrap_or(0)) % (split - 64) + 1;
        vec![OwnedRange {
            start: 64,
            end: 64 + width,
        }]
    } else {
        Vec::new()
    };
    let _ = verify_diff_ownership(before, after, &owned);

    let mut hash_candidate = data.to_vec();
    let _ = integrity::update_sha1(&mut hash_candidate);

    let request = MutationRequest {
        field: String::from_utf8_lossy(data.get(..split.min(64)).unwrap_or_default()).into_owned(),
        value: data.len().to_string(),
    };
    let _ = execute_set(data.to_vec(), request);
});
