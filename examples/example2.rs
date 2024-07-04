use rand::Rng;
use std::path::PathBuf;

fn main() {
    // In this task you have n coins with values a1, a2, ..., an. You need to find the smallest sum, you cannot get using these coins.
    // For example, if you have coins with values 1, 2 and 4, you can get any sum from 1 to 7, but you cannot get 8.

    let mut task = ezcp::Task::new("Coins", &PathBuf::from("task2"));

    // Constraint: n = 1
    let mut subtask1 = ezcp::Subtask::new(10);

    subtask1.set_checker(|mut input| {
        let array = input.get_array()?;
        input.expect_end()?;
        let n = array.len();
        if n != 1 {
            ezcp::bail!("n should be 1");
        }
        let x = array[0];
        if !(1..=1_000_000_000).contains(&x) {
            ezcp::bail!("all array values should be in range [1, 1_000_000_000]");
        }
        Ok(())
    });

    subtask1.add_test(5, ezcp::array_generator(1, 1, 1, 1000));
    subtask1.add_test_str("1\n 1\n".to_owned());

    // Constraint: elements in the array are powers of 2 and n <= 30
    let mut subtask2 = ezcp::Subtask::new(20);

    subtask2.set_checker(|mut input| {
        let array = input.get_array()?;
        input.expect_end()?;
        let n = array.len();
        if !(1..=30).contains(&n) {
            ezcp::bail!("n should be in range [1, 30]");
        }
        for (i, x) in array.iter().enumerate() {
            if *x != 1 << i {
                ezcp::bail!("all array values should be powers of 2");
            }
        }
        Ok(())
    });

    subtask2.add_test(5, || {
        let mut rng = rand::thread_rng();
        let n = rng.gen_range(1..=30);
        let mut array = Vec::new();
        for i in 0..n {
            array.push(1 << i);
        }
        ezcp::array_to_string(&array, true)
    });

    // Constraint: n <= 1000
    let mut subtask3 = ezcp::Subtask::new(50);

    subtask3.set_checker(|mut input| {
        let array = input.get_array()?;
        input.expect_end()?;
        let n = array.len();
        if !(1..=1000).contains(&n) {
            ezcp::bail!("n should be in range [1, 1000]");
        }
        for x in array {
            if !(1..=1_000_000_000).contains(&x) {
                ezcp::bail!("all array values should be in range [1, 1_000_000_000]");
            }
        }
        Ok(())
    });

    subtask3.add_test(5, ezcp::array_generator(1, 1000, 1, 1000));
    subtask3.add_test(5, ezcp::array_generator(1, 1000, 1, 1_000_000_000));
    subtask3.add_test(5, ezcp::array_generator(1, 1000, 1, 1));
    subtask3.add_test(5, ezcp::array_generator(1000, 1000, 1, 1000));
    subtask3.add_test(5, ezcp::array_generator(1000, 1000, 1, 1_000_000_000));
    subtask3.add_test(1, ezcp::array_generator(1000, 1000, 1, 1));

    // Constraint: n <= 200_000
    let mut subtask4 = ezcp::Subtask::new(20);

    subtask4.set_checker(|mut input| {
        let array = input.get_array()?;
        input.expect_end()?;
        let n = array.len();
        if !(1..=200_000).contains(&n) {
            ezcp::bail!("n should be in range [1, 200_000]");
        }
        for x in array {
            if !(1..=1_000_000_000).contains(&x) {
                ezcp::bail!("all array values should be in range [1, 1_000_000_000]");
            }
        }
        Ok(())
    });

    subtask4.add_test(5, ezcp::array_generator(1, 200_000, 1, 1000));
    subtask4.add_test(5, ezcp::array_generator(1, 200_000, 1, 1_000_000_000));
    subtask4.add_test(5, ezcp::array_generator(1, 200_000, 1, 1));
    subtask4.add_test(5, ezcp::array_generator(200_000, 200_000, 1, 1000));
    subtask4.add_test(5, ezcp::array_generator(200_000, 200_000, 1, 1_000_000_000));
    subtask4.add_test(1, ezcp::array_generator(200_000, 200_000, 1, 1));

    // add subtasks to task
    let subtask1 = task.add_subtask(subtask1);
    let subtask2 = task.add_subtask(subtask2);
    let subtask3 = task.add_subtask(subtask3);
    let subtask4 = task.add_subtask(subtask4);

    // add dependencies
    task.add_subtask_dependency(subtask3, subtask1);
    task.add_subtask_dependency(subtask3, subtask2);
    task.add_subtask_dependency(subtask4, subtask3);

    task.create_tests().ok();
}
