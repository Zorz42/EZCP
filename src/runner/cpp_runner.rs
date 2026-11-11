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

/// Low, so that contention between runs does not change verdicts.
const MAX_DEFAULT_WORKERS: usize = 4;

static SCRATCH_FILES: AtomicUsize = AtomicUsize::new(0);

fn scratch_source_name(hash: u64) -> String {
    format!("p{hash}-{}-{}.cpp", std::process::id(), SCRATCH_FILES.fetch_add(1, Ordering::Relaxed))
}

/// Renames a finished file into place, so others see none or all of it. On Windows this fails while
/// `destination` is open, but names are content hashes, so an existing `destination` is right.
fn install(scratch: &Path, destination: &Path) -> Result<()> {
    std::fs::rename(scratch, destination).or_else(|err| {
        let _ = std::fs::remove_file(scratch);
        if destination.exists() { Ok(()) } else { Err(Error::io(destination)(err)) }
    })
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ProgramHandle(usize);

#[derive(Clone, Copy, Debug)]
pub struct Job<'input> {
    pub program: ProgramHandle,
    pub input: &'input str,
}

/// Stops the workers if the thread collecting results unwinds, rather than running every job left.
struct StopOnDrop<'flag>(&'flag AtomicBool);

impl Drop for StopOnDrop<'_> {
    fn drop(&mut self) {
        self.0.store(true, Ordering::Relaxed);
    }
}

/// Compiles C++ programs into a cached build folder and runs them in parallel under the timer.
pub struct CppRunner {
    gcc: Gcc,
    build_folder: PathBuf,
    timer: ProgramHandle,
    /// Executables, indexed by `ProgramHandle`.
    programs: Vec<PathBuf>,
    hash_to_handle: HashMap<u64, ProgramHandle>,
    /// What `clean_build_folder` keeps.
    necessary_files: HashSet<PathBuf>,
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
            workers: std::thread::available_parallelism().map_or(1, std::num::NonZeroUsize::get).min(MAX_DEFAULT_WORKERS),
        };
        runner.timer = runner.add_program(include_str!("timer.cpp"))?;
        Ok(runner)
    }

    pub fn add_program(&mut self, source_code: &str) -> Result<ProgramHandle> {
        let hash = {
            let mut s = DefaultHasher::new();
            source_code.hash(&mut s);
            self.gcc.path.hash(&mut s);
            // Stands in for the flags `Gcc::compile` passes.
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

        // Processes (such as servers a judge starts) may share the build folder, so each compiles a
        // private copy and renames the results into place. Compiling the shared source breaks on
        // Windows, where a rename over a file being read succeeds and the reader loses it.
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

        let handle = ProgramHandle(self.programs.len());
        self.programs.push(executable_file);
        self.hash_to_handle.insert(hash, handle);
        Ok(handle)
    }

    pub fn set_workers(&mut self, workers: usize) {
        self.workers = workers.max(1);
    }

    pub const fn workers(&self) -> usize {
        self.workers
    }

    pub fn check_programs(&self, input: &str, programs: &[ProgramHandle], limits: Limits) -> Result<Vec<RunResult>> {
        let jobs: Vec<_> = programs.iter().map(|&program| Job { program, input }).collect();
        self.run_all(&jobs, limits)
    }

    /// The results are in the order of the jobs.
    pub fn run_all(&self, jobs: &[Job], limits: Limits) -> Result<Vec<RunResult>> {
        let mut results = vec![None; jobs.len()];
        self.run_jobs(jobs, limits, &|_job| false, |job, result| results[job] = result)?;
        results.into_iter().collect::<Option<Vec<_>>>().ok_or_else(|| Error::TimerFailed {
            details: "a run ended without a result".to_owned(),
        })
    }

    /// The workers take the jobs in order. `skip` is asked on a worker right before a job would start;
    /// `on_result` is called on this thread as each job ends (`None` if skipped), so it can change
    /// what `skip` answers.
    pub fn run_jobs<F: FnMut(usize, Option<RunResult>)>(&self, jobs: &[Job], limits: Limits, skip: &(dyn Fn(usize) -> bool + Sync), mut on_result: F) -> Result<()> {
        let timer = self.programs[self.timer.0].as_path();
        let (next_job, stopped) = (AtomicUsize::new(0), AtomicBool::new(false));
        let (sender, receiver) = mpsc::channel();
        let mut first_error = None;

        std::thread::scope(|scope| {
            let _stop_on_unwind = StopOnDrop(&stopped);
            for _ in 0..self.workers.min(jobs.len()) {
                let (sender, next_job, stopped) = (sender.clone(), &next_job, &stopped);
                scope.spawn(move || {
                    // After a failure no job starts; running ones are waited for.
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
            // Only the workers hold senders now, so this ends with them.
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

    /// Deletes what belongs to no added program, so call it once all are added.
    pub fn clean_build_folder(&self) -> Result<()> {
        for entry in std::fs::read_dir(&self.build_folder).map_err(Error::io(&self.build_folder))? {
            let path = entry.map_err(Error::io(&self.build_folder))?.path();
            // A leftover only costs disk space.
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
