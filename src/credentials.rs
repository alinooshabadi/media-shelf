//! User-local token persistence. Windows protects the bytes with user-scoped DPAPI.
use std::{
    io::{self, Write},
    path::{Path, PathBuf},
};

pub fn token_path() -> PathBuf {
    crate::storage::database_path().with_file_name("tmdb-token.dat")
}

pub fn load(path: &Path) -> io::Result<String> {
    let bytes = match std::fs::read(path) {
        Ok(bytes) => bytes,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(String::new()),
        Err(e) => return Err(e),
    };
    String::from_utf8(protect(&bytes, false)?)
        .map_err(|_| io::Error::other("Stored token is invalid"))
}

pub fn save(path: &Path, token: &str) -> io::Result<()> {
    let token = token.trim();
    if token.is_empty() {
        return match std::fs::remove_file(path) {
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(()),
            result => result,
        };
    }
    if token.len() > 16384 {
        return Err(io::Error::other("Token is too long"));
    }
    let bytes = protect(token.as_bytes(), true)?;
    let parent = path
        .parent()
        .ok_or_else(|| io::Error::other("Invalid token location"))?;
    std::fs::create_dir_all(parent)?;
    let mut temp = tempfile::NamedTempFile::new_in(parent)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        temp.as_file()
            .set_permissions(std::fs::Permissions::from_mode(0o600))?;
    }
    temp.write_all(&bytes)?;
    temp.as_file().sync_all()?;
    temp.persist(path).map_err(|e| e.error)?;
    Ok(())
}

#[cfg(windows)]
fn protect(bytes: &[u8], encrypt: bool) -> io::Result<Vec<u8>> {
    use windows_sys::Win32::{
        Foundation::LocalFree,
        Security::Cryptography::{
            CRYPT_INTEGER_BLOB, CRYPTPROTECT_UI_FORBIDDEN, CryptProtectData, CryptUnprotectData,
        },
    };
    let input = CRYPT_INTEGER_BLOB {
        cbData: u32::try_from(bytes.len()).map_err(|_| io::Error::other("Token is too large"))?,
        pbData: bytes.as_ptr().cast_mut(),
    };
    let mut output = CRYPT_INTEGER_BLOB {
        cbData: 0,
        pbData: std::ptr::null_mut(),
    };
    // DPAPI borrows the input and allocates output, which must be freed with LocalFree.
    unsafe {
        let ok = if encrypt {
            CryptProtectData(
                &input,
                std::ptr::null(),
                std::ptr::null(),
                std::ptr::null(),
                std::ptr::null(),
                CRYPTPROTECT_UI_FORBIDDEN,
                &mut output,
            )
        } else {
            CryptUnprotectData(
                &input,
                std::ptr::null_mut(),
                std::ptr::null(),
                std::ptr::null(),
                std::ptr::null(),
                CRYPTPROTECT_UI_FORBIDDEN,
                &mut output,
            )
        };
        if ok == 0 {
            return Err(io::Error::last_os_error());
        }
        let result = std::slice::from_raw_parts(output.pbData, output.cbData as usize).to_vec();
        LocalFree(output.pbData.cast());
        Ok(result)
    }
}

#[cfg(not(windows))]
fn protect(bytes: &[u8], _: bool) -> io::Result<Vec<u8>> {
    Ok(bytes.to_vec())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn saved_token_survives_reload_replacement_and_removal() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings/token.dat");
        assert_eq!(load(&path).unwrap(), "");
        save(&path, " sample-test-token ").unwrap();
        assert_eq!(load(&path).unwrap(), "sample-test-token");
        #[cfg(windows)]
        assert!(
            !std::fs::read(&path)
                .unwrap()
                .windows(17)
                .any(|v| v == b"sample-test-token")
        );
        save(&path, "replacement-test-token").unwrap();
        assert_eq!(load(&path).unwrap(), "replacement-test-token");
        save(&path, "").unwrap();
        assert_eq!(load(&path).unwrap(), "");
    }
    #[test]
    #[cfg(windows)]
    fn corrupt_saved_token_reports_failure() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("token.dat");
        std::fs::write(&path, b"invalid fixture").unwrap();
        assert!(load(&path).is_err());
    }
}
