pub struct KeyInfo {
    pub name: &'static str,
    pub text: &'static str,
}

pub fn key_info(code: u8) -> Option<KeyInfo> {
    match code {
        8 => Some(KeyInfo {
            name: "Backspace",
            text: "\u{0008}",
        }),
        9 => Some(KeyInfo {
            name: "Tab",
            text: "\t",
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
