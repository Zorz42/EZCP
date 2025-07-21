#[cfg(test)]
#[allow(clippy::unwrap_used)]
pub mod gcc_tests {
    use crate::gcc::Gcc;

    #[test]
    fn test_gcc_new() {
        let gcc = Gcc::new();
        assert!(gcc.is_ok());
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
        gcc.compile(&source_path, &(*tempdir.path()).join("test")).unwrap();

        assert!(tempdir.path().join("test").exists());

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
        "#.replace("KEY", &key.to_string());

        let source_path = tempdir.path().join("test.cpp");
        // Write the source code to a file
        std::fs::write(&source_path, source_code).unwrap();

        let output_path = tempdir.path().join("test_output");
        gcc.compile(&source_path, &output_path).unwrap();

        assert!(output_path.exists());

        // run the compiled program
        let output = std::process::Command::new(&output_path)
            .output()
            .expect("Failed to execute compiled program");

        assert!(output.status.success());

        let output_str = String::from_utf8_lossy(&output.stdout);

        assert!(output_str.contains(&key.to_string()));

        drop(tempdir);
    }
}