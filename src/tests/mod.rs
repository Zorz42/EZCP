mod array_tests;
mod checker_tests;
mod dependencies_tests;
mod generic_tests;
mod graph_tests;
mod input_tests;
mod partial_solution_tests;
mod gcc_tests;
mod runner_tests;

#[cfg(test)]
mod test_shared {
    use std::sync::Once;
    use log::LevelFilter;
    use crate::task::CustomPrefixToken;

    #[cfg(test)]
    static INIT: Once = Once::new();

    #[cfg(test)]
    pub fn initialize_logger() {
        INIT.call_once(|| {
            let mut builder = colog::default_builder();
            builder.filter_level(LevelFilter::Trace);
            builder.format(colog::formatter(CustomPrefixToken));
            builder.init();
        });
    }
}