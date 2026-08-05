use super::types::{DimensionsError, FieldDimensions, MAX_FIELD_SAMPLES};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FieldError {
    Dimensions(DimensionsError),
    OutOfBounds,
    NonFiniteValue,
    ValueOutOfRange,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ScalarField {
    dimensions: FieldDimensions,
    values: Vec<f32>,
}

impl ScalarField {
    pub fn try_new(dimensions: FieldDimensions) -> Result<Self, FieldError> {
        let sample_count = dimensions.sample_count();
        if sample_count > MAX_FIELD_SAMPLES {
            return Err(FieldError::Dimensions(DimensionsError::SampleCountTooLarge));
        }

        Ok(Self {
            dimensions,
            values: vec![0.0; sample_count],
        })
    }

    pub fn dimensions(&self) -> FieldDimensions {
        self.dimensions
    }

    pub fn width(&self) -> usize {
        self.dimensions.width()
    }

    pub fn height(&self) -> usize {
        self.dimensions.height()
    }

    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
    }

    pub fn as_slice(&self) -> &[f32] {
        &self.values
    }

    pub fn read(&self, x: usize, y: usize) -> Option<f32> {
        if x >= self.width() || y >= self.height() {
            return None;
        }
        Some(self.values[y * self.width() + x])
    }

    pub fn write(&mut self, x: usize, y: usize, value: f32) -> Result<(), FieldError> {
        validate_value(value)?;
        let index = self.index(x, y).ok_or(FieldError::OutOfBounds)?;
        self.values[index] = value;
        Ok(())
    }

    pub fn fill(&mut self, value: f32) -> Result<(), FieldError> {
        validate_value(value)?;
        self.values.fill(value);
        Ok(())
    }

    fn index(&self, x: usize, y: usize) -> Option<usize> {
        if x >= self.width() || y >= self.height() {
            return None;
        }
        Some(y * self.width() + x)
    }
}

fn validate_value(value: f32) -> Result<(), FieldError> {
    if !value.is_finite() {
        return Err(FieldError::NonFiniteValue);
    }
    if !(0.0..=1.0).contains(&value) {
        return Err(FieldError::ValueOutOfRange);
    }
    Ok(())
}
