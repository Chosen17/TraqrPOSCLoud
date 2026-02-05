//! Shared helpers (e.g. slug generation).

pub fn slug_from_title(title: &str) -> String {
    let s: String = title
        .chars()
        .map(|c| match c {
            'A'..='Z' => char::from(c as u8 + 32),
            'a'..='z' | '0'..='9' => c,
            ' ' | '-' | '_' => '-',
            _ => '\0',
        })
        .filter(|c| *c != '\0')
        .collect();
    s.split('-').filter(|p| !p.is_empty()).collect::<Vec<_>>().join("-")
}
