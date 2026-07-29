use sha2::{Digest, Sha256};

/// 为章节正文生成稳定的业务摘要。
///
/// 摘要只用于判断分析结果是否仍属于当前正文，不保存正文，也不承担密码学签名职责。
pub(crate) fn chapter_content_digest(content: &str) -> String {
    format!("sha256:{}", hex::encode(Sha256::digest(content.as_bytes())))
}

#[cfg(test)]
mod tests {
    use super::chapter_content_digest;

    #[test]
    fn digest_is_stable_and_content_sensitive() {
        let first = chapter_content_digest("同一章节正文");
        let repeated = chapter_content_digest("同一章节正文");
        let changed = chapter_content_digest("已修改章节正文");

        assert_eq!(first, repeated);
        assert_ne!(first, changed);
        assert!(first.starts_with("sha256:"));
        assert_eq!(first.len(), "sha256:".len() + 64);
    }
}
