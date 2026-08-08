// Timer utility used by EZCP to run one compiled solution under a time limit.
//
// Usage: timer <executable> <time_limit_ms>
//
// The solution inherits stdin/stdout/stderr, so it reads its input and writes
// its answer straight through EZCP's pipes. Once the solution is done the timer
// appends a single result line to stderr:
//
//     \n__EZCP_RESULT__ <OK|TLE|RTE|ERR> <wall_time_ms>\n
//
// EZCP parses the *last* occurrence of that marker, so a solution that writes to
// stderr itself cannot confuse the protocol, and no information has to be
// smuggled through the exit code (exit codes are not portable: Unix reports
// signals separately, Windows reports 32-bit NTSTATUS values).
//
// ERR means the timer could not start the solution at all; every other verdict
// describes the solution itself.
//
// The time limit is enforced on CPU time on both platforms so that a machine
// under load (EZCP runs several solutions in parallel) does not turn correct
// solutions into false TLEs. A wall-clock safety net catches solutions that
// block forever without burning CPU.

#ifdef _WIN32

#define WIN32_LEAN_AND_MEAN
// windows.h has to come first: shellapi.h relies on the types it defines.
#include <windows.h>

#include <io.h>
#include <shellapi.h>
#include <stdio.h>
#include <string>
#include <vector>

static void report(const char *verdict, long long elapsed_ms) {
  fprintf(stderr, "\n__EZCP_RESULT__ %s %lld\n", verdict, elapsed_ms);
  fflush(stderr);
}

static long long filetime_to_ms(const FILETIME &ft) {
  ULARGE_INTEGER value;
  value.LowPart = ft.dwLowDateTime;
  value.HighPart = ft.dwHighDateTime;
  return (long long)(value.QuadPart / 10000ULL); // 100ns ticks -> ms
}

// User + kernel time consumed by the process so far.
static long long get_cpu_time_ms(HANDLE process) {
  FILETIME creation_time, exit_time, kernel_time, user_time;
  if (!GetProcessTimes(process, &creation_time, &exit_time, &kernel_time, &user_time)) {
    return 0;
  }
  return filetime_to_ms(kernel_time) + filetime_to_ms(user_time);
}

static void make_inheritable(HANDLE handle) {
  if (handle != NULL && handle != INVALID_HANDLE_VALUE) {
    SetHandleInformation(handle, HANDLE_FLAG_INHERIT, HANDLE_FLAG_INHERIT);
  }
}

