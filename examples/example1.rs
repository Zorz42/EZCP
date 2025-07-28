use rand::Rng;
use std::path::PathBuf;
use log::LevelFilter;

fn main() {
    // The first task you get an array of integers. You need to find the sum of all elements in the array minus the half of the maximum element.
    // Also all elements in the array are even.

    let mut task = ezcp::Task::new("Coupon", &PathBuf::from("task1"));
    task.debug_level = LevelFilter::Trace;

    // Constraint: n = 1
    let mut subtask1 = ezcp::Subtask::new();

    // This checker is optional and can be omitted.
    subtask1.set_checker(|mut input| {
        // read an array from input
        let array = input.get_array()?;
        // expect end of input
        input.expect_end()?;
        let n = array.len();
        if n != 1 {
            ezcp::bail!("n should be 1");
        }
        // check if the only value is even
        if array[0] % 2 != 0 {
            ezcp::bail!("all array values should be even");
        }
        // check if the only value is in range
        if !(0..=1_000_000_000).contains(&array[0]) {
            ezcp::bail!("all array values should be in range [0, 1_000_000_000]");
        }
        Ok(())
    });

    // add 5 tests where an array is generated with length 1 and even values between 0 and 1_000_000_000 (inclusive)
    subtask1.add_test(5, ezcp::array_generator_custom(1, 1, |rng| rng.random_range(0..=500_000_000) * 2));
    // this is a faulty test for testing the checker
    //subtask1.add_test_str("1\n 0 0\n".to_owned());

    // Constraint: all values are the same
    let mut subtask2 = ezcp::Subtask::new();

    subtask2.set_checker(|mut input| {
        let array = input.get_array()?;
        input.expect_end()?;
        let n = array.len();
        if !(1..=200_000).contains(&n) {
            ezcp::bail!("n should be in range [1, 200_000]");
        }
        let x = array[0];
        for i in array {
            if i != x {
                ezcp::bail!("all array values should be the same");
            }
            if i % 2 != 0 {
                ezcp::bail!("all array values should be even");
            }
            if !(0..=1_000_000_000).contains(&i) {
                ezcp::bail!("all array values should be in range [0, 1_000_000_000]");
            }
        }
        Ok(())
    });

    // add 5 random tests where each test is an array of length between 1 and 200_000 (inclusive) and all values are the same even value between 0 and 1_000_000_000 (inclusive)
    subtask2.add_test(5, || {
        let mut rng = rand::rng();
        let n = rng.random_range(1..=200_000);
        let x = rng.random_range(0..=500_000_000) * 2;
        ezcp::array_to_string(&vec![x; n as usize], true)
    });

    // add an edge case where n is maximal
    let mut rng = rand::rng();
    let x = rng.random_range(0..=500_000_000) * 2;
    subtask2.add_test_str(ezcp::array_to_string(&vec![x; 200_000], true));

    // add 3 edge cases where all values are maximal
    subtask2.add_test(3, ezcp::array_generator(1, 200_000, 1_000_000_000, 1_000_000_000));

    // add an edge case where all values and n are maximal
    subtask2.add_test_str(ezcp::array_to_string(&vec![1_000_000_000; 200_000], true));

    // No additional constraints
    let mut subtask3 = ezcp::Subtask::new();

    subtask3.set_checker(|mut input| {
        let array = input.get_array()?;
        input.expect_end()?;
        let n = array.len();
        if !(1..=200_000).contains(&n) {
            ezcp::bail!("n should be in range [1, 200_000]");
        }
        for i in array {
            if i % 2 != 0 {
                ezcp::bail!("all array values should be even");
            }
            if !(0..=1_000_000_000).contains(&i) {
                ezcp::bail!("all array values should be in range [0, 1_000_000_000]");
            }
        }
        Ok(())
    });

    // add some random tests
    subtask3.add_test(5, ezcp::array_generator_custom(1, 200_000, |rng| rng.random_range(0..=500_000_000) * 2));

    // add 5 edge cases where n is maximal (other edge cases are handled by subtask2)
    subtask3.add_test(5, ezcp::array_generator_custom(200_000, 200_000, |rng| rng.random_range(0..=500_000_000) * 2));

    // add subtasks to task
    let subtask1 = task.add_subtask(subtask1);
    let subtask2 = task.add_subtask(subtask2);
    let subtask3 = task.add_subtask(subtask3);

    // add dependencies (dependencies are only if constraints of a dependency are a subset of constraints of a subtask)
    task.add_subtask_dependency(subtask3, subtask1);
    task.add_subtask_dependency(subtask3, subtask2);

    // there is a partial solution that only reads 2 integers: n, x and prints x / 2 which is correct for subtask1 but should fail for subtask2 and subtask3
    task.add_partial_solution("solution1.cpp", &[subtask1]);

    // finally create the tests
    task.create_tests().ok();
}
