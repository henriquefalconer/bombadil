pub struct KeyInfo {
    pub name: &'static str,
    pub text: &'static str,
}

pub fn key_info(code: u8) -> Option<KeyInfo> {
    match code {
        8 => Some(KeyInfo {
            name: "Backspace",
            text: "",
        }),
        9 => Some(KeyInfo {
            name: "Tab",
            text: "",
        }),
        13 => Some(KeyInfo {
            name: "Enter",
            text: "\r",
        }),
        27 => Some(KeyInfo {
            name: "Escape",
            text: "",
        }),
        37 => Some(KeyInfo {
            name: "ArrowLeft",
            text: "",
        }),
        38 => Some(KeyInfo {
            name: "ArrowUp",
            text: "",
        }),
        39 => Some(KeyInfo {
            name: "ArrowRight",
            text: "",
        }),
        40 => Some(KeyInfo {
            name: "ArrowDown",
            text: "",
        }),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_backspace_has_no_text() {
        let info = key_info(8).unwrap();
        assert_eq!(info.name, "Backspace");
        assert_eq!(info.text, "");
    }

    #[test]
    fn test_tab_has_no_text() {
        let info = key_info(9).unwrap();
        assert_eq!(info.name, "Tab");
        assert_eq!(info.text, "");
    }

    #[test]
    fn test_enter_has_text() {
        let info = key_info(13).unwrap();
        assert_eq!(info.name, "Enter");
        assert_eq!(info.text, "\r");
    }

    #[test]
    fn test_escape_has_no_text() {
        let info = key_info(27).unwrap();
        assert_eq!(info.name, "Escape");
        assert_eq!(info.text, "");
    }

    #[test]
    fn test_arrow_keys_have_no_text() {
        for (code, name) in [
            (37, "ArrowLeft"),
            (38, "ArrowUp"),
            (39, "ArrowRight"),
            (40, "ArrowDown"),
        ] {
            let info = key_info(code).unwrap();
            assert_eq!(info.name, name);
            assert_eq!(info.text, "");
        }
    }

    #[test]
    fn test_unknown_codes_return_none() {
        assert!(key_info(0).is_none());
        assert!(key_info(255).is_none());
    }
}
