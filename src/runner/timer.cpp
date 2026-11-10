// Usage: timer <executable> <time_limit_ms> [memory_limit_kb]
//
// Runs the solution on the inherited stdin/stdout/stderr, then appends
//
//     \n__EZCP_RESULT__ <OK|TLE|RTE|MLE|ERR> <cpu_time_ms> <peak_memory_kb>\n
//
// to stderr. EZCP reads the last such line, so the solution's own stderr
// cannot spoof it; exit codes are not used because they are not portable.
// ERR means the solution could not be started.
//
// Limits and times are CPU time, so that load from the solutions EZCP runs in
// parallel does not change verdicts. A wall-clock deadline catches solutions
// that block without using CPU.
//
// EZCP compares the reported peak with the memory limit itself. Here the limit
// only sets a far higher cap on what the solution may allocate, so that a
// runaway one fails instead of exhausting the machine. MLE is reported only for
// a solution too large to even start under that cap.

#include <stdio.h>

static void report(const char *verdict, long long cpu_time_ms, long long peak_memory_kb) {
  fprintf(stderr, "\n__EZCP_RESULT__ %s %lld %lld\n", verdict, cpu_time_ms, peak_memory_kb);
  fflush(stderr);
}

// Twice the limit plus 1 GiB: nothing that stays within the limit comes close.
static unsigned long long memory_cap_bytes(long long memory_limit_kb) {
  return (unsigned long long)memory_limit_kb * 1024ULL * 2ULL + (1ULL << 30);
}

#ifdef _WIN32

#define WIN32_LEAN_AND_MEAN
// Must precede shellapi.h, which uses its types.
#include <windows.h>

#include <io.h>
#include <shellapi.h>
#include <stdlib.h>
#include <string>
#include <vector>

static long long filetime_to_ms(const FILETIME &ft) {
  ULARGE_INTEGER value;
  value.LowPart = ft.dwLowDateTime;
  value.HighPart = ft.dwHighDateTime;
  return (long long)(value.QuadPart / 10000ULL); // 100 ns ticks
}

static long long get_cpu_time_ms(HANDLE process) {
  FILETIME creation_time, exit_time, kernel_time, user_time;
  if (!GetProcessTimes(process, &creation_time, &exit_time, &kernel_time, &user_time)) {
    return 0;
  }
  return filetime_to_ms(kernel_time) + filetime_to_ms(user_time);
}

// The peak committed memory of the solution, which the job keeps track of.
static long long get_peak_memory_kb(HANDLE job) {
  JOBOBJECT_EXTENDED_LIMIT_INFORMATION job_info;
  ZeroMemory(&job_info, sizeof(job_info));
  if (job == NULL || !QueryInformationJobObject(job, JobObjectExtendedLimitInformation, &job_info, sizeof(job_info), NULL)) {
    return 0;
  }
  return (long long)(job_info.PeakProcessMemoryUsed / 1024);
}

static void make_inheritable(HANDLE handle) {
  if (handle != NULL && handle != INVALID_HANDLE_VALUE) {
    SetHandleInformation(handle, HANDLE_FLAG_INHERIT, HANDLE_FLAG_INHERIT);
  }
}

