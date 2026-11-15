// Trusted, dependency-free result codec. Candidate source never receives host I/O.
#[allow(dead_code)]
struct Wire(String);
#[allow(dead_code)]
impl Wire {
    fn text(&mut self, value: &str) {
        if self.0.len().saturating_add(value.len()) > 8192 { std::process::exit(67); }
        self.0.push_str(value);
    }
    fn string(&mut self, value: &str) {
        if value.len() > 4096 { std::process::exit(67); }
        self.text("\"");
        for ch in value.chars() {
            match ch {
                '"' => self.text("\\\""), '\\' => self.text("\\\\"),
                ch if ch.is_control() => self.text(&format!("\\u{:04x}",ch as u32)),
                ch => self.text(ch.encode_utf8(&mut [0;4])),
            }
        }
        self.text("\"");
    }
}