int main() {
  // A crashing solution must never pop up the Windows Error Reporting dialog:
  // it would block the whole test run until somebody clicks it away. Child
  // processes inherit the error mode of their parent.
  SetErrorMode(SEM_FAILCRITICALERRORS | SEM_NOGPFAULTERRORBOX | SEM_NOOPENFILEERRORBOX);

  // Take the arguments as UTF-16 so that paths containing non-ASCII characters
  // (a user name with an accent is enough) survive; the narrow argv would have
  // been mangled by the process code page.
  int argc = 0;
  LPWSTR *argv = CommandLineToArgvW(GetCommandLineW(), &argc);
  if (argv == NULL || argc < 3) {
    report("ERR", 0);
    return 1;
  }

  const wchar_t *executable = argv[1];
  int time_limit_ms = _wtoi(argv[2]);
  if (time_limit_ms <= 0) {
    time_limit_ms = 1000;
  }

  // CreateProcessW needs a modifiable command line buffer. The executable path
  // is passed separately as well, so quoting only has to keep argv[0] intact.
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

  // Round the CPU limit up to whole seconds. Unix has to do that because
  // RLIMIT_CPU only has second granularity, and both platforms must reach the
  // same verdict for the same solution.
  const long long cpu_limit_ms = (long long)((time_limit_ms + 999) / 1000) * 1000;

  // Put the solution in a job object, which gives two guarantees that do not
  // depend on this timer staying alive:
  //   * KILL_ON_JOB_CLOSE - Windows does not kill children together with their
  //     parent, so without it a solution stuck in an endless loop would keep
  //     burning a core forever if this timer were killed instead of exiting on
  //     its own. Closing the last handle (which happens even on abnormal exit)
  //     tears the whole job down.
  //   * PerProcessUserTimeLimit - a kernel enforced CPU backstop, the counterpart
  //     of the RLIMIT_CPU hard limit on Unix. The generous margin over the real
  //     limit keeps the polling loop below in charge of the actual verdict.
  // The handle is deliberately never closed by hand.
  HANDLE job = CreateJobObjectW(NULL, NULL);
  if (job != NULL) {
    JOBOBJECT_EXTENDED_LIMIT_INFORMATION job_limits;
    ZeroMemory(&job_limits, sizeof(job_limits));
    job_limits.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE | JOB_OBJECT_LIMIT_PROCESS_TIME;
    // The field counts 100ns ticks.
    job_limits.BasicLimitInformation.PerProcessUserTimeLimit.QuadPart = (cpu_limit_ms + 5000) * 10000LL;
    SetInformationJobObject(job, JobObjectExtendedLimitInformation, &job_limits, sizeof(job_limits));
  }

  LARGE_INTEGER frequency, start_counter;
  QueryPerformanceFrequency(&frequency);
  QueryPerformanceCounter(&start_counter);

  // Start suspended so the solution cannot fork anything off before it is inside
  // the job.
  if (!CreateProcessW(executable, command_line.data(), NULL, NULL, TRUE, CREATE_NO_WINDOW | CREATE_SUSPENDED, NULL, NULL, &startup_info, &process_info)) {
    report("ERR", 0);
    return 1;
  }

  if (job != NULL) {
    AssignProcessToJobObject(job, process_info.hProcess);
  }

  if (ResumeThread(process_info.hThread) == (DWORD)-1) {
    TerminateProcess(process_info.hProcess, 1);
    CloseHandle(process_info.hProcess);
    CloseHandle(process_info.hThread);
    report("ERR", 0);
    return 1;
  }

  // Drop our own copy of the input pipe. Without this the pipe stays open even
  // after the solution exits, and EZCP would block forever writing input that
  // nobody is going to read.
  _close(0);

  // Solutions that block instead of burning CPU never hit the CPU limit, so
  // keep a wall-clock safety net as well. The multiplier is small so that a
  // sleeping solution does not hold up the whole run.
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

  LARGE_INTEGER end_counter;
  QueryPerformanceCounter(&end_counter);
  long long elapsed_ms = (frequency.QuadPart > 0) ? ((end_counter.QuadPart - start_counter.QuadPart) * 1000 / frequency.QuadPart) : 0;

  CloseHandle(process_info.hProcess);
  CloseHandle(process_info.hThread);

  report(verdict, elapsed_ms);
  return 0;
}

#else

#include <errno.h>
#include <signal.h>
#include <stdio.h>
#include <stdlib.h>
#include <sys/resource.h>
#include <sys/wait.h>
#include <time.h>
#include <unistd.h>

#ifdef __linux__
#include <sys/prctl.h>
#endif

static void report(const char *verdict, long long elapsed_ms) {
  fprintf(stderr, "\n__EZCP_RESULT__ %s %lld\n", verdict, elapsed_ms);
  fflush(stderr);
}

static long long get_wall_time_ms() {
  struct timespec ts;
  clock_gettime(CLOCK_MONOTONIC, &ts);
  return (long long)ts.tv_sec * 1000 + ts.tv_nsec / 1000000;
}

static void sleep_ms(long ms) {
  struct timespec ts;
  ts.tv_sec = ms / 1000;
  ts.tv_nsec = (ms % 1000) * 1000000L;
  nanosleep(&ts, NULL);
}

