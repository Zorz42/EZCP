use anyhow::{anyhow, bail, Result};

pub struct Input {
    iter: std::vec::IntoIter<String>,
}

impl Input {
    pub(super) fn new(input_str: &str) -> Self {
        Self {
            iter: input_str.split_whitespace().map(std::borrow::ToOwned::to_owned).collect::<Vec<_>>().into_iter(),
        }
    }

    pub fn get_int(&mut self) -> Result<i32> {
        self.iter
            .next()
            .ok_or_else(|| anyhow!("Expected integer"))
            .and_then(|s| s.parse().map_err(|_err| anyhow!("Expected integer")))
    }

    pub fn get_float(&mut self) -> Result<f32> {
        self.iter
            .next()
            .ok_or_else(|| anyhow!("Expected float"))
            .and_then(|s| s.parse().map_err(|_err| anyhow!("Expected float")))
    }

    pub fn get_string(&mut self) -> Result<String> {
        self.iter.next().ok_or_else(|| anyhow!("Expected string"))
    }

    pub fn get_char(&mut self) -> Result<char> {
        self.iter
            .next()
            .ok_or_else(|| anyhow!("Expected char"))
            .and_then(|s| s.chars().next().ok_or_else(|| anyhow!("Expected char")))
    }

    pub fn expect_end(&mut self) -> Result<()> {
        self.iter.next().is_none().then_some(()).ok_or_else(|| anyhow!("Expected end of input"))
    }

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

    pub fn get_array(&mut self) -> Result<Vec<i32>> {
        let n = self.get_int()?;
        self.get_ints(n)
    }
}
