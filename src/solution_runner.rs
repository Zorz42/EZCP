use std::io::{Read, Write};
use std::mem::swap;
use crate::{Error, Result};
use std::path::PathBuf;
use std::process::Stdio;
use std::thread::spawn;
use libc::wait;
use crate::logger::Logger;
use crate::progress_bar::{clear_progress_bar, print_progress_bar};

#[cfg(windows)]
enum WindowsCompiler {
    FullPath(PathBuf),
    Command(PathBuf),
}

#[cfg(windows)]
impl WindowsCompiler {
    pub fn get_path(&self) -> PathBuf {
        match self {
            Self::FullPath(path) => path.clone(),
            Self::Command(command) => command.clone(),
        }
    }
}

#[cfg(windows)]
fn get_gcc_path() -> Result<WindowsCompiler> {
    if let Ok(gcc_path) = std::env::var("GCC_PATH") {
        return Ok(WindowsCompiler::FullPath(PathBuf::from(gcc_path)));
    }
    let possible_commands = ["g++", "c++"];
    for command in possible_commands {
        if let Ok(gcc_path) = std::process::Command::new(command).arg("--version").output() {
            if gcc_path.status.success() {
                return Ok(WindowsCompiler::Command(PathBuf::from(command)));
            }
        }
    }

    let possible_paths = ["C:\\MinGW\\bin\\c++.exe"];
    for path in possible_paths {
        if PathBuf::from(path).exists() {
            return Ok(WindowsCompiler::FullPath(PathBuf::from(path)));
        }
    }

    Err(Error::CompilerNotFoundWindows)
}

/// The only job of this function is to build the solution.
/// It takes a c++ source file and produces an executable file.
/// It returns true if the executable was built and false if it was up to date.
pub fn build_solution(source_file: &PathBuf, executable_file: &PathBuf) -> Result<bool> {
    // if solution executable exists, check if it's up to date
    if executable_file.exists() {
        let solution_last_modified = std::fs::metadata(source_file).map_err(|err| Error::IOError { err })?.modified().map_err(|err| Error::IOError { err })?;
        let solution_exe_last_modified = std::fs::metadata(executable_file)
            .map_err(|err| Error::IOError { err })?
            .modified()
            .map_err(|err| Error::IOError { err })?;

        if solution_exe_last_modified > solution_last_modified {
            return Ok(false);
        }
    }

    #[cfg(windows)]
    {
        let gcc_path = get_gcc_path()?;
        let prev_working_dir = std::env::current_dir().map_err(|err| Error::IOError { err })?;

        let mut process = std::process::Command::new(gcc_path.get_path());

        if let WindowsCompiler::FullPath(gcc_path) = &gcc_path {
            let working_dir = std::path::Path::new(gcc_path).parent().unwrap_or_else(|| std::path::Path::new("/")).to_path_buf();
            process.current_dir(working_dir);
        }

        // check if g++ is installed
        if std::process::Command::new(gcc_path.get_path()).arg("--version").output().is_err() {
            return Err(Error::CompilerNotFoundWindows);
        }

        let executable_file = prev_working_dir.join(executable_file);
        let source_file = prev_working_dir.join(source_file);

        // invoke g++ to build solution
        let process = process
            .arg("-std=c++17")
            .arg("-O2")
            .arg("-o")
            .arg(executable_file)
            .arg(source_file)
            .output()
            .map_err(|err| Error::IOError { err })?;

        if !process.status.success() {
            return Err(Error::CompilerError {
                stderr: String::from_utf8_lossy(&process.stderr).to_string(),
                stdout: String::from_utf8_lossy(&process.stdout).to_string(),
            });
        }
    }

    #[cfg(unix)]
    {
        // check if g++ is installed
        if std::process::Command::new("g++").arg("--version").output().is_err() {
            return Err(Error::CompilerNotFoundUnix);
        }

        // invoke g++ to build solution
        let process = std::process::Command::new("g++")
            .arg("-std=c++20")
            .arg("-O2")
            .arg("-o")
            .arg(executable_file)
            .arg(source_file)
            .output()
            .map_err(|err| Error::IOError { err })?;

        if !process.status.success() {
            return Err(Error::CompilerError {
                stderr: String::from_utf8_lossy(&process.stderr).to_string(),
                stdout: String::from_utf8_lossy(&process.stdout).to_string(),
            });
        }
    }

    Ok(true)
}

#[derive(Clone)]
pub enum TestResult {
    Ok(i32), // elapsed time in milliseconds
    TimedOut,
    Crashed,
}

pub struct SolutionRunner {
    tasks: Vec<(PathBuf, PathBuf, PathBuf, f32, Option<Result<TestResult>>)>,
}

impl SolutionRunner {
    pub const fn new() -> Self {
        Self { tasks: Vec::new() }
    }
    
    pub fn add_task(&mut self, executable_file: PathBuf, input_file: PathBuf, output_file: PathBuf, time_limit: f32) -> usize {
        self.tasks.push((executable_file, input_file, output_file, time_limit, None));
        self.tasks.len() - 1
    }
    