int main(int argc, char *argv[]) {
  if (argc < 3) {
    report("ERR", 0);
    return 1;
  }

  const char *command = argv[1];
  int time_limit_ms = atoi(argv[2]);
  if (time_limit_ms <= 0) {
    time_limit_ms = 1000;
  }

  long long start = get_wall_time_ms();

  pid_t parent_pid = getpid();

  pid_t pid = fork();
  if (pid < 0) {
    report("ERR", 0);
    return 1;
  }

  if (pid == 0) {
#ifdef __linux__
    // Have the kernel kill the solution if this timer dies without being able to
    // clean up, so an endless loop can never be left burning a core. Re-check the
    // parent afterwards: it may already have died before the call landed, in
    // which case the signal would never be delivered.
    prctl(PR_SET_PDEATHSIG, SIGKILL);
    if (getppid() != parent_pid) {
      _exit(127);
    }
#else
    (void)parent_pid;
#endif

    // Child: limit CPU time before exec, so that TLE reflects the CPU the
    // solution actually consumed and is unaffected by scheduler delays or by
    // the other solutions EZCP runs in parallel.
    int limit_s = (time_limit_ms + 999) / 1000; // round up to whole seconds
    struct rlimit cpu_limit;
    cpu_limit.rlim_cur = (rlim_t)limit_s;       // soft limit -> SIGXCPU
    cpu_limit.rlim_max = (rlim_t)(limit_s + 5); // hard limit -> SIGKILL
    setrlimit(RLIMIT_CPU, &cpu_limit);

    // Raise the stack and address space limits as far as the system allows;
    // competitive programming solutions routinely recurse very deeply.
    struct rlimit stack_limit;
    if (getrlimit(RLIMIT_STACK, &stack_limit) == 0 && stack_limit.rlim_cur != stack_limit.rlim_max) {
      stack_limit.rlim_cur = stack_limit.rlim_max;
      setrlimit(RLIMIT_STACK, &stack_limit);
    }

    struct rlimit address_space_limit;
    if (getrlimit(RLIMIT_AS, &address_space_limit) == 0 && address_space_limit.rlim_cur != address_space_limit.rlim_max) {
      address_space_limit.rlim_cur = address_space_limit.rlim_max;
      setrlimit(RLIMIT_AS, &address_space_limit);
    }

    execl(command, command, (char *)NULL);
    _exit(127);
  }

  // Parent: drop our own copy of the input pipe. Without this the pipe stays
  // open even after the solution exits, and EZCP would block forever writing
  // input that nobody is going to read.
  close(STDIN_FILENO);

  // Solutions that block instead of burning CPU never hit RLIMIT_CPU, so keep a
  // wall-clock safety net as well. The multiplier is small so that a sleeping
  // solution does not hold up the whole run.
  long long wall_deadline = start + (long long)time_limit_ms * 2 + 2000;

  for (;;) {
    int status = 0;
    pid_t result = waitpid(pid, &status, WNOHANG);

    if (result == pid) {
      long long elapsed = get_wall_time_ms() - start;
      if (WIFEXITED(status)) {
        report(WEXITSTATUS(status) == 0 ? "OK" : "RTE", elapsed);
      } else if (WIFSIGNALED(status)) {
        int signal_number = WTERMSIG(status);
        // SIGXCPU: CPU soft limit reached.
        // SIGKILL: CPU hard limit reached, or our wall-clock safety kill.
        report((signal_number == SIGXCPU || signal_number == SIGKILL) ? "TLE" : "RTE", elapsed);
      } else {
        report("RTE", elapsed);
      }
      return 0;
    }

    if (result < 0 && errno != EINTR) {
      report("ERR", get_wall_time_ms() - start);
      return 1;
    }

    if (get_wall_time_ms() >= wall_deadline) {
      kill(pid, SIGKILL);
      waitpid(pid, &status, 0);
      report("TLE", get_wall_time_ms() - start);
      return 0;
    }

    sleep_ms(2);
  }
}

#endif
