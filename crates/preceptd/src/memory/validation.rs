use precept_ipc::Error;

/// Bounded text: trimmed, no control characters but line breaks and tabs.
pub(super) fn text(raw: &str, max: usize) -> Result<String, Error> {
    let t = raw.trim();
    if t.is_empty()
        || t.chars().count() > max
        || t.chars().any(|c| c.is_control() && c != '\n' && c != '\t')
    {
        return Err(Error::InvalidRequest);
    }
    Ok(t.to_owned())
}
/// A title as compared: lowercase, runs of space as one.
pub(super) fn normal(title: &str) -> String {
    title
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
}
pub(super) fn new_id() -> Result<String, Error> {
    let mut raw = [0u8; 8];
    getrandom::fill(&mut raw).map_err(|_| Error::Unavailable)?;
    Ok(raw.iter().map(|b| format!("{b:02x}")).collect())
}
pub(super) fn valid_id(id: &str) -> bool {
    id.len() == 16 && id.bytes().all(|b| b.is_ascii_hexdigit())
}
