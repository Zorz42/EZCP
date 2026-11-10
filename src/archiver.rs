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

/// Splits `files` into at most `pieces` runs of about the same number of bytes.
fn split_evenly<N, C: AsRef<str>>(files: &[(N, C)], pieces: usize) -> Vec<&[(N, C)]> {
    let size = |files: &[(N, C)]| files.iter().map(|(_, contents)| contents.as_ref().len()).sum::<usize>();
    let per_piece = size(files).div_ceil(pieces.max(1)).max(1);

    let mut result = Vec::new();
    let mut rest = files;
    while !rest.is_empty() {
        // At least one file, then as many as stay within the share of a piece.
        let mut taken = 1;
        while taken < rest.len() && size(&rest[..=taken]) <= per_piece {
            taken += 1;
        }
        let (piece, remaining) = rest.split_at(taken);
        result.push(piece);
        rest = remaining;
    }
    result
}

/// An archive of `files`, in memory.
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

/// Writes an archive of `files`, given as names and contents, to `archive_path`.
///
/// Compressing is most of the work, so the files are compressed in pieces on
/// `workers` threads. The pieces are then joined in order, which copies the
/// compressed data as it is.
pub fn archive_files<N: AsRef<str> + Sync, C: AsRef<str> + Sync>(files: &[(N, C)], archive_path: &Path, workers: usize, logger: &MultiProgress) -> Result<()> {
    let progress_bar = ScopedProgressBar::new(logger, files.len() as u64);
    let pieces = parallel_map(&split_evenly(files, workers), workers, |piece| compress(piece, &progress_bar));

    let mut zipper = ZipWriter::new(std::fs::File::create(archive_path).map_err(Error::io(archive_path))?);
    for piece in pieces {
        let piece = piece.unwrap_or_else(|panic| resume_unwind(panic))?;
        zipper.merge_archive(ZipArchive::new(Cursor::new(piece)).map_err(zip_error)?).map_err(zip_error)?;
    }
    // `Drop` would finish it too, but silently ignore a failure such as a full disk.
    zipper.finish().map_err(zip_error)?;
    Ok(())
}
