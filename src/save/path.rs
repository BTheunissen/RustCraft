//! Small platform-native per-user directory selection, with no new dependency.

use std::io;
use std::path::PathBuf;

pub(super) fn local_directory() -> io::Result<PathBuf> {
    let home = std::env::var_os("HOME").map(PathBuf::from);
    #[cfg(target_os = "macos")]
    let data_home = None;
    #[cfg(target_os = "windows")]
    let data_home = std::env::var_os("LOCALAPPDATA").map(PathBuf::from);
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    let data_home = std::env::var_os("XDG_DATA_HOME").map(PathBuf::from);
    directory(home, data_home).ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::NotFound,
            "no local application-data directory",
        )
    })
}

fn directory(home: Option<PathBuf>, data_home: Option<PathBuf>) -> Option<PathBuf> {
    let home = home.filter(|path| path.is_absolute());
    let data_home = data_home.filter(|path| path.is_absolute());
    #[cfg(target_os = "macos")]
    {
        let _ = data_home;
        Some(home?.join("Library/Application Support/RustCraft"))
    }
    #[cfg(target_os = "windows")]
    {
        let _ = home;
        Some(data_home?.join("RustCraft"))
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        let root = data_home.or_else(|| Some(home?.join(".local/share")))?;
        Some(root.join("rustcraft"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_and_relative_environment_paths_never_save_in_the_launch_folder() {
        assert!(directory(None, None).is_none());
        assert!(directory(Some(PathBuf::from("relative")), Some(PathBuf::new())).is_none());
        let home = std::env::temp_dir().join("rustcraft-test-home");
        let data = std::env::temp_dir().join("rustcraft-test-data");
        let selected = directory(Some(home.clone()), Some(data.clone())).unwrap();
        #[cfg(target_os = "macos")]
        assert_eq!(selected, home.join("Library/Application Support/RustCraft"));
        #[cfg(target_os = "windows")]
        assert_eq!(selected, data.join("RustCraft"));
        #[cfg(not(any(target_os = "macos", target_os = "windows")))]
        {
            assert_eq!(selected, data.join("rustcraft"));
            assert_eq!(
                directory(Some(home.clone()), Some(PathBuf::from("relative"))).unwrap(),
                home.join(".local/share/rustcraft")
            );
        }
    }
}
