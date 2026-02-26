pub struct KeyInfo {
    pub code: &'static str,
    pub key: &'static str,
    pub text: &'static str,
}

/// All key codes supported by Bombadil. Must match `keycodes()` in
/// `src/specification/random.ts` — that list is the TypeScript side of this
/// cross-boundary contract.
pub const SUPPORTED_KEY_CODES: &[u8] = &[8, 9, 13, 27, 37, 38, 39, 40];

pub fn key_info(code: u8) -> Option<KeyInfo> {
    // NOTE: For this set of special keys `code` and `key` happen to be
    // identical strings. This is correct per CDP spec for named keys
    // (Backspace, Tab, Enter, Escape, Arrow*). For other key categories
    // they must diverge — do NOT copy this pattern blindly:
    //   • Printable chars: code=49 → code:"Digit1", key:"1"
    //   • Modifiers:       code=16 → code:"ShiftLeft", key:"Shift"
    //   • Numpad:          code=96 → code:"Numpad0", key:"0"
    match code {
        8 => Some(KeyInfo {
            code: "Backspace",
            key: "Backspace",
            text: "",
        }),
        9 => Some(KeyInfo {
            code: "Tab",
            key: "Tab",
            text: "",
        }),
        13 => Some(KeyInfo {
            code: "Enter",
            key: "Enter",
            text: "\r",
        }),
        27 => Some(KeyInfo {
            code: "Escape",
            key: "Escape",
            text: "",
        }),
        37 => Some(KeyInfo {
            code: "ArrowLeft",
            key: "ArrowLeft",
            text: "",
        }),
        38 => Some(KeyInfo {
            code: "ArrowUp",
            key: "ArrowUp",
            text: "",
        }),
        39 => Some(KeyInfo {
            code: "ArrowRight",
            key: "ArrowRight",
            text: "",
        }),
        40 => Some(KeyInfo {
            code: "ArrowDown",
            key: "ArrowDown",
            text: "",
        }),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn backspace_has_no_text() {
        let info = key_info(8).unwrap();
        assert_eq!(info.code, "Backspace");
        assert_eq!(info.key, "Backspace");
        assert_eq!(info.text, "");
    }

    #[test]
    fn tab_has_no_text() {
        let info = key_info(9).unwrap();
        assert_eq!(info.code, "Tab");
        assert_eq!(info.key, "Tab");
        assert_eq!(info.text, "");
    }

    #[test]
    fn enter_has_text() {
        let info = key_info(13).unwrap();
        assert_eq!(info.code, "Enter");
        assert_eq!(info.key, "Enter");
        assert_eq!(info.text, "\r");
    }

    #[test]
    fn escape_has_no_text() {
        let info = key_info(27).unwrap();
        assert_eq!(info.code, "Escape");
        assert_eq!(info.key, "Escape");
        assert_eq!(info.text, "");
    }

    #[test]
    fn arrow_keys_have_no_text() {
        for (code, name) in [
            (37, "ArrowLeft"),
            (38, "ArrowUp"),
            (39, "ArrowRight"),
            (40, "ArrowDown"),
        ] {
            let info = key_info(code).unwrap();
            assert_eq!(info.code, name);
            assert_eq!(info.key, name);
            assert_eq!(info.text, "");
        }
    }

    #[test]
    fn unknown_codes_return_none() {
        assert!(key_info(0).is_none());
        assert!(key_info(255).is_none());
    }

    #[test]
    fn all_supported_codes_have_key_info() {
        for &code in SUPPORTED_KEY_CODES {
            assert!(
                key_info(code).is_some(),
                "key_info({code}) returned None but {code} is in SUPPORTED_KEY_CODES"
            );
        }
    }
}
