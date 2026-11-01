//! Same-directory temporary files keep the commit to one atomic rename.

use std::fs::{self, File, OpenOptions};
use std::io;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_TEMPORARY: AtomicU64 = AtomicU64::new(1);

pub(super) fn replace(
    path: &Path,
    write: impl FnOnce(&mut File) -> io::Result<()>,
) -> io::Result<()> {
    let directory = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty());
    let directory = directory.unwrap_or_else(|| Path::new("."));
    fs::create_dir_all(directory)?;
    #[cfg(unix)]
    let directory_file = File::open(directory)?;
    let (temporary, mut file) = create_temporary(path)?;
    write(&mut file)?;
    file.sync_all()?;
    drop(file);
    fs::rename(&temporary.path, path)?;
    #[cfg(unix)]
    directory_file.sync_all()?;
    Ok(())
}

fn create_temporary(path: &Path) -> io::Result<(Temporary, File)> {
    let name = path
        .file_name()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "save path has no filename"))?;
    loop {
        let mut temporary_name = name.to_os_string();
        let sequence = NEXT_TEMPORARY.fetch_add(1, Ordering::Relaxed);
        temporary_name.push(format!(".{}.{sequence}.tmp", std::process::id()));
        let temporary_path = path.with_file_name(temporary_name);
        match OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary_path)
        {
            Ok(file) => {
                return Ok((
                    Temporary {
                        path: temporary_path,
                    },
                    file,
                ));
            }
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(error),
        }
    }
}

struct Temporary {
    path: PathBuf,
}

impl Drop for Temporary {
    fn drop(&mut self) {
        // Only our exclusively created staging file is ours to remove.
        let _ = fs::remove_file(&self.path);
    }
}
