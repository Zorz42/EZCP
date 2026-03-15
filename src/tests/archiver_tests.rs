#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod archiver_tests {
    use crate::archiver::archive_files;
    use crate::Error;
    use indicatif::MultiProgress;
    use std::io::Read;
    use std::path::PathBuf;
    use tempfile::TempDir;

    fn make_progress() -> MultiProgress {
        MultiProgress::new()
    }

    #[test]
    fn test_archive_empty_file_list() {
        let dir = TempDir::new().unwrap();
        let archive_path = dir.path().join("empty.zip");
        let mp = make_progress();

        archive_files(&[], &archive_path, &mp).unwrap();

        assert!(archive_path.exists());
        // Opening the zip should succeed and contain 0 files
        let archive_file = std::fs::File::open(&archive_path).unwrap();
        let zip = zip::ZipArchive::new(archive_file).unwrap();
        assert_eq!(zip.len(), 0);
    }

    #[test]
    fn test_archive_single_file_content() {
        let dir = TempDir::new().unwrap();
        let file_path = dir.path().join("hello.txt");
        let content = b"Hello, archive!";
        std::fs::write(&file_path, content).unwrap();

        let archive_path = dir.path().join("out.zip");
        let mp = make_progress();

        archive_files(&[file_path], &archive_path, &mp).unwrap();

        // Re-open and check content
        let archive_file = std::fs::File::open(&archive_path).unwrap();
        let mut zip = zip::ZipArchive::new(archive_file).unwrap();
        assert_eq!(zip.len(), 1);

        let mut file = zip.by_index(0).unwrap();
        assert_eq!(file.name(), "hello.txt");
        let mut buf = Vec::new();
        file.read_to_end(&mut buf).unwrap();
        assert_eq!(buf, content);
    }

    #[test]
    fn test_archive_multiple_files() {
        let dir = TempDir::new().unwrap();
        let mut paths: Vec<PathBuf> = Vec::new();
        for i in 0..5 {
            let p = dir.path().join(format!("file{i}.txt"));
            std::fs::write(&p, format!("content {i}")).unwrap();
            paths.push(p);
        }

        let archive_path = dir.path().join("multi.zip");
        let mp = make_progress();
        archive_files(&paths, &archive_path, &mp).unwrap();

        let archive_file = std::fs::File::open(&archive_path).unwrap();
        let zip = zip::ZipArchive::new(archive_file).unwrap();
        assert_eq!(zip.len(), 5);
    }

    #[test]
    fn test_archive_nonexistent_file_returns_error() {
        let dir = TempDir::new().unwrap();
        let missing = PathBuf::from("/nonexistent/path/to/nowhere.txt");
        let archive_path = dir.path().join("err.zip");
        let mp = make_progress();

        // archive_files will try to read the file, which should fail
        let result = archive_files(&[missing], &archive_path, &mp);
        assert!(matches!(result, Err(Error::IOError { .. })));
    }

    #[test]
    fn test_archive_preserves_file_names() {
        let dir = TempDir::new().unwrap();
        let names = ["alpha.in", "beta.out", "gamma.txt"];
        let mut paths = Vec::new();
        for name in &names {
            let p = dir.path().join(name);
            std::fs::write(&p, "data").unwrap();
            paths.push(p);
        }

        let archive_path = dir.path().join("named.zip");
        let mp = make_progress();
        archive_files(&paths, &archive_path, &mp).unwrap();

        let archive_file = std::fs::File::open(&archive_path).unwrap();
        let mut zip = zip::ZipArchive::new(archive_file).unwrap();
        let mut zip_names: Vec<String> = (0..zip.len())
            .map(|i| zip.by_index(i).unwrap().name().to_owned())
            .collect();
        zip_names.sort_unstable();
        let mut expected: Vec<String> = names.iter().map(|s| (*s).to_owned()).collect();
        expected.sort_unstable();
        assert_eq!(zip_names, expected);
    }
}
