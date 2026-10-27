//! 匿名内存文件：共享内存缓冲与虚拟键盘的键位表都要一个 fd 交给合成器。

use std::ffi::CStr;
use std::fs::File;
use std::io::Write;
use std::os::fd::FromRawFd;

/// 建一个 `size` 字节的匿名文件。
pub fn create(name: &CStr, size: usize) -> std::io::Result<File> {
    // SAFETY: name 是合法的 C 字符串；成功时返回的 fd 归我们
    let fd = unsafe { libc::memfd_create(name.as_ptr(), libc::MFD_CLOEXEC) };
    if fd < 0 {
        return Err(std::io::Error::last_os_error());
    }
    // SAFETY: fd 刚建好、没有别人持有
    let file = unsafe { File::from_raw_fd(fd) };
    file.set_len(size as u64)?;
    Ok(file)
}

/// 建一个装着 `bytes` 的匿名文件。
pub fn with_contents(name: &CStr, bytes: &[u8]) -> std::io::Result<File> {
    let mut file = create(name, 0)?;
    file.write_all(bytes)?;
    Ok(file)
}
