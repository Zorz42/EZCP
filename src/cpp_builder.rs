use std::path::PathBuf;
use std::sync::LazyLock;
use crate::Error;
use crate::gcc::Gcc;
use crate::logger::Logger;
use crate::Result;

static GCC: LazyLock<Result<Gcc>> = LazyLock::new(|| {
    Gcc::new().map(|mut gcc| {
        gcc.add_flag("-O2");
        gcc.add_flag("-std=c++20");
        gcc
    })
});

/// The only job of this function is to build the solution.
/// It takes a c++ source file and produces an executable file.
/// It returns true if the executable was built and false if it was up to date.
pub fn build_solution(source_file: &PathBuf, executable_file: &PathBuf, logger: &Logger) -> Result<bool> {
    // if solution executable exists, check if it's up to date
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
            return Ok(false);
        }
    }

    let gcc = GCC.as_ref().map_err(|_err| Error::CompilerNotFound { })?;
    logger.logln(format!("Building file: {}", source_file.display()));

    let working_dir = std::env::current_dir().map_err(|err| Error::IOError { err, file: String::new() })?;
    let executable_file = working_dir.join(executable_file);
    let source_file = working_dir.join(source_file);

    gcc.compile(&source_file, &executable_file)?;

    Ok(true)
}