int main() {
  // Inherited by the solution: a crash must not open an error dialog and block the run.
  SetErrorMode(SEM_FAILCRITICALERRORS | SEM_NOGPFAULTERRORBOX | SEM_NOOPENFILEERRORBOX);

  // UTF-16, since the narrow argv mangles non-ASCII paths.
  int argc = 0;
  LPWSTR *argv = CommandLineToArgvW(GetCommandLineW(), &argc);
  if (argv == NULL || argc < 3) {
    report("ERR", 0, 0);
    return 1;
  }

  const wchar_t *executable = argv[1];
  int time_limit_ms = _wtoi(argv[2]);
  if (time_limit_ms <= 0) {
    time_limit_ms = 1000;
  }
  long long memory_limit_kb = (argc >= 4) ? _wtoi64(argv[3]) : 0;

  // CreateProcessW needs a mutable command line.
  std::wstring quoted;
  quoted.push_back(L'"');
  quoted.append(executable);
  quoted.push_back(L'"');
  std::vector<wchar_t> command_line(quoted.begin(), quoted.end());
  command_line.push_back(L'\0');

  STARTUPINFOW startup_info;
  ZeroMemory(&startup_info, sizeof(startup_info));
  startup_info.cb = sizeof(startup_info);
  startup_info.dwFlags = STARTF_USESTDHANDLES;
  startup_info.hStdInput = GetStdHandle(STD_INPUT_HANDLE);
  startup_info.hStdOutput = GetStdHandle(STD_OUTPUT_HANDLE);
  startup_info.hStdError = GetStdHandle(STD_ERROR_HANDLE);
  make_inheritable(startup_info.hStdInput);
  make_inheritable(startup_info.hStdOutput);
  make_inheritable(startup_info.hStdError);

  PROCESS_INFORMATION process_info;
  ZeroMemory(&process_info, sizeof(process_info));

  // Whole seconds, like RLIMIT_CPU on Unix, so both platforms agree. EZCP
  // applies the exact limit to the reported time.
  const long long cpu_limit_ms = (long long)((time_limit_ms + 999) / 1000) * 1000;

  // The job kills the solution when this timer dies, however it dies, since
  // Windows does not kill children with their parent. Its CPU limit is a kernel
  // backstop; the loop below decides the verdict. Never closed by hand.
  HANDLE job = CreateJobObjectW(NULL, NULL);
  if (job != NULL) {
    JOBOBJECT_EXTENDED_LIMIT_INFORMATION job_limits;
    ZeroMemory(&job_limits, sizeof(job_limits));
    job_limits.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE | JOB_OBJECT_LIMIT_PROCESS_TIME;
    // In 100 ns ticks.
    job_limits.BasicLimitInformation.PerProcessUserTimeLimit.QuadPart = (cpu_limit_ms + 5000) * 10000LL;
    if (memory_limit_kb > 0) {
      const unsigned long long cap = memory_cap_bytes(memory_limit_kb);
      job_limits.BasicLimitInformation.LimitFlags |= JOB_OBJECT_LIMIT_PROCESS_MEMORY;
      // SIZE_T is 32 bits wide in a 32-bit build.
      const SIZE_T largest = ~(SIZE_T)0;
      job_limits.ProcessMemoryLimit = (cap > (unsigned long long)largest) ? largest : (SIZE_T)cap;
    }
    SetInformationJobObject(job, JobObjectExtendedLimitInformation, &job_limits, sizeof(job_limits));
  }

  LARGE_INTEGER frequency, start_counter;
  QueryPerformanceFrequency(&frequency);
  QueryPerformanceCounter(&start_counter);

  // Suspended until it is in the job, so nothing it starts escapes the job.
  if (!CreateProcessW(executable, command_line.data(), NULL, NULL, TRUE, CREATE_NO_WINDOW | CREATE_SUSPENDED, NULL, NULL, &startup_info, &process_info)) {
    report("ERR", 0, 0);
    return 1;
  }

  if (job != NULL) {
    AssignProcessToJobObject(job, process_info.hProcess);
  }

  if (ResumeThread(process_info.hThread) == (DWORD)-1) {
    TerminateProcess(process_info.hProcess, 1);
    CloseHandle(process_info.hProcess);
    CloseHandle(process_info.hThread);
    report("ERR", 0, 0);
    return 1;
  }

  // Otherwise EZCP could block forever writing input nobody reads.
  _close(0);

  const long long wall_deadline_ms = (long long)time_limit_ms * 2 + 2000;
  const char *verdict = "RTE";

  for (;;) {
    DWORD wait_result = WaitForSingleObject(process_info.hProcess, 5);

    if (wait_result == WAIT_OBJECT_0) {
      DWORD exit_code = 1;
      GetExitCodeProcess(process_info.hProcess, &exit_code);
      verdict = (exit_code == 0) ? "OK" : "RTE";
      break;
    }

    if (wait_result == WAIT_FAILED) {
      verdict = "ERR";
      break;
    }

    LARGE_INTEGER now;
    QueryPerformanceCounter(&now);
    long long wall_ms = (frequency.QuadPart > 0) ? ((now.QuadPart - start_counter.QuadPart) * 1000 / frequency.QuadPart) : 0;

    if (get_cpu_time_ms(process_info.hProcess) > cpu_limit_ms || wall_ms > wall_deadline_ms) {
      TerminateProcess(process_info.hProcess, 1);
      WaitForSingleObject(process_info.hProcess, INFINITE);
      verdict = "TLE";
      break;
    }
  }

  long long cpu_time_ms = get_cpu_time_ms(process_info.hProcess);
  long long peak_memory_kb = get_peak_memory_kb(job);

  CloseHandle(process_info.hProcess);
  CloseHandle(process_info.hThread);

  report(verdict, cpu_time_ms, peak_memory_kb);
  return 0;
}

