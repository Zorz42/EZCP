use std::io::Write;
use std::path::{Path, PathBuf};
use indicatif::{MultiProgress, ProgressBar};
use zip::write::SimpleFileOptions;
use crate::Error;
use crate::Result;

pub fn archive_files(files: &[PathBuf], archive_path: &Path, logger: &MultiProgress) -> Result<()> {
    let mut zipper = zip::ZipWriter::new(std::fs::File::create(archive_path).map_err(|err| Error::IOError { err, file: String::new() })?);
    let options = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);

    let progress_bar = logger.add(ProgressBar::new(files.len() as u64));

    for file in files {
        progress_bar.inc(1);

        zipper
            .start_file(file.file_name().map_or("", |a| a.to_str().unwrap_or("")), options)
            .map_err(|err| Error::ZipError { err })?;
        let input_file = std::fs::read(file).map_err(|err| Error::IOError { err, file: String::new() })?;
        zipper.write_all(&input_file).map_err(|err| Error::IOError { err, file: String::new() })?;
    }

    logger.remove(&progress_bar);
    
    Ok(())
}