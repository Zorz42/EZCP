use std::rc::Rc;

/// This struct contains all the information about a competitive programming test generator
pub struct TestGenerator {
    
}

/// This struct contains all the information about a competitive programming test
/// It only takes a generator as a closure
pub struct Test {
    generator: Rc<TestGenerator>
}