#![no_main]

use libfuzzer_sys::fuzz_target;
use syntaxmate::Theme;

fuzz_target!(|data: &[u8]| {
    if let Ok(json) = std::str::from_utf8(data) {
        let _ = Theme::from_json(json);
    }
});
