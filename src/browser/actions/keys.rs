pub fn key_name(code: u8) -> Option<&'static str> {
    match code {
        13 => Some("Enter"),
        _ => None,
    }
}
