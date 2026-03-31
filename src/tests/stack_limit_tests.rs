#[cfg(test)]
#[allow(clippy::unwrap_used)]
pub mod stack_limit_tests {
    use crate::runner::cpp_runner::CppRunner;
    use crate::runner::exec_runner::RunResult;
    use crate::tests::test_shared::initialize_logger;
    use tempfile::TempDir;

    #[test]
    fn test_deep_recursion_stack_limit() {
        initialize_logger();

        let tempdir = TempDir::new().unwrap();
        let mut runner = CppRunner::new(tempdir.path()).unwrap();

        // This program uses deep recursion and should exceed the default stack limit (usually 8MB on macOS).
        // 16384 * 1024 bytes = 16MB
        let program_source = r#"
        #include <iostream>

        void recursive_function(int depth) {
            if (depth == 0) return;
            // Use 1KB of stack space per frame
            volatile char large_array[1024];
            for (int i = 0; i < 1024; ++i) large_array[i] = (char)(i % 256);
            recursive_function(depth - 1);
            // Prevent optimization
            if (large_array[0] != 0) std::cout << "sumthin" << std::endl;
        }

        int main() {
            recursive_function(100000); 
            std::cout << "Success" << std::endl;
            return 0;
        }
        "#;

        let program_handle = runner.add_program(program_source).unwrap();
        let results = runner.check_programs("", &[program_handle], 2000).unwrap();
        let result = &results[0];

        // This is expected to FAIL currently (it will return Crashed due to stack overflow)
        assert!(matches!(result, RunResult::Ok(..)), "Expected OK but got {result:?}");

        if let RunResult::Ok(_, output) = result {
            assert_eq!(output.trim(), "Success");
        }

        drop(tempdir);
    }
}
