use std::path::PathBuf;
use std::sync::LazyLock;
use std::time::SystemTime;
use log::{debug, info};
use crate::Error;
use crate::runner::gcc::{Gcc, GccOptimization, GccStandard};
use crate::Result;

static GCC: LazyLock<Result<Gcc>> = LazyLock::new(|| {
    Gcc::new().map(|mut gcc| {
        gcc.optimization = Some(GccOptimization::Level2);
        gcc.standard = Some(GccStandard::Cpp17);
        gcc
    })
});

fn get_file_modified_time(file: &PathBuf) -> Result<SystemTime> {
    let file_str1 = file.to_str().unwrap_or("???").to_owned();
    let file_str2 = file_str1.clone();
    std::fs::metadata(file)
        .map_err(|err| Error::IOError { err, file: file_str1 })?
        .modified()
        .map_err(|err| Error::IOError { err, file: file_str2 })
}

/// The only job of this function is to build the solution.
/// It takes a c++ source file and produces an executable file.
/// It returns true if the executable was built and false if it was up to date.
pub fn build_solution(source_file: &PathBuf, executable_file: Option<&PathBuf>) -> Result<(bool, PathBuf)> {
    // if solution executable exists, check if it's up to date
    let gcc = GCC.as_ref().map_err(|_err| Error::CompilerNotFound { })?;
    if let Some(executable_file) = executable_file {
        if source_file.exists() {
            let solution_last_modified = get_file_modified_time(source_file)?;
            let solution_exe_last_modified = get_file_modified_time(executable_file)?;

            if solution_exe_last_modified > solution_last_modified {
                let timer_path = Gcc::transform_output_file(source_file, Some(executable_file));
                debug!("Solution executable is up to date: {}", executable_file.to_str().unwrap_or("???"));
                return Ok((false, timer_path));
            }
        }
    }

    info!("Building solution: {}", source_file.to_str().unwrap_or("???"));

    let working_dir = std::env::current_dir().map_err(|err| Error::IOError { err, file: String::new() })?;
    let executable_file = executable_file.map(|file| working_dir.join(file));
    let source_file = working_dir.join(source_file);

    let exec_path = gcc.compile(&source_file, executable_file.as_ref())?;

    Ok((true, exec_path))
}