    pub fn run_tasks(&mut self, logger: &Logger) {
        let loading_progress_max = self.tasks.len() as i32;
        let mut loading_progress = 0;

        let num_threads = num_cpus::get();
        let mut threads = Vec::new();

        let mut it = 0;

        loop {
            while threads.len() < num_threads && it < self.tasks.len() {
                let executable_file = self.tasks[it].0.clone();
                let input_file = self.tasks[it].1.clone();
                let output_file = self.tasks[it].2.clone();
                let time_limit = self.tasks[it].3;
                it += 1;
                loading_progress += 1;
                print_progress_bar((loading_progress as f32) / (loading_progress_max as f32), logger);

                threads.push((std::thread::spawn(move || run_solution(&executable_file, &input_file, &output_file, time_limit)), it - 1));
            }

            let mut new_threads = Vec::new();
            for (thread, idx) in threads {
                if thread.is_finished() {
                    let result = thread.join().unwrap();
                    self.tasks[idx].4 = Some(result);
                } else {
                    new_threads.push((thread, idx));
                }
            }

            threads = new_threads;

            std::thread::sleep(std::time::Duration::from_millis(1));

            if it == self.tasks.len() && threads.is_empty() {
                break;
            }
        }

        assert_eq!(loading_progress, loading_progress_max);
        clear_progress_bar(logger);
    }
    
    pub fn get_result(&mut self, task_id: usize) -> Result<TestResult> {
        let mut res = None;
        swap(&mut res, &mut self.tasks.get_mut(task_id).as_mut().unwrap().4);
        res.unwrap()
    }
}

fn parse_time_output(output: &str) -> (f32, f32) {
    let mut tokens = output.split_whitespace().collect::<Vec<_>>();
    let mut floats = Vec::new();
    for token in tokens {
        if let Ok(float) = token.parse::<f32>() {
            floats.push(float);
        }
    }
    assert_eq!(floats.len(), 3);
    (floats[1], floats[2])
}

/// This function takes an executable file and runs it with the input file.
/// It writes the output to the output file, and returns the result of the test.
pub fn run_solution(executable_file: &PathBuf, input_file: &PathBuf, output_file: &PathBuf, time_limit: f32) -> Result<TestResult> {
    let start_time = std::time::Instant::now();

    let working_dir = std::env::current_dir().map_err(|err| Error::IOError { err })?;

    let executable_file = working_dir.join(executable_file);
    #[cfg(unix)]
    let mut solution_process = std::process::Command::new("time");
    #[cfg(unix)]
    let solution_process = solution_process.arg(executable_file);
    #[cfg(windows)]
    let mut solution_process = std::process::Command::new(executable_file);

    #[cfg(windows)]
    {
        let gcc_path = get_gcc_path()?;
        if let WindowsCompiler::FullPath(gcc_path) = &gcc_path {
            let working_dir = std::path::Path::new(gcc_path).parent().unwrap_or_else(|| std::path::Path::new("/")).to_path_buf();
            solution_process.current_dir(working_dir);
        }
    }

    let input_file = working_dir.join(input_file);
    let output_file = working_dir.join(output_file);

    // spawn the solution process
    let mut solution_process = solution_process
        .stdin(std::fs::File::open(input_file).map_err(|err| Error::IOError { err })?)
        .stdout(std::fs::File::create(output_file).map_err(|err| Error::IOError { err })?)
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|err| Error::IOError { err })?;

    while solution_process.try_wait().map_err(|err| Error::IOError { err })?.is_none() {
        std::thread::sleep(std::time::Duration::from_millis(1));
        let end_time = std::time::Instant::now();
        let elapsed = end_time.duration_since(start_time);
        let elapsed_time = elapsed.as_secs_f32();
        if elapsed_time > 5.0 * time_limit {
            solution_process.kill().map_err(|err| Error::IOError { err })?;
            return Ok(TestResult::TimedOut);
        }
    }

    #[cfg(unix)]
    let elapsed_time = {
        // capture stderr from solution process
        let stderr = solution_process.stderr.as_mut().unwrap();
        let mut stderr_str = String::new();
        stderr.read_to_string(&mut stderr_str).map_err(|err| Error::IOError { err })?;
        // parse output from time command
        let (user_time, sys_time) = parse_time_output(&stderr_str);
        user_time + sys_time
    };

    #[cfg(windows)]
    let elapsed_time = {
        let end_time = std::time::Instant::now();
        end_time.duration_since(start_time);
    };

    if elapsed_time > time_limit {
        println!("{elapsed_time} > {time_limit}");
        return Ok(TestResult::TimedOut);
    }
    
    let solution_status = solution_process.wait().map_err(|err| Error::IOError { err })?;

    if !solution_status.success() {
        return Ok(TestResult::Crashed);
    }

    let elapsed_time_ms = (elapsed_time * 1000.0).ceil() as i32;
    Ok(TestResult::Ok(elapsed_time_ms))
}

/// Compares if two file have equal contents.
/// It ignores whitespace.
pub fn are_files_equal(file1: &PathBuf, file2: &PathBuf) -> Result<bool> {
    let file1 = std::fs::read_to_string(file1).map_err(|err| Error::IOError { err })?;
    let file2 = std::fs::read_to_string(file2).map_err(|err| Error::IOError { err })?;

    let file1_it = file1.split_whitespace();
    let file2_it = file2.split_whitespace();

    let mut file1 = Vec::new();
    let mut file2 = Vec::new();

    for i in file1_it {
        file1.push(i.to_owned());
    }

    for i in file2_it {
        file2.push(i.to_owned());
    }
    
    Ok(file1 == file2)
}
