#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod array_tests {
    use crate::tests::generic_tests::generic_tests::Test;
    use crate::{Subtask, array_generator};

    #[test]
    fn test_array_generator() {
        let mut task = Test::new();

        // create solution file
        let solution_contents = r#"
        #include <iostream>
        using namespace std;
        
        int main() {
            int n;
            cin>>n;
            int a[n];
            for(int i=0;i<n;i++) {
                cin>>a[i];
            }
            cout<<"1\n";
            return 0; 
        }
        
        "#;

        task.task = task.task.with_solution_source(solution_contents);

        // create subtasks
        let subtask1 = Subtask::new("")
            .with_test(5, array_generator(1, 100, 1, 100))
            .with_test(5, array_generator(1, 100, 1, 1))
            .with_test(5, array_generator(100, 100, 1, 100))
            .with_test(5, array_generator(100, 100, 1, 1))
            .with_test(1, array_generator(100, 100, 1, 1));

        // n = 42
        let subtask2 = Subtask::new("")
            .with_test(5, array_generator(42, 42, 1, 100))
            .with_test(5, array_generator(42, 42, 1, 1))
            .with_test(5, array_generator(42, 42, 100, 100));

        // all values are 47
        let subtask3 = Subtask::new("")
            .with_test(5, array_generator(1, 100, 47, 47))
            .with_test(5, array_generator(100, 100, 47, 47))
            .with_test(5, array_generator(1, 1, 47, 47));

        task.task = task.task.with_subtask(subtask1);
        task.task = task.task.with_subtask(subtask2);
        task.task = task.task.with_subtask(subtask3);

        task.task.run().unwrap();
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod array_unit_tests {
    use crate::{array_generator, array_to_string};

    #[test]
    fn test_array_to_string_empty_with_count() {
        let arr = vec![];
        let result = array_to_string(&arr, true);
        // Should contain the count (0) then a newline for the (empty) elements line
        assert!(result.starts_with("0\n"), "expected count line, got: {result:?}");
    }

    #[test]
    fn test_array_to_string_empty_no_count() {
        let arr: Vec<i32> = vec![];
        let result = array_to_string(&arr, false);
        // No count line; just the trailing newline for the element line
        assert!(!result.starts_with('0'), "unexpected count prefix in: {result:?}");
        assert_eq!(result, "\n");
    }

    #[test]
    fn test_array_to_string_single_element_with_count() {
        let arr = vec![7];
        let result = array_to_string(&arr, true);
        assert!(result.starts_with("1\n"), "count line missing, got: {result:?}");
        assert!(result.contains('7'));
    }

    #[test]
    fn test_array_to_string_no_count_omits_count_line() {
        let arr = vec![1, 2, 3];
        let result_with = array_to_string(&arr, true);
        let result_without = array_to_string(&arr, false);
        // With count has one extra line at the front
        let lines_with: Vec<&str> = result_with.lines().collect();
        assert_eq!(lines_with.len(), result_without.lines().count() + 1);
        assert_eq!(lines_with[0], "3");
    }

    #[test]
    fn test_array_to_string_values_present() {
        let arr = vec![10, 20, 30];
        let result = array_to_string(&arr, false);
        assert!(result.contains("10"));
        assert!(result.contains("20"));
        assert!(result.contains("30"));
    }

    #[test]
    fn test_array_generator_min_equals_max_length() {
        let generator = array_generator(5, 5, 1, 100);
        for _ in 0..20 {
            let output = generator();
            let mut lines = output.lines();
            let count: usize = lines.next().unwrap().trim().parse().unwrap();
            assert_eq!(count, 5, "expected length 5");
            assert_eq!(lines.next().unwrap_or("").split_whitespace().filter_map(|s| s.parse::<i32>().ok()).count(), 5, "expected 5 elements");
        }
    }

    #[test]
    fn test_array_generator_values_in_range() {
        let min_x = 10;
        let max_x = 20;
        let generator = array_generator(1, 50, min_x, max_x);
        for _ in 0..50 {
            let output = generator();
            let mut lines = output.lines();
            let _count_line = lines.next();
            if let Some(elem_line) = lines.next() {
                for tok in elem_line.split_whitespace() {
                    let v: i32 = tok.parse().unwrap();
                    assert!(v >= min_x && v <= max_x, "value {v} out of range [{min_x}, {max_x}]");
                }
            }
        }
    }
}
