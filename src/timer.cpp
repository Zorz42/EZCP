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