#else

#include <errno.h>
#include <fcntl.h>
#include <signal.h>
#include <stdlib.h>
#include <sys/resource.h>
#include <sys/wait.h>
#include <time.h>
#include <unistd.h>

#ifdef __linux__
#include <sys/prctl.h>
#endif

struct Usage {
  long long cpu_time_ms;
  long long peak_memory_kb;
};

// Only counts reaped children, and the solution is the only child.
static Usage get_child_usage() {
  Usage result = {0, 0};
  struct rusage usage;
  if (getrusage(RUSAGE_CHILDREN, &usage) != 0) {
    return result;
  }
  long long user_ms = (long long)usage.ru_utime.tv_sec * 1000 + usage.ru_utime.tv_usec / 1000;
  long long system_ms = (long long)usage.ru_stime.tv_sec * 1000 + usage.ru_stime.tv_usec / 1000;
  result.cpu_time_ms = user_ms + system_ms;
#ifdef __APPLE__
  result.peak_memory_kb = (long long)usage.ru_maxrss / 1024; // in bytes there
#else
  result.peak_memory_kb = (long long)usage.ru_maxrss;
#endif
  return result;
}

static void report(const char *verdict) {
  Usage usage = get_child_usage();
  report(verdict, usage.cpu_time_ms, usage.peak_memory_kb);
}

static long long get_wall_time_us() {
  struct timespec ts;
  clock_gettime(CLOCK_MONOTONIC, &ts);
  return (long long)ts.tv_sec * 1000000 + ts.tv_nsec / 1000;
}

static void sleep_us(long long us) {
  struct timespec ts;
  ts.tv_sec = us / 1000000;
  ts.tv_nsec = (us % 1000000) * 1000L;
  nanosleep(&ts, NULL);
}

