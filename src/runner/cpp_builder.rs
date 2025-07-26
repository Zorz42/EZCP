use std::path::PathBuf;
use std::sync::LazyLock;
use log::info;
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

/// The only job of this function is to build the solution.
/// It takes a c++ source file and produces an executable file.
/// It returns true if the executable was built and false if it was up to date.
pub fn build_solution(source_file: &PathBuf, executable_file: &PathBuf) -> Result<(bool, PathBuf)> {
    // if solution executable exists, check if it's up to date
    let gcc = GCC.as_ref().map_err(|_err| Error::CompilerNotFound { })?;
    if executable_file.exists() {
        let executable_file_str1 = executable_file.to_str().unwrap_or("???").to_owned();
        let executable_file_str2 = executable_file_str1.clone();
        let executable_file_str3 = executable_file_str1.clone();
        let executable_file_str4 = executable_file_str1.clone();
        let solution_last_modified = std::fs::metadata(source_file)
            .map_err(|err| Error::IOError { err, file: executable_file_str1 })?
            .modified().map_err(|err| Error::IOError { err, file: executable_file_str2 })?;
        let solution_exe_last_modified = std::fs::metadata(executable_file)
            .map_err(|err| Error::IOError { err, file: executable_file_str3 })?
            .modified()
            .map_err(|err| Error::IOError { err, file: executable_file_str4 })?;

        if solution_exe_last_modified > solution_last_modified {
            let timer_path = Gcc::transform_output_file(source_file, Some(executable_file));
            return Ok((false, timer_path));
        }
    }

    info!("Building solution: {}", executable_file.to_str().unwrap_or("???"));

    let working_dir = std::env::current_dir().map_err(|err| Error::IOError { err, file: String::new() })?;
    let executable_file = working_dir.join(executable_file);
    let source_file = working_dir.join(source_file);

    let timer_path = gcc.compile(&source_file, Some(&executable_file))?;

    Ok((true, timer_path))
}