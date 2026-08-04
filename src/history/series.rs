pub struct Series {
    values: Vec<f64>,
    limit: usize,
}

impl Series {
    pub fn new(limit: usize) -> Self {
        Self {
            values: Vec::with_capacity(limit),
            limit,
        }
    }

    pub fn push(&mut self, value: f64) {
        if self.limit == 0 {
            return;
        }

        if self.values.len() == self.limit {
            self.values.remove(0);
        }

        self.values.push(value);
    }

    pub fn as_slice(&self) -> &[f64] {
        &self.values
    }
}
