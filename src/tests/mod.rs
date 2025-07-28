mod array_tests;
mod checker_tests;
mod dependencies_tests;
mod generic_tests;
mod graph_tests;
mod input_tests;
mod partial_solution_tests;
mod gcc_tests;
mod cpp_runner_tests;

#[cfg(test)]
mod test_shared {
    use log::LevelFilter;
    use crate::logger_format::logger_format;
    use crate::task::LOGGER_INIT;

    #[cfg(test)]
    pub fn initialize_logger() {
        LOGGER_INIT.call_once(|| {
            let mut builder = env_logger::builder();
            builder.filter(None, LevelFilter::Trace);
            builder.format(logger_format);
            let _env_logger_instance = builder.build();
            log::set_max_level(LevelFilter::Trace);
        });
    }
}