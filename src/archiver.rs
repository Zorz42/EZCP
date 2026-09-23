use crate::progress::ScopedProgressBar;
use crate::{Error, Result};
use indicatif::MultiProgress;
use std::io::Write;
use std::path::{Path, PathBuf};
use zip::write::SimpleFileOptions;

pub fn archive_files(files: &[PathBuf], archive_path: &Path, logger: &MultiProgress) -> Result<()> {
    let zip_error = |err| Error::ZipError { err };
    let mut zipper = zip::ZipWriter::new(std::fs::File::create(archive_path).map_err(Error::io(archive_path))?);
    let options = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
    let progress_bar = ScopedProgressBar::new(logger, files.len() as u64);

    for file in files {
        progress_bar.inc(1);
        zipper.start_file(file.file_name().unwrap_or_default().to_string_lossy(), options).map_err(zip_error)?;
        let contents = std::fs::read(file).map_err(Error::io(file))?;
        zipper.write_all(&contents).map_err(Error::io(file))?;
    }

    // `Drop` would finish it too, but silently ignore a failure such as a full disk.
    zipper.finish().map_err(zip_error)?;
    Ok(())
}
