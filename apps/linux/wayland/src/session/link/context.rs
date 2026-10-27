//! 应用在 Server 上的上下文标识（`LinuxHello.context`）。

/// 不知道是哪个应用时的上下文：所有输入框共用这一个会话。
const SHARED: &str = "wayland";

/// 应用的上下文标识：`wayland-` 加 app_id 的 64 位 FNV-1a 哈希（16 位十六进制），同一个应用每次都一样；
/// 不知道是哪个应用就是 `wayland`。Server 要求它非空、不超过 64 字节、同一条连接上不重复，
/// 而 app_id 可以很长、可以带任意字符，所以不原样用。
pub fn context(app: Option<&str>) -> String {
    match app {
        Some(app) => format!("{SHARED}-{:016x}", fnv1a(app)),
        None => SHARED.to_owned(),
    }
}

/// 64 位 FNV-1a：算法写死，换 Rust 版本也不变（标准库的哈希不保证这一点）。
fn fnv1a(text: &str) -> u64 {
    text.bytes().fold(0xcbf2_9ce4_8422_2325, |hash, byte| {
        (hash ^ u64::from(byte)).wrapping_mul(0x0100_0000_01b3)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 对得上 FNV-1a 公开的测试值：哈希写死了，同一个应用重启前后、换版本前后都是同一个上下文。
    #[test]
    fn the_hash_is_plain_fnv1a() {
        assert_eq!(fnv1a(""), 0xcbf2_9ce4_8422_2325);
        assert_eq!(fnv1a("a"), 0xaf63_dc4c_8601_ec8c);
        assert_eq!(fnv1a("foobar"), 0x8594_4171_f739_67e8);
        assert_eq!(context(Some("a")), "wayland-af63dc4c8601ec8c");
    }

    /// 再长的 app_id 也是 24 字节，带非 ASCII 字符也一样；不同应用不同，不知道是哪个应用是原来的 `wayland`。
    #[test]
    fn contexts_fit_the_server_limits() {
        let long = "org.example.".repeat(40);
        assert_eq!(context(Some(&long)).len(), 24);
        assert_eq!(context(Some("应用")).len(), 24);
        assert_ne!(
            context(Some("firefox")),
            context(Some("org.mozilla.firefox"))
        );
        assert_eq!(context(Some("foot")), context(Some("foot")));
        assert_eq!(context(None), "wayland");
    }
}
