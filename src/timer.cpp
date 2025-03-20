#ifdef _WIN32
#include <iostream>
#include <chrono>
#include <Windows.h>
#include <thread>
#include <string>

using namespace std;

long long get_rusage(HANDLE process) {
    FILETIME creationTime, exitTime, kernelTime, userTime;
    GetProcessTimes(process, &creationTime, &exitTime, &kernelTime, &userTime);

    ULARGE_INTEGER userTimeInt, sysTimeInt;
    userTimeInt.LowPart = userTime.dwLowDateTime;
    userTimeInt.HighPart = userTime.dwHighDateTime;
    sysTimeInt.LowPart = kernelTime.dwLowDateTime;
    sysTimeInt.HighPart = kernelTime.dwHighDateTime;

    long long user_ms = userTimeInt.QuadPart / 10000;  // Convert to milliseconds
    long long sys_ms = sysTimeInt.QuadPart / 10000;    // Convert to milliseconds

    return user_ms + sys_ms;
}

int run_command_with_timeout(const string& command, int timeout_ms) {
    // Use STARTUPINFOW for wide characters
    STARTUPINFOW si = { sizeof(si) };
    PROCESS_INFORMATION pi;
    ZeroMemory(&pi, sizeof(pi));

    // Convert command to wide string for Windows (needed for CreateProcessW)
    wstring wcommand(command.begin(), command.end());

    // Use CreateProcessW (wide character version)
    if (!CreateProcessW(
            NULL, &wcommand[0], NULL, NULL, FALSE, 0, NULL, NULL, &si, &pi)) {
        return -1;  // Error in creating process
    }

    DWORD waitResult;
    int elapsed = 0;
    int wait_time_ms = 10; // check every 10ms

    while (elapsed < timeout_ms) {
        waitResult = WaitForSingleObject(pi.hProcess, wait_time_ms);
        if (waitResult == WAIT_OBJECT_0) {
            DWORD exitCode;
            GetExitCodeProcess(pi.hProcess, &exitCode);
            CloseHandle(pi.hProcess);
            CloseHandle(pi.hThread);
            return exitCode;
        }

        this_thread::sleep_for(chrono::milliseconds(wait_time_ms));
        elapsed += wait_time_ms;
    }

    TerminateProcess(pi.hProcess, 1); // Kill the process
    WaitForSingleObject(pi.hProcess, INFINITE);  // Wait for it to terminate

    CloseHandle(pi.hProcess);
    CloseHandle(pi.hThread);

    return 1; // Timeout occurred
}

int main(int argc, const char* argv[]) {
    string command = argv[1];
    int time_limit_ms = stoi(argv[2]);

    // Start the command and measure its execution time
    auto start = chrono::high_resolution_clock::now();

    int exit_status = run_command_with_timeout(command, time_limit_ms * 2);

    auto end = chrono::high_resolution_clock::now();
    auto elapsed = end - start;

    if (elapsed.count() > time_limit_ms)
        return 1;

    cerr << elapsed.count() << endl;
    return exit_status;
}

#else
#include <iostream>
#include <chrono>
#include <unistd.h>
#include <sys/wait.h>
#include <signal.h>
#include <thread>
#include <sys/resource.h>
using namespace std;


long long get_rusage() {
    rusage res;
    getrusage(RUSAGE_CHILDREN, &res);
    timeval user = res.ru_utime;
    timeval sys = res.ru_stime;
    long long user_ms = 1LL * user.tv_sec * 1000 + 1LL * user.tv_usec / 1000;
    long long sys_ms = 1LL * sys.tv_sec * 1000 + 1LL * sys.tv_usec / 1000;
    return user_ms + sys_ms;
}

int run_command_with_timeout(const string& command, int timeout_ms) {
    pid_t pid = fork();

    if (pid < 0) {
        // fork failed
        return -1;
    }

    if (pid == 0) {
        exit(execl(command.c_str(), (char*) nullptr));
    }

    int status;
    int wait_time_ms = 10; // check every 10ms
    int elapsed = 0;

    while (elapsed < timeout_ms) {
        pid_t result = waitpid(pid, &status, WNOHANG);
        if (result == pid) {
            return WEXITSTATUS(status);
        }

        this_thread::sleep_for(chrono::milliseconds(wait_time_ms));
        elapsed += wait_time_ms;
    }

    kill(pid, SIGKILL);
    waitpid(pid, &status, 0);

    return 1;
}

int main(int argc, const char* argv[]) {
    string command = argv[1];
    int time_limit_ms = stoi(argv[2]);
    
    long long start = get_rusage();
    int exit_status = run_command_with_timeout(command, time_limit_ms * 2);
    long long end = get_rusage();
    
    long long elapsed = end - start;

    if(elapsed > time_limit_ms)
        return 1;
    
    cerr << end - start;
    return exit_status;
}
#endif