int main(int argc, char *argv[]) {
  if (argc < 3) {
    report("ERR", 0, 0);
    return 1;
  }

  const char *command = argv[1];
  int time_limit_ms = atoi(argv[2]);
  if (time_limit_ms <= 0) {
    time_limit_ms = 1000;
  }
  long long memory_limit_kb = (argc >= 4) ? atoll(argv[3]) : 0;

  // A failed exec writes errno here; a successful one just closes it. Otherwise
  // a failed exec would look like a solution exiting with 127, i.e. a crash.
  int exec_status[2];
  if (pipe(exec_status) != 0 || fcntl(exec_status[1], F_SETFD, FD_CLOEXEC) != 0) {
    report("ERR", 0, 0);
    return 1;
  }

  long long start = get_wall_time_us();

  pid_t parent_pid = getpid();

  pid_t pid = fork();
  if (pid < 0) {
    report("ERR", 0, 0);
    return 1;
  }

  if (pid == 0) {
    close(exec_status[0]);

#ifdef __linux__
    // Kill the solution if the timer dies. The parent may have died already,
    // before this took effect.
    prctl(PR_SET_PDEATHSIG, SIGKILL);
    if (getppid() != parent_pid) {
      _exit(127);
    }
#else
    (void)parent_pid;
#endif

    int limit_s = (time_limit_ms + 999) / 1000;
    struct rlimit cpu_limit;
    cpu_limit.rlim_cur = (rlim_t)limit_s;       // soft limit -> SIGXCPU
    cpu_limit.rlim_max = (rlim_t)(limit_s + 5); // hard limit -> SIGKILL
    setrlimit(RLIMIT_CPU, &cpu_limit);

    // Solutions often recurse very deeply.
    struct rlimit stack_limit;
    if (getrlimit(RLIMIT_STACK, &stack_limit) == 0 && stack_limit.rlim_cur != stack_limit.rlim_max) {
      stack_limit.rlim_cur = stack_limit.rlim_max;
      setrlimit(RLIMIT_STACK, &stack_limit);
    }

    struct rlimit address_space_limit;
    if (getrlimit(RLIMIT_AS, &address_space_limit) == 0) {
      rlim_t wanted = address_space_limit.rlim_max;
#ifdef __linux__
      // macOS does not enforce this limit, so there it is left alone.
      if (memory_limit_kb > 0) {
        const unsigned long long cap = memory_cap_bytes(memory_limit_kb);
        if (wanted == RLIM_INFINITY || (unsigned long long)wanted > cap) {
          wanted = (rlim_t)cap;
        }
      }
#endif
      if (address_space_limit.rlim_cur != wanted) {
        address_space_limit.rlim_cur = wanted;
        setrlimit(RLIMIT_AS, &address_space_limit);
      }
    }

    execl(command, command, (char *)NULL);
    int exec_error = errno;
    if (write(exec_status[1], &exec_error, sizeof(exec_error)) < 0) {
      // The parent then just sees exit status 127.
    }
    _exit(127);
  }

  close(exec_status[1]);
  int exec_error = 0;
  ssize_t exec_report;
  do {
    exec_report = read(exec_status[0], &exec_error, sizeof(exec_error));
  } while (exec_report < 0 && errno == EINTR);
  close(exec_status[0]);

  if (exec_report > 0) {
    waitpid(pid, NULL, 0);
    // Too large to be loaded under the cap, which is far above the limit.
    report((memory_limit_kb > 0 && exec_error == ENOMEM) ? "MLE" : "ERR", 0, 0);
    return 1;
  }

  // Otherwise EZCP could block forever writing input nobody reads.
  close(STDIN_FILENO);

  long long wall_deadline = start + ((long long)time_limit_ms * 2 + 2000) * 1000;

  for (;;) {
    int status = 0;
    pid_t result = waitpid(pid, &status, WNOHANG);

    if (result == pid) {
      if (WIFEXITED(status)) {
        report(WEXITSTATUS(status) == 0 ? "OK" : "RTE");
      } else if (WIFSIGNALED(status)) {
        int signal_number = WTERMSIG(status);
        // SIGXCPU and SIGKILL come from the CPU limits or the wall-clock kill.
        report((signal_number == SIGXCPU || signal_number == SIGKILL) ? "TLE" : "RTE");
      } else {
        report("RTE");
      }
      return 0;
    }

    if (result < 0 && errno != EINTR) {
      report("ERR");
      return 1;
    }

    long long elapsed_us = get_wall_time_us() - start;
    if (start + elapsed_us >= wall_deadline) {
      kill(pid, SIGKILL);
      waitpid(pid, &status, 0);
      report("TLE");
      return 0;
    }

    // An eighth of the time the solution has run for, so that a short run is
    // noticed soon after it ends and a long one is not polled needlessly often.
    long long pause_us = elapsed_us / 8;
    sleep_us(pause_us < 50 ? 50 : (pause_us > 2000 ? 2000 : pause_us));
  }
}

#endif
