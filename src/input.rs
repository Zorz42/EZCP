use anyhow::{anyhow, bail, Result};

/// Input is a helper struct for parsing input.
/// It is used by the generators to parse the input string.
/// It ignores different kinds of whitespace.
pub struct Input {
    iter: std::vec::IntoIter<String>,
}

impl Input {
    pub(super) fn new(input_str: &str) -> Self {
        Self {
            iter: input_str.split_whitespace().map(std::borrow::ToOwned::to_owned).collect::<Vec<_>>().into_iter(),
        }
    }

    /// This function returns the next integer in the input.
    /// If there is no next integer, it returns an error.
    pub fn get_int(&mut self) -> Result<i32> {
        self.iter
            .next()
            .ok_or_else(|| anyhow!("Expected integer"))
            .and_then(|s| s.parse().map_err(|_err| anyhow!("Expected integer")))
    }

    /// This function returns the next float in the input.
    /// If there is no next float, it returns an error.
    pub fn get_float(&mut self) -> Result<f32> {
        self.iter
            .next()
            .ok_or_else(|| anyhow!("Expected float"))
            .and_then(|s| s.parse().map_err(|_err| anyhow!("Expected float")))
    }

    /// This function returns the next string in the input.
    /// If there is no next string, it returns an error.
    pub fn get_string(&mut self) -> Result<String> {
        self.iter.next().ok_or_else(|| anyhow!("Expected string"))
    }

    /// This function expects the end of the input.
    /// If the input didn't end yet it returns an error.
    pub fn expect_end(&mut self) -> Result<()> {
        self.iter.clone().peekable().peek().is_none().then_some(()).ok_or_else(|| anyhow!("Expected end of input"))
    }

    /// This function returns the next n integers in the input.
    /// If there are less than n integers in the input, it returns an error.
    pub fn get_ints(&mut self, n: i32) -> Result<Vec<i32>> {
        let mut result = Vec::new();
        for _ in 0..n {
            let next = self.get_int();
            if let Ok(next) = next {
                result.push(next);
            } else {
                bail!("Expected {} integers", n);
            }
        }
        Ok(result)
    }

    /// This function reads the first integer in the input and returns the next n integers in the input.
    /// If there are less than n integers in the input or no integer in the beginning, it returns an error.
    pub fn get_array(&mut self) -> Result<Vec<i32>> {
        let n = self.get_int()?;
        self.get_ints(n)
    }
}
