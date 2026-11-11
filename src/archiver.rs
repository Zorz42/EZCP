use crate::parallel::parallel_map;
use crate::progress::ScopedProgressBar;
use crate::{Error, Result};
use indicatif::MultiProgress;
use std::io::{Cursor, Write};
use std::panic::resume_unwind;
use std::path::Path;
use zip::write::SimpleFileOptions;
use zip::{ZipArchive, ZipWriter};

const fn zip_error(err: zip::result::ZipError) -> Error {
    Error::ZipError { err }
}

fn compress<N: AsRef<str>, C: AsRef<str>>(files: &[(N, C)], progress_bar: &ScopedProgressBar) -> Result<Vec<u8>> {
    let options = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
    let mut zipper = ZipWriter::new(Cursor::new(Vec::new()));
    for (name, contents) in files {
        zipper.start_file(name.as_ref(), options).map_err(zip_error)?;
        zipper.write_all(contents.as_ref().as_bytes()).map_err(Error::io(Path::new(name.as_ref())))?;
        progress_bar.inc(1);
    }
    Ok(zipper.finish().map_err(zip_error)?.into_inner())
}

/// Compresses pieces on `workers` threads, then joins them in order, which copies the compressed data.
pub fn archive_files<N: AsRef<str> + Sync, C: AsRef<str> + Sync>(files: &[(N, C)], archive_path: &Path, workers: usize, logger: &MultiProgress) -> Result<()> {
    let progress_bar = ScopedProgressBar::new(logger, files.len() as u64);
    // Far more pieces than workers, so large files do not leave the others idle.
    let pieces: Vec<_> = files.chunks(files.len().div_ceil(8 * workers.max(1)).max(1)).collect();
    let pieces = parallel_map(&pieces, workers, |piece| compress(piece, &progress_bar));

    let mut zipper = ZipWriter::new(std::fs::File::create(archive_path).map_err(Error::io(archive_path))?);
    for piece in pieces {
        let piece = piece.unwrap_or_else(|panic| resume_unwind(panic))?;
        zipper.merge_archive(ZipArchive::new(Cursor::new(piece)).map_err(zip_error)?).map_err(zip_error)?;
    }
    // `Drop` would ignore a failure such as a full disk.
    zipper.finish().map_err(zip_error)?;
    Ok(())
}
