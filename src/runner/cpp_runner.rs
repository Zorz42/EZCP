use crate::runner::exec_runner::{Limits, RunResult, run_solution};
use crate::runner::gcc::{Gcc, canonicalize};
use crate::{Error, Result};
use log::{trace, warn};
use std::collections::{HashMap, HashSet};
use std::hash::{DefaultHasher, Hash, Hasher};
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::mpsc;

/// Kept low so that contention between the runs does not affect verdicts.
const MAX_DEFAULT_WORKERS: usize = 4;

/// How many solutions run at once unless the task says otherwise.
pub fn default_workers() -> usize {
    std::thread::available_parallelism().map_or(1, std::num::NonZeroUsize::get).min(MAX_DEFAULT_WORKERS)
}

/// Keeps scratch names unique between threads of one process.
static SCRATCH_FILES: AtomicUsize = AtomicUsize::new(0);

fn scratch_source_name(hash: u64) -> String {
    format!("p{hash}-{}-{}.cpp", std::process::id(), SCRATCH_FILES.fetch_add(1, Ordering::Relaxed))
}

/// Renames a finished file into place, so others see either none or all of it.
///
/// On Windows the rename can fail while `destination` is open. Names are hashes
/// of the contents, so an existing `destination` is already the right file.
fn install(scratch: &Path, destination: &Path) -> Result<()> {
    std::fs::rename(scratch, destination).or_else(|err| {
        let _ = std::fs::remove_file(scratch);
        if destination.exists() { Ok(()) } else { Err(Error::io(destination)(err)) }
    })
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ProgramHandle(usize);

/// One run of a program on an input.
#[derive(Clone, Copy, Debug)]
pub struct Job<'input> {
    pub program: ProgramHandle,
    pub input: &'input str,
}

/// Stops the workers from taking more jobs if the thread collecting the results
/// unwinds, so that a panic there is not followed by every remaining run.
struct StopOnDrop<'flag>(&'flag AtomicBool);

impl Drop for StopOnDrop<'_> {
    fn drop(&mut self) {
        self.0.store(true, Ordering::Relaxed);
    }
}

/// Compiles C++ programs into a cached build folder and runs them in parallel
/// under the timer.
pub struct CppRunner {
    gcc: Gcc,
    build_folder: PathBuf,
    timer: ProgramHandle,
    /// Executables, indexed by `ProgramHandle`.
    programs: Vec<PathBuf>,
    hash_to_handle: HashMap<u64, ProgramHandle>,
    /// What `clean_build_folder` keeps.
    necessary_files: HashSet<PathBuf>,
    /// How many solutions run at once.
    workers: usize,
}

impl CppRunner {
    pub fn new(build_folder: &Path) -> Result<Self> {
        std::fs::create_dir_all(build_folder).map_err(Error::io(build_folder))?;
        let mut runner = Self {
            gcc: Gcc::new()?,
            build_folder: canonicalize(build_folder)?,
            timer: ProgramHandle(0),
            programs: Vec::new(),
            hash_to_handle: HashMap::new(),
            necessary_files: HashSet::new(),
            workers: default_workers(),
        };
        runner.timer = runner.add_program(include_str!("timer.cpp"))?;
        Ok(runner)
    }

    /// Compiles `source_code`, unless it is cached already.
    pub fn add_program(&mut self, source_code: &str) -> Result<ProgramHandle> {
        let hash = {
            let mut s = DefaultHasher::new();
            source_code.hash(&mut s);
            // The version stands in for the flags `Gcc::compile` passes.
            self.gcc.path.hash(&mut s);
            env!("CARGO_PKG_VERSION").hash(&mut s);
            s.finish()
        };

        if let Some(&handle) = self.hash_to_handle.get(&hash) {
            return Ok(handle);
        }

        let source_file = self.build_folder.join(format!("p{hash}.cpp"));
        let executable_file = Gcc::executable_path(&source_file)?;
        self.necessary_files.insert(source_file.clone());
        self.necessary_files.insert(executable_file.clone());

        // Several processes (e.g. servers started by a judge) may share the build
        // folder, so each compiles a private copy and renames the finished files
        // into place. Compiling the shared source breaks on Windows, where a rename
        // over a file that is being read succeeds and the reader loses the file.
        if !executable_file.exists() {
            trace!("Compiling: {}", executable_file.display());
            let scratch_source = self.build_folder.join(scratch_source_name(hash));
            std::fs::write(&scratch_source, source_code).map_err(Error::io(&scratch_source))?;
            let scratch_executable = Gcc::executable_path(&scratch_source)?;
            let compiled = self.gcc.compile(&scratch_source, &scratch_executable).and_then(|()| install(&scratch_executable, &executable_file));
            // Only left behind by a failed compile.
            let _ = std::fs::remove_file(&scratch_executable);
            install(&scratch_source, &source_file)?;
            compiled?;
        }

        // Only now, so a failed compile leaves no handle to a missing program.
        let handle = ProgramHandle(self.programs.len());
        self.programs.push(executable_file);
        self.hash_to_handle.insert(hash, handle);
        Ok(handle)
    }

