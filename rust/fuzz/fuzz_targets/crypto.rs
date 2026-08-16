#![no_main]

use kadishutu::{crypto, io::MAX_INPUT_LENGTH};
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    if data.len() > MAX_INPUT_LENGTH {
        return;
    }
    let encrypted = crypto::encrypt(data);
    let decrypted = crypto::decrypt(data);
    if data.len() % 16 == 0 {
        let encrypted = encrypted.expect("aligned encryption must succeed");
        assert_eq!(crypto::decrypt(&encrypted).as_deref(), Ok(data));
        let decrypted = decrypted.expect("aligned decryption must succeed");
        assert_eq!(crypto::encrypt(&decrypted).as_deref(), Ok(data));
    } else {
        assert!(encrypted.is_err());
        assert!(decrypted.is_err());
    }
});
