#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod archiver_tests {
    use crate::Error;
    use crate::archiver::archive_files;
    use indicatif::MultiProgress;
    use std::io::Read;
    use std::path::{Path, PathBuf};
    use tempfile::TempDir;

    /// `(name, contents)` of every file in the archive, sorted by name.
    fn archived(archive: &Path) -> Vec<(String, String)> {
        let mut zip = zip::ZipArchive::new(std::fs::File::open(archive).unwrap()).unwrap();
        let mut files: Vec<_> = (0..zip.len())
            .map(|i| {
                let mut file = zip.by_index(i).unwrap();
                let mut contents = String::new();
                file.read_to_string(&mut contents).unwrap();
                (file.name().to_owned(), contents)
            })
            .collect();
        files.sort();
        files
    }

    #[test]
    fn test_archive_files() {
        let dir = TempDir::new().unwrap();
        let names = ["alpha.in", "beta.out", "gamma.txt"];
        let paths: Vec<PathBuf> = names.iter().map(|name| dir.path().join(name)).collect();
        for (path, name) in paths.iter().zip(names) {
            std::fs::write(path, format!("content of {name}")).unwrap();
        }

        let archive = dir.path().join("tests.zip");
        archive_files(&paths, &archive, &MultiProgress::new()).unwrap();

        assert_eq!(archived(&archive), names.map(|name| (name.to_owned(), format!("content of {name}"))));
    }

    #[test]
    fn test_archive_empty_file_list() {
        let dir = TempDir::new().unwrap();
        let archive = dir.path().join("empty.zip");
        archive_files(&[], &archive, &MultiProgress::new()).unwrap();
        assert!(archived(&archive).is_empty());
    }

    #[test]
    fn test_archive_nonexistent_file_returns_error() {
        let dir = TempDir::new().unwrap();
        let result = archive_files(&[PathBuf::from("/nonexistent/path/to/nowhere.txt")], &dir.path().join("err.zip"), &MultiProgress::new());
        assert!(matches!(result, Err(Error::IOError { .. })));
    }
}
