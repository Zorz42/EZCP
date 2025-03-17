use std::io::Write;
use std::path::{Path, PathBuf};
use zip::write::SimpleFileOptions;
use crate::Error;
use crate::logger::Logger;
use crate::progress_bar::{clear_progress_bar, print_progress_bar};
use crate::Result;

pub fn archive_files(files: &[PathBuf], archive_path: &Path, logger: &Logger) -> Result<()> {
    let mut zipper = zip::ZipWriter::new(std::fs::File::create(archive_path).map_err(|err| Error::IOError { err, file: String::new() })?);
    let options = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);

    let loading_progress_max = files.len() as i32;

    for (loading_progress, file) in files.iter().enumerate() {
        print_progress_bar((loading_progress as f32) / (loading_progress_max as f32), logger);

        zipper
            .start_file(file.file_name().map_or("", |a| a.to_str().unwrap_or("")), options)
            .map_err(|err| Error::ZipError { err })?;
        let input_file = std::fs::read(file).map_err(|err| Error::IOError { err, file: String::new() })?;
        zipper.write_all(&input_file).map_err(|err| Error::IOError { err, file: String::new() })?;
    }

    clear_progress_bar(logger);

    Ok(())
}