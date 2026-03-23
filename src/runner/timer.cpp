#ifdef _WIN32
#include <chrono>
#include <iostream>
#include <string>
#include <thread>
#include <windows.h>

using namespace std;

long long get_rusage(HANDLE process) {
  FILETIME creationTime, exitTime, kernelTime, userTime;
  GetProcessTimes(process, &creationTime, &exitTime, &kernelTime, &userTime);

  ULARGE_INTEGER userTimeInt, sysTimeInt;
  userTimeInt.LowPart = userTime.dwLowDateTime;
  userTimeInt.HighPart = userTime.dwHighDateTime;
  sysTimeInt.LowPart = kernelTime.dwLowDateTime;
  sysTimeInt.HighPart = kernelTime.dwHighDateTime;

  long long user_ms = userTimeInt.QuadPart / 10000; // Convert to milliseconds
  long long sys_ms = sysTimeInt.QuadPart / 10000;   // Convert to milliseconds

  return user_ms + sys_ms;
}

int main(int argc, const char *argv[]) {
  string command = argv[1];
  int timeout_ms = stoi(argv[2]);

  // Use STARTUPINFOW for wide characters
  STARTUPINFOW si = {sizeof(si)};
  PROCESS_INFORMATION pi;
  ZeroMemory(&pi, sizeof(pi));

  // Convert command to wide string for Windows (needed for CreateProcessW)
  wstring wcommand(command.begin(), command.end());
  // Quote the command path in case it contains spaces and build a mutable
  // buffer
  wstring quoted = L"\"" + wcommand + L"\"";
  // CreateProcessW requires a modifiable buffer for the command line
  vector<wchar_t> cmdline(quoted.begin(), quoted.end());
  cmdline.push_back(L'\0');

  // Use CreateProcessW (wide character version)
  if (!CreateProcessW(NULL, cmdline.data(), NULL, NULL, TRUE, CREATE_NO_WINDOW,
                      NULL, NULL, &si, &pi)) {
    return -1; // Error in creating process
  }

  DWORD waitResult;
  int elapsed = 0;
  int wait_time_ms = 10; // check every 10ms

  while (elapsed < timeout_ms) {
    waitResult = WaitForSingleObject(pi.hProcess, wait_time_ms);
    if (waitResult == WAIT_OBJECT_0) {
      long long elapsed_time = get_rusage(pi.hProcess);
      cerr << elapsed_time << endl; // Output the elapsed time

      DWORD exitCode;
      GetExitCodeProcess(pi.hProcess, &exitCode);
      CloseHandle(pi.hProcess);
      CloseHandle(pi.hThread);
      return exitCode;
    }

    Sleep(wait_time_ms);
    elapsed += wait_time_ms;
  }

  TerminateProcess(pi.hProcess, 1);           // Kill the process
  WaitForSingleObject(pi.hProcess, INFINITE); // Wait for it to terminate

  CloseHandle(pi.hProcess);
  CloseHandle(pi.hThread);

  return 175; // Timeout occurred
}

#else
#include <chrono>
#include <iostream>
#include <signal.h>
#include <sys/resource.h>
#include <sys/wait.h>
#include <time.h>
#include <thread>
#include <unistd.h>
using namespace std;

long long get_wall_time_ms() {
  struct timespec ts;
  clock_gettime(CLOCK_MONOTONIC, &ts);
  return 1LL * ts.tv_sec * 1000 + ts.tv_nsec / 1000000;
}

// Runs `command` with a CPU-time limit of `time_limit_ms` milliseconds.
// Returns:
//   175           – TLE (CPU limit exceeded or wall-time safety-kill)
//   0             – exited successfully
//   positive int  – exit code or signal number for crashes
int run_command_with_timeout(const string &command, int time_limit_ms) {
  pid_t pid = fork();

  if (pid < 0) {
    // fork failed
    return -1;
  }

  if (pid == 0) {
    // Child: set a CPU-time limit before exec so TLE is determined by actual
    // CPU consumption, not wall time. This is unaffected by scheduler delays
    // or parallel test load.
    int limit_s = (time_limit_ms + 999) / 1000; // round up to whole seconds
    struct rlimit rl;
    rl.rlim_cur = (rlim_t)limit_s;       // soft limit  -> SIGXCPU
    rl.rlim_max = (rlim_t)(limit_s + 5); // hard limit  -> SIGKILL
    setrlimit(RLIMIT_CPU, &rl);

    execl(command.c_str(), command.c_str(), (char *)nullptr);
    _exit(127);
  }

  int status;
  bool killed_by_us = false;
  // Safety-net wall-time kill: 10x the CPU limit + 5 s. This only fires when
  // the process is somehow not consuming CPU (e.g., sleeping in a syscall
  // forever). Normal programs and infinite loops are caught by RLIMIT_CPU.
  long long wall_deadline = get_wall_time_ms() + (long long)time_limit_ms * 10 + 5000;

  while (true) {
    pid_t result = waitpid(pid, &status, WNOHANG);
    if (result == pid) {
      if (WIFEXITED(status)) {
        return WEXITSTATUS(status);
      } else if (WIFSIGNALED(status)) {
        int sig = WTERMSIG(status);
        // SIGXCPU: CPU soft limit hit (TLE).
        // SIGKILL: either CPU hard limit hit or our wall-time safety kill.
        if (sig == SIGXCPU || sig == SIGKILL) {
          return 175;
        }
        // Any other signal (SIGSEGV, SIGABRT, …) is a crash.
        return sig;
      }
    }

    if (get_wall_time_ms() >= wall_deadline) {
      kill(pid, SIGKILL);
      killed_by_us = true;
      waitpid(pid, &status, 0);
      return 175;
    }

    this_thread::sleep_for(chrono::milliseconds(10));
  }
}

int main(int argc, const char *argv[]) {
  string command = argv[1];
  int time_limit_ms = stoi(argv[2]);

  long long start = get_wall_time_ms();
  int exit_status = run_command_with_timeout(command, time_limit_ms);
  long long end = get_wall_time_ms();

  if (exit_status == 175) {
    return 175;
  }

  cerr << end - start;
  return exit_status;
}

#endif
