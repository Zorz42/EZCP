#[cfg(test)]
#[allow(clippy::unwrap_used)]
pub mod gcc_tests {
    use crate::Error;
    use crate::runner::gcc::{Gcc, GccOptimization, GccStandard};

    #[test]
    fn test_gcc_new() {
        let gcc = Gcc::new();
        gcc.unwrap();
    }

    #[test]
    fn test_gcc_compile() {
        let gcc = Gcc::new().unwrap();

        let tempdir = tempfile::TempDir::new().unwrap();

        let source_code = r#"
        #include <iostream>
        using namespace std;

        int main() {
            cout << "Hello, World!" << endl;
            return 0;
        }
        "#;

        let source_path = tempdir.path().join("test.cpp");
        // Write the source code to a file
        std::fs::write(&source_path, source_code).unwrap();
        let out_file = gcc.compile(&source_path, None).unwrap();

        assert!(out_file.exists());

        drop(tempdir);
    }

    #[test]
    fn test_gcc_compile_with_flags() {
        let mut gcc = Gcc::new().unwrap();
        gcc.optimization = Some(GccOptimization::Level2);
        gcc.standard = Some(GccStandard::Cpp17);

        let tempdir = tempfile::TempDir::new().unwrap();

        let source_code = r#"
        #include <iostream>
        using namespace std;

        int main() {
            cout << "Hello, World!" << endl;
            return 0;
        }
        "#;

        let source_path = tempdir.path().join("test.cpp");
        // Write the source code to a file
        std::fs::write(&source_path, source_code).unwrap();
        let out_file = gcc.compile(&source_path, None).unwrap();

        assert!(out_file.exists());

        drop(tempdir);
    }

    #[test]
    fn test_gcc_compile_output() {
        let gcc = Gcc::new().unwrap();

        let tempdir = tempfile::TempDir::new().unwrap();

        let key = rand::random::<u64>();

        let source_code = r#"
        #include <iostream>
        using namespace std;

        int main() {
            cout << "KEY" << endl;
            return 0;
        }
        "#
        .replace("KEY", &key.to_string());

        let source_path = tempdir.path().join("test.cpp");
        // Write the source code to a file
        std::fs::write(&source_path, source_code).unwrap();

        let output_path = gcc.compile(&source_path, None).unwrap();

        assert!(output_path.exists());

        // run the compiled program
        let output = std::process::Command::new(&output_path).output().unwrap();

        assert!(output.status.success());

        let output_str = String::from_utf8_lossy(&output.stdout);

        assert!(output_str.contains(&key.to_string()));

        drop(tempdir);
    }

    #[test]
    fn test_transform_output_file_extension() {
        let tempdir = tempfile::TempDir::new().unwrap();
        let source_path = tempdir.path().join("foo.cpp");
        std::fs::write(&source_path, "int main(){return 0;}").unwrap();

        // No explicit output path → based on source
        let transformed = Gcc::transform_output_file(&source_path, None).unwrap();

        #[cfg(windows)]
        assert_eq!(transformed.extension().and_then(|e| e.to_str()), Some("exe"));

        #[cfg(unix)]
        {
            // No extension on Unix; filename should be exactly "foo"
            assert_eq!(transformed.extension(), None);
            assert_eq!(transformed.file_name().unwrap().to_string_lossy(), "foo");
        }

        drop(tempdir);
    }

    #[test]
    fn test_compile_error() {
        let gcc = Gcc::new().unwrap();

        let tempdir = tempfile::TempDir::new().unwrap();

        let source_code = r#"
        #include <iostream>
        using namespace std;

        int main() {
        fdsahfjkasfhjk;
            cout << "Hello, World!" << endl;
            return 0;
        }
        "#;

        let source_path = tempdir.path().join("test.cpp");
        // Write the source code to a file
        std::fs::write(&source_path, source_code).unwrap();
        assert!(matches!(gcc.compile(&source_path, None), Err(Error::CompilerError { .. })));

        drop(tempdir);
    }

    // --- Additional GCC coverage from ANALYSIS.md ---

    fn compile_hello_world_with(gcc: &Gcc) {
        let tempdir = tempfile::TempDir::new().unwrap();
        let source_code = "int main() { return 0; }";
        let source_path = tempdir.path().join("test.cpp");
        std::fs::write(&source_path, source_code).unwrap();
        let out = gcc.compile(&source_path, None).unwrap();
        assert!(out.exists());
    }

    #[test]
    fn test_gcc_standard_cpp11() {
        let mut gcc = Gcc::new().unwrap();
        gcc.standard = Some(GccStandard::Cpp11);
        compile_hello_world_with(&gcc);
    }

    #[test]
    fn test_gcc_standard_cpp14() {
        let mut gcc = Gcc::new().unwrap();
        gcc.standard = Some(GccStandard::Cpp14);
        compile_hello_world_with(&gcc);
    }

    #[test]
    fn test_gcc_standard_cpp17() {
        let mut gcc = Gcc::new().unwrap();
        gcc.standard = Some(GccStandard::Cpp17);
        compile_hello_world_with(&gcc);
    }

    #[test]
    fn test_gcc_standard_cpp20() {
        let mut gcc = Gcc::new().unwrap();
        gcc.standard = Some(GccStandard::Cpp20);
        compile_hello_world_with(&gcc);
    }

    #[test]
    fn test_gcc_optimization_level1() {
        let mut gcc = Gcc::new().unwrap();
        gcc.optimization = Some(GccOptimization::Level1);
        compile_hello_world_with(&gcc);
    }

    #[test]
    fn test_gcc_optimization_level2() {
        let mut gcc = Gcc::new().unwrap();
        gcc.optimization = Some(GccOptimization::Level2);
        compile_hello_world_with(&gcc);
    }

    #[test]
    fn test_gcc_optimization_level3() {
        let mut gcc = Gcc::new().unwrap();
        gcc.optimization = Some(GccOptimization::Level3);
        compile_hello_world_with(&gcc);
    }

    #[test]
    fn test_gcc_optimization_small() {
        let mut gcc = Gcc::new().unwrap();
        gcc.optimization = Some(GccOptimization::Small);
        compile_hello_world_with(&gcc);
    }

    #[test]
    fn test_gcc_optimization_fast() {
        let mut gcc = Gcc::new().unwrap();
        gcc.optimization = Some(GccOptimization::Fast);
        compile_hello_world_with(&gcc);
    }

    #[test]
    fn test_gcc_standard_as_str_values() {
        assert_eq!(GccStandard::Cpp98.as_str(), "c++98");
        assert_eq!(GccStandard::Cpp11.as_str(), "c++11");
        assert_eq!(GccStandard::Cpp14.as_str(), "c++14");
        assert_eq!(GccStandard::Cpp17.as_str(), "c++17");
        assert_eq!(GccStandard::Cpp20.as_str(), "c++20");
        assert_eq!(GccStandard::Cpp23.as_str(), "c++23");
    }

    #[test]
    fn test_gcc_optimization_as_str_values() {
        assert_eq!(GccOptimization::Level1.as_str(), "1");
        assert_eq!(GccOptimization::Level2.as_str(), "2");
        assert_eq!(GccOptimization::Level3.as_str(), "3");
        assert_eq!(GccOptimization::Small.as_str(), "s");
        assert_eq!(GccOptimization::Fast.as_str(), "fast");
    }
}
