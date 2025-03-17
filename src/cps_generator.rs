use crate::{Error, Task};

#[derive(serde::Serialize)]
pub struct CPSTests {
    pub tests: Vec<(String, String)>,
    pub subtask_tests: Vec<Vec<usize>>,
    pub subtask_points: Vec<i32>,
}

impl Task {
    /// Generate a CPS compatible
    pub(crate) fn generate_cps_file(&self) -> crate::Result<()> {
        let mut cps_tests = CPSTests {
            tests: Vec::new(),
            subtask_tests: vec![Vec::new(); self.subtasks.len()],
            subtask_points: vec![0; self.subtasks.len()],
        };

        for subtask in &self.subtasks {
            cps_tests.subtask_points[subtask.number] = subtask.points;

            let mut subtask_tests = Vec::new();
            for dependency in &subtask.dependencies {
                subtask_tests.extend_from_slice(&cps_tests.subtask_tests[*dependency]);
            }
            for _test in &subtask.tests {
                let input_file = self.get_input_file_path(cps_tests.tests.len() as i32, subtask.number as i32, subtask_tests.len() as i32);
                let output_file = self.get_output_file_path(cps_tests.tests.len() as i32, subtask.number as i32, subtask_tests.len() as i32);

                let input = std::fs::read_to_string(&input_file).map_err(|err| Error::IOError { err, file: String::new() })?;
                let output = std::fs::read_to_string(&output_file).map_err(|err| Error::IOError { err, file: String::new() })?;

                subtask_tests.push(cps_tests.tests.len());

                cps_tests.tests.push((input, output));
            }
            cps_tests.subtask_tests[subtask.number] = subtask_tests;
        }

        let mut buffer = Vec::new();
        bincode::serialize_into(&mut buffer, &cps_tests).map_err(|err| Error::BincodeError { err })?;
        let data = snap::raw::Encoder::new().compress_vec(&buffer).map_err(|err| Error::SnapError { err })?;
        std::fs::write(&self.cps_tests_archive_path, data).map_err(|err| Error::IOError { err, file: String::new() })?;

        Ok(())
    }
}