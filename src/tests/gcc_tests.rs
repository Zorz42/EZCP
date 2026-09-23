#[cfg(test)]
#[allow(clippy::unwrap_used)]
pub mod gcc_tests {
    use crate::runner::gcc::Gcc;
    use crate::{Error, Result};
    use std::path::{Path, PathBuf};
    use tempfile::TempDir;

    fn compile(dir: &Path, source_code: &str) -> Result<PathBuf> {
        let source = dir.join("test.cpp");
        std::fs::write(&source, source_code).unwrap();
        let executable = Gcc::executable_path(&source)?;
        Gcc::new()?.compile(&source, &executable)?;
        Ok(executable)
    }

    #[test]
    fn test_gcc_compile_and_run() {
        let tempdir = TempDir::new().unwrap();
        let executable = compile(tempdir.path(), "#include <iostream>\nint main() { std::cout << 1234567 << std::endl; }").unwrap();

        let output = std::process::Command::new(&executable).output().unwrap();
        assert!(output.status.success());
        assert!(String::from_utf8_lossy(&output.stdout).contains("1234567"));
    }

    #[test]
    fn test_compile_error() {
        let tempdir = TempDir::new().unwrap();
        assert!(matches!(compile(tempdir.path(), "int main() { fdsahfjkasfhjk; }"), Err(Error::CompilerError { .. })));
    }

    #[test]
    fn test_executable_path_extension() {
        let executable = Gcc::executable_path(&TempDir::new().unwrap().path().join("foo.cpp")).unwrap();

        #[cfg(windows)]
        assert_eq!(executable.file_name().unwrap(), "foo.exe");
        #[cfg(unix)]
        assert_eq!(executable.file_name().unwrap(), "foo");
    }

    #[test]
    fn test_executable_path_never_overwrites_the_source() {
        let tempdir = TempDir::new().unwrap();
        let source = tempdir.path().join("noextension");
        std::fs::write(&source, "int main(){return 0;}").unwrap();

        let executable = Gcc::executable_path(&source).unwrap();

        assert_ne!(executable, source);
        assert!(executable.is_absolute(), "{executable:?} should be absolute");
        assert!(!executable.exists(), "{executable:?} should not have been created");
    }

    #[test]
    fn test_executable_path_stays_in_its_directory() {
        let tempdir = TempDir::new().unwrap();
        let executable = Gcc::executable_path(&tempdir.path().join("prog.cpp")).unwrap();
        assert_eq!(executable.parent(), Some(dunce::canonicalize(tempdir.path()).unwrap().as_path()));
    }
}
