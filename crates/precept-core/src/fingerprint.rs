//! Frozen acp-worktree-v1 byte stream. No I/O or Git dependency.
use sha2::{Digest, Sha256};
pub const MAX_BYTES: usize = 16 * 1024 * 1024;
pub const MAX_UNTRACKED: usize = 500;

pub struct FingerprintV1 {
    digest: Sha256,
    bytes: usize,
    files: usize,
}
impl FingerprintV1 {
    pub fn new(status: &[u8], diff: &[u8]) -> Option<Self> {
        let bytes = status.len().checked_add(diff.len())?;
        if bytes > MAX_BYTES {
            return None;
        }
        let mut digest = Sha256::new();
        digest.update(b"acp-worktree-v1\0");
        digest.update(status);
        digest.update(b"tracked-diff\0");
        digest.update(diff);
        Some(Self {
            digest,
            bytes,
            files: 0,
        })
    }
    pub fn untracked(&mut self, raw_name: &[u8], posix_mode: u32) -> Option<()> {
        self.files += 1;
        if self.files > MAX_UNTRACKED {
            return None;
        }
        self.digest.update(b"untracked\0");
        self.digest.update(raw_name);
        self.digest.update(posix_mode.to_string().as_bytes());
        Some(())
    }
    pub fn remaining(&self) -> usize {
        MAX_BYTES - self.bytes
    }
    pub fn regular_chunk(&mut self, bytes: &[u8]) -> Option<()> {
        if bytes.len() > self.remaining() {
            return None;
        }
        self.bytes += bytes.len();
        self.digest.update(bytes);
        Some(())
    }
    /// v1 does not charge symlink target bytes to its regular-file budget.
    pub fn symlink_target(&mut self, raw: &[u8]) {
        self.digest.update(raw);
    }
    pub fn finish(self) -> String {
        format!("{:x}", self.digest.finalize())
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn fixed_v1_vectors_cover_binary_unicode_raw_names_modes_and_symlink_order() {
        // Golden SHA-256 vectors derived from the frozen byte stream with Node
        // crypto, independently of this implementation. No external oracle runs.
        assert_eq!(
            FingerprintV1::new(b" M a\0", &[0, 1, 255])
                .unwrap()
                .finish(),
            "9959d6e7f79e6930e10813be68885d853c09befecb0c2a4c64508530c5d0b6ef"
        );
        let mut unicode = FingerprintV1::new("?? 中文\0".as_bytes(), b"").unwrap();
        unicode.untracked("中文".as_bytes(), 33188).unwrap();
        unicode.regular_chunk(&[0, 255, 10]).unwrap();
        assert_eq!(
            unicode.finish(),
            "1029b0d98de930b4800cd803d5e73c457f4387e7de616b24b204dd7c2e97712e"
        );
        let mut ordered = FingerprintV1::new(b"", b"").unwrap();
        ordered.untracked(b"a\xff", 41471).unwrap();
        ordered.symlink_target(b"t\xfe");
        assert_eq!(ordered.remaining(), MAX_BYTES);
        ordered.untracked(b"b", 33261).unwrap();
        ordered.regular_chunk(b"body").unwrap();
        assert_eq!(
            ordered.finish(),
            "6f6dba7aca158279d256b329a8f046a9633deafdcd572e1bb99ccefd6d86e8dd"
        );
    }
    #[test]
    fn domains_mode_order_and_exact_limits_are_part_of_v1() {
        assert_eq!(
            FingerprintV1::new(b"", b"").unwrap().finish(),
            "869b578b87029f73beda1c4cf4a76a19645cf0b93a1d4f5aa324c4e5b49c6e1b"
        );
        let mut hash = FingerprintV1::new(b"", b"").unwrap();
        for _ in 0..MAX_UNTRACKED {
            assert!(hash.untracked(b"a", 33188).is_some());
        }
        assert!(hash.untracked(b"a", 33188).is_none());
        let mut hash = FingerprintV1::new(b"a", b"b").unwrap();
        assert!(hash.regular_chunk(&vec![0; MAX_BYTES - 2]).is_some());
        assert!(hash.regular_chunk(&[0]).is_none());
        assert!(FingerprintV1::new(&vec![0; MAX_BYTES], b"x").is_none());
    }
}
