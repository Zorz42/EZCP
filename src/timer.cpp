#include <iostream>
#include <chrono>
#include <unistd.h>
#include <sys/wait.h>
#include <signal.h>
#include <thread>
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
        //cerr << "Fork failed!" << endl;
        return -1;
    }

    if (pid == 0) {
        // Child process: run the command
        execl("/bin/sh", "sh", "-c", command.c_str(), (char *)nullptr);
        // If execl fails
        //cerr << "execl failed!" << endl;
        _exit(127);
    }

    // Parent process
    auto start = chrono::high_resolution_clock::now();

    int status;
    int wait_time_ms = 100; // check every 100ms
    int elapsed = 0;

    while (elapsed < timeout_ms) {
        pid_t result = waitpid(pid, &status, WNOHANG);
        if (result == pid) {
            // Child finished
            auto end = chrono::high_resolution_clock::now();
            auto duration = chrono::duration_cast<chrono::milliseconds>(end - start);
            //cout << "Process finished with exit code " << WEXITSTATUS(status) << endl;
            //cout << "Time taken: " << duration.count() << " ms" << endl;
            return WEXITSTATUS(status);
        }

        this_thread::sleep_for(chrono::milliseconds(wait_time_ms));
        elapsed += wait_time_ms;
    }

    // Timeout expired, kill the process
    //cout << "Process timed out. Terminating..." << endl;
    kill(pid, SIGKILL);
    waitpid(pid, &status, 0); // Clean up

    return 1; // Indicate timeout
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
