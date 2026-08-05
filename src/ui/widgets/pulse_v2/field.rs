use super::types::{DimensionsError, FieldDimensions, MAX_FIELD_SAMPLES};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FieldError {
    Dimensions(DimensionsError),
    DimensionsMismatch,
    OutOfBounds,
    NonFiniteValue,
    ValueOutOfRange,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FieldGradient {
    x: f32,
    y: f32,
}

impl FieldGradient {
    pub fn x(self) -> f32 {
        self.x
    }

    pub fn y(self) -> f32 {
        self.y
    }
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

    pub fn gradient(&self, x: usize, y: usize) -> Option<FieldGradient> {
        let center = self.read(x, y)?;
        let x_gradient = axis_gradient(
            self.read(x.saturating_sub(1), y),
            self.read(x.saturating_add(1), y),
            center,
            x,
            self.width(),
        );
        let y_gradient = axis_gradient(
            self.read(x, y.saturating_sub(1)),
            self.read(x, y.saturating_add(1)),
            center,
            y,
            self.height(),
        );

        Some(FieldGradient {
            x: x_gradient,
            y: y_gradient,
        })
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

fn axis_gradient(
    previous: Option<f32>,
    next: Option<f32>,
    center: f32,
    coordinate: usize,
    length: usize,
) -> f32 {
    if length <= 1 {
        return 0.0;
    }

    let step = 1.0 / (length.saturating_sub(1) as f32);
    let derivative = match (coordinate, previous, next) {
        (0, _, Some(next)) => (next - center) / step,
        (last, Some(previous), _) if last + 1 == length => (center - previous) / step,
        (_, Some(previous), Some(next)) => (next - previous) / (2.0 * step),
        _ => 0.0,
    };

    derivative.clamp(-1.0, 1.0)
}
