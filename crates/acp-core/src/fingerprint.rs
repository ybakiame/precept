//! Frozen Python acp-worktree-v1 byte stream. No I/O or Git dependency.
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
    pub fn untracked(&mut self, raw_name: &[u8], python_st_mode: u32) -> Option<()> {
        self.files += 1;
        if self.files > MAX_UNTRACKED {
            return None;
        }
        self.digest.update(b"untracked\0");
        self.digest.update(raw_name);
        self.digest.update(python_st_mode.to_string().as_bytes());
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
