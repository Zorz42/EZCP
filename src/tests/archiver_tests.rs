use crate::Error;
use crate::archiver::archive_files;
use indicatif::MultiProgress;
use std::io::Read;
use std::path::Path;
use tempfile::TempDir;

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
    let files = ["alpha.in", "beta.out", "gamma.txt"].map(|name| (name.to_owned(), format!("content of {name}")));

    let archive = dir.path().join("tests.zip");
    archive_files(&files, &archive, 1, &MultiProgress::new()).unwrap();

    assert_eq!(archived(&archive), files);
}

#[test]
fn test_archive_empty_file_list() {
    let dir = TempDir::new().unwrap();
    let archive = dir.path().join("empty.zip");
    archive_files::<&str, &str>(&[], &archive, 4, &MultiProgress::new()).unwrap();
    assert_eq!(archived(&archive), []);
}

#[test]
fn test_archive_in_a_missing_directory_returns_error() {
    let dir = TempDir::new().unwrap();
    let result = archive_files(&[("a.in", "1\n")], &dir.path().join("missing").join("err.zip"), 1, &MultiProgress::new());
    assert!(matches!(result, Err(Error::IOError { .. })));
}

/// Files of very different sizes, including empty ones, so the pieces the
/// workers compress are uneven.
#[test]
fn test_the_archive_does_not_depend_on_the_number_of_workers() {
    let dir = TempDir::new().unwrap();
    let files: Vec<_> = (0..23_usize).map(|i| (format!("test.{i:02}.in"), format!("{i} ").repeat((i * 7919) % 5000))).collect();

    let archive_with = |workers: usize| {
        let archive = dir.path().join(format!("tests_{workers}.zip"));
        archive_files(&files, &archive, workers, &MultiProgress::new()).unwrap();
        // In the order they were given, not sorted as by `archived`.
        let mut zip = zip::ZipArchive::new(std::fs::File::open(&archive).unwrap()).unwrap();
        let names: Vec<_> = (0..zip.len()).map(|i| zip.by_index(i).unwrap().name().to_owned()).collect();
        assert_eq!(names, files.iter().map(|(name, _)| name.clone()).collect::<Vec<_>>(), "with {workers} workers");
        assert_eq!(archived(&archive), files, "with {workers} workers");
        std::fs::read(&archive).unwrap()
    };

    let single = archive_with(1);
    for workers in [2, 4, 7, 100] {
        assert_eq!(archive_with(workers), single, "the archive made by {workers} workers differs");
    }
}
