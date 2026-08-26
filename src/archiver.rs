use crate::Error;
use crate::Result;
use crate::progress::ScopedProgressBar;
use indicatif::MultiProgress;
use std::io::Write;
use std::path::{Path, PathBuf};
use zip::write::SimpleFileOptions;

pub fn archive_files(files: &[PathBuf], archive_path: &Path, logger: &MultiProgress) -> Result<()> {
    // Paths are only ever guaranteed to be UTF-8 on some platforms, so never
    // unwrap on the conversion: a lossy label is enough for error messages.
    let mut zipper = zip::ZipWriter::new(std::fs::File::create(archive_path).map_err(|err| Error::IOError {
        err,
        file: archive_path.to_string_lossy().into_owned(),
    })?);
    let options = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);

    let progress_bar = ScopedProgressBar::new(logger, files.len() as u64);

    for file in files {
        progress_bar.inc(1);
        let file_name = file.to_string_lossy().into_owned();

        zipper
            .start_file(file.file_name().unwrap_or_default().to_string_lossy(), options)
            .map_err(|err| Error::ZipError { err })?;
        let input_file = std::fs::read(file).map_err(|err| Error::IOError { err, file: file_name.clone() })?;
        zipper.write_all(&input_file).map_err(|err| Error::IOError { err, file: file_name.clone() })?;
    }

    // Write the central directory by hand instead of leaving it to `Drop`, which
    // has nowhere to report a failure to. Without this a full disk would leave a
    // truncated archive behind and the run would still be reported as a success.
    zipper.finish().map_err(|err| Error::ZipError { err })?;

    Ok(())
}