    /// Sets how many solutions run at once; at least one does.
    pub fn set_workers(&mut self, workers: usize) {
        self.workers = workers.max(1);
    }

    pub const fn workers(&self) -> usize {
        self.workers
    }

    /// Runs every program on the same input.
    pub fn check_programs(&self, input: &str, programs: &[ProgramHandle], limits: Limits) -> Result<Vec<RunResult>> {
        let jobs: Vec<_> = programs.iter().map(|&program| Job { program, input }).collect();
        self.run_all(&jobs, limits)
    }

    /// Runs every job and returns the results in the order of the jobs.
    pub fn run_all(&self, jobs: &[Job], limits: Limits) -> Result<Vec<RunResult>> {
        let mut results = vec![None; jobs.len()];
        self.run_jobs(jobs, limits, &|_job| false, |job, result| results[job] = result)?;
        results.into_iter().collect::<Option<Vec<_>>>().ok_or_else(|| Error::TimerFailed {
            details: "a run ended without a result".to_owned(),
        })
    }

    /// Runs the jobs on the workers, which take them in order.
    ///
    /// `skip` is asked about a job right before it would start, on a worker
    /// thread. `on_result` is called on this thread as each job ends, with
    /// `None` for a skipped one, so it can change what `skip` answers.
    pub fn run_jobs<F: FnMut(usize, Option<RunResult>)>(&self, jobs: &[Job], limits: Limits, skip: &(dyn Fn(usize) -> bool + Sync), mut on_result: F) -> Result<()> {
        let timer = self.programs[self.timer.0].as_path();
        let (next_job, stopped) = (AtomicUsize::new(0), AtomicBool::new(false));
        let (sender, receiver) = mpsc::channel();
        // Returning on the first error would leave the other workers running.
        let mut first_error = None;

        std::thread::scope(|scope| {
            let _stop_on_unwind = StopOnDrop(&stopped);
            for _ in 0..self.workers.min(jobs.len()) {
                let (sender, next_job, stopped) = (sender.clone(), &next_job, &stopped);
                scope.spawn(move || {
                    // After a failure no new job is started; running ones are waited for.
                    while !stopped.load(Ordering::Relaxed) {
                        let idx = next_job.fetch_add(1, Ordering::Relaxed);
                        let Some(job) = jobs.get(idx) else { break };
                        let outcome = if skip(idx) {
                            Ok(None)
                        } else {
                            catch_unwind(AssertUnwindSafe(|| run_solution(&self.programs[job.program.0], job.input, limits, timer)))
                                .unwrap_or_else(|_panic| {
                                    Err(Error::TimerFailed {
                                        details: format!("the worker thread for job {idx} panicked"),
                                    })
                                })
                                .map(Some)
                        };
                        if outcome.is_err() {
                            stopped.store(true, Ordering::Relaxed);
                        }
                        if sender.send((idx, outcome)).is_err() {
                            break;
                        }
                    }
                });
            }
            // Only the workers hold senders now, so this ends when they are done.
            drop(sender);

            for (idx, outcome) in receiver {
                match outcome {
                    Ok(result) => {
                        trace!("Job {idx} finished with result: {result:?}");
                        on_result(idx, result);
                    }
                    Err(err) => {
                        first_error.get_or_insert(err);
                    }
                }
            }
        });

        first_error.map_or(Ok(()), Err)
    }

    /// Deletes the files in the build folder that belong to no added program.
    /// Call it only once every program has been added.
    pub fn clean_build_folder(&self) -> Result<()> {
        for entry in std::fs::read_dir(&self.build_folder).map_err(Error::io(&self.build_folder))? {
            let path = entry.map_err(Error::io(&self.build_folder))?.path();
            // A leftover only costs disk space, which is no reason to fail.
            if !self.necessary_files.contains(&path)
                && path.is_file()
                && let Err(err) = std::fs::remove_file(&path)
            {
                warn!("Could not remove {} from the build folder: {err}", path.display());
            }
        }
        Ok(())
    }
}
