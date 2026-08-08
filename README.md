# EZCP
A Rust framework to easily create tasks for competitive programming.

Features:
- Generate test inputs and save them into files.
- Generate correct outputs.
- Make a solution checker if there are multiple valid solutions.
- Graph generator.
- Array generator.
- Add a partial solution and specify which subtasks it should pass.
- Automatically search for testcases that break all bad solutions.
- Automatically archive all test files into a zip file.

See `examples/` for more information.

## Requirements

EZCP runs on Linux, macOS and Windows. It needs:

- Rust 1.88 or newer.
- A C++ compiler, because solutions are compiled and executed:
  - **Linux** — `g++` (e.g. `apt install g++`).
  - **macOS** — `g++`/`clang++` from the Xcode command line tools (`xcode-select --install`).
  - **Windows** — a MinGW-w64 toolchain, for example via [MSYS2](https://www.msys2.org)
    (`pacman -S mingw-w64-ucrt-x86_64-gcc`) or `choco install mingw`.

The compiler is looked up on `PATH`, plus the usual install locations on Windows.
Set the `GCC_PATH` environment variable to point at a specific compiler.

Solutions get a large stack (512 MB where the platform allows it), and their time
limit is measured in CPU time, so a machine under load does not turn a correct
solution into a timeout.

