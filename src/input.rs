pub struct Input {
    elements: Vec<String>,
    iter: std::vec::IntoIter<String>,
}

impl Input {
    pub(super) fn new(input_str: String) -> Self {
        let elements: Vec<_> = input_str.split_whitespace().map(|s| s.to_owned()).collect();
        Self {
            elements: elements.clone(),
            iter: elements.into_iter(),
        }
    }

    pub fn get_int(&mut self) -> Option<i32> {
        self.iter.next().unwrap().parse().ok()
    }

    pub fn expect_int(&mut self) -> i32 {
        self.get_int().unwrap()
    }

    pub fn get_float(&mut self) -> Option<f32> {
        self.iter.next().unwrap().parse().ok()
    }

    pub fn expect_float(&mut self) -> f32 {
        self.get_float().unwrap()
    }

    pub fn get_string(&mut self) -> Option<String> {
        self.iter.next().map(|s| s.to_owned())
    }

    pub fn expect_string(&mut self) -> String {
        self.get_string().unwrap()
    }

    pub fn get_char(&mut self) -> Option<char> {
        self.iter.next().unwrap().chars().next()
    }

    pub fn expect_char(&mut self) -> char {
        self.get_char().unwrap()
    }

    pub fn expect_end(&mut self) {
        assert!(self.iter.next().is_none());
    }

    pub fn get_ints(&mut self, n: i32) -> Option<Vec<i32>> {
        let mut result = Vec::new();
        for _ in 0..n {
            let next = self.get_int();
            if let Some(next) = next {
                result.push(next);
            } else {
                return None;
            }
        }
        Some(result)
    }

    pub fn expect_ints(&mut self, n: i32) -> Vec<i32> {
        self.get_ints(n).unwrap()
    }

    pub fn expect_array(&mut self) -> Vec<i32> {
        let n = self.expect_int();
        self.expect_ints(n)
    }
}
