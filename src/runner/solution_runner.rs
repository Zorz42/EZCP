use std::io::Read;
use std::mem::swap;
use crate::{Error, Result};
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::thread::spawn;
use indicatif::{MultiProgress, ProgressBar};
use crate::runner::cpp_builder::build_solution;

#[derive(Clone, PartialEq, Eq)]
pub enum RunResult {
    Ok(i32), // elapsed time in milliseconds
    TimedOut,
    Crashed,
}

pub struct SolutionRunner {
    tasks: Vec<(PathBuf, PathBuf, PathBuf, f32, Option<Result<RunResult>>)>,
}

impl SolutionRunner {
    pub const fn new() -> Self {
        Self { tasks: Vec::new() }
    }
    
    pub fn add_task(&mut self, executable_file: PathBuf, input_file: PathBuf, output_file: PathBuf, time_limit: f32) -> usize {
        self.tasks.push((executable_file, input_file, output_file, time_limit, None));
        self.tasks.len() - 1
    }
    
    pub fn run_tasks(&mut self, logger: &MultiProgress, timer_path: &Path) {
        let num_threads = num_cpus::get();
        let mut threads = Vec::new();

        let mut it = 0;

        let progress_bar = logger.add(ProgressBar::new(self.tasks.len() as u64));

        loop {
            while threads.len() < num_threads && it < self.tasks.len() {
                let executable_file = self.tasks[it].0.clone();
                let input_file = self.tasks[it].1.clone();
                let output_file = self.tasks[it].2.clone();
                let time_limit = self.tasks[it].3;
                it += 1;
                progress_bar.inc(1);

                let timer_path = timer_path.to_owned();
                threads.push((spawn(move || run_solution(&executable_file, &input_file, &output_file, time_limit, &timer_path)), it - 1));
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
    
        logger.remove(&progress_bar);
    }
    
    pub fn get_result(&mut self, task_id: usize) -> Result<RunResult> {
        let mut res = None;
        swap(&mut res, &mut self.tasks.get_mut(task_id).as_mut().unwrap().4);
        res.unwrap()
    }
}

pub fn build_timer(build_dir: &Path) -> Result<PathBuf> {
    let timer_source = build_dir.join("timer.cpp");
    let timer_executable = build_dir.join("timer");
    if timer_executable.exists() {
        let timer_source_content = std::fs::read_to_string(&timer_source).unwrap();
        if timer_source_content != include_str!("timer.cpp") {
            std::fs::write(&timer_source, include_str!("timer.cpp")).unwrap();
        }
    } else {
        std::fs::write(&timer_source, include_str!("timer.cpp")).unwrap();
    }
    
    let (_, timer_path) = build_solution(&timer_source, Some(&timer_executable))?;
    
    Ok(timer_path)
}

/// This function takes an executable file and runs it with the input file.
/// It writes the output to the output file, and returns the result of the test.
/// Build dir is needed, so that timer can be built if it's not already.
pub fn run_solution(executable_file: &PathBuf, input_file: &PathBuf, output_file: &PathBuf, time_limit: f32, timer_path: &Path) -> Result<RunResult> {
    let working_dir = std::env::current_dir().map_err(|err| Error::IOError { err, file: "Error1".to_owned() })?;

    let executable_file = working_dir.join(executable_file);
    let mut solution_process = std::process::Command::new(timer_path);
    let solution_process = solution_process.arg(executable_file).arg(format!("{}", (time_limit * 1000.0) as i32));

    let input_file = working_dir.join(input_file);
    let output_file = working_dir.join(output_file);

    let input_file_str = input_file.to_str().unwrap_or("???").to_owned();
    let output_file_str = output_file.to_str().unwrap_or("???").to_owned();
    
    // spawn the solution process
    let mut solution_process = solution_process
        .stdin(std::fs::File::open(input_file).map_err(|err| Error::IOError { err, file: input_file_str })?)
        .stdout(std::fs::File::create(output_file).map_err(|err| Error::IOError { err, file: output_file_str })?)
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|err| Error::IOError { err, file: "Error2".to_owned() })?;

    let return_code = solution_process.wait().map_err(|err| Error::IOError { err, file: "Error3".to_owned() })?;
    
    if return_code.code() == Some(1) {
        return Ok(RunResult::TimedOut);
    }
    
    if !return_code.success() {
        return Ok(RunResult::Crashed);
    }

    let elapsed_time_ms = {
        // capture stderr from solution process
        let stderr = solution_process.stderr.as_mut().unwrap();
        let mut stderr_str = String::new();
        stderr.read_to_string(&mut stderr_str).map_err(|err| Error::IOError { err, file: "Error4".to_owned() })?;
        // parse output from timer command
        stderr_str.parse::<i32>().unwrap()
    };

    if elapsed_time_ms as f32 * 0.001 > time_limit {
        return Ok(RunResult::TimedOut);
    }
    
    Ok(RunResult::Ok(elapsed_time_ms))
}
