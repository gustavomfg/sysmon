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

    /// Deposits bounded energy at a normalized position. This is an internal
    /// construction primitive for the V2 organic-motion stage; it does not
    /// expose geometry or terminal concepts.
    pub(crate) fn deposit_normalized(
        &mut self,
        position: (f32, f32),
        energy: f32,
        radius: f32,
    ) -> Result<(), FieldError> {
        validate_value(energy)?;
        if !position.0.is_finite() || !position.1.is_finite() || !radius.is_finite() {
            return Err(FieldError::NonFiniteValue);
        }
        if radius < 0.0 {
            return Err(FieldError::ValueOutOfRange);
        }
        if self.is_empty() {
            return Ok(());
        }

        if !(0.0..=1.0).contains(&position.0) {
            return Ok(());
        }
        let center_x = position.0 * self.width().saturating_sub(1) as f32;
        let center_y = position.1.clamp(0.0, 1.0) * self.height().saturating_sub(1) as f32;
        let radius = radius.max(0.25);
        let left = (center_x - radius).floor().max(0.0) as usize;
        let right = (center_x + radius)
            .ceil()
            .min(self.width().saturating_sub(1) as f32) as usize;
        let top = (center_y - radius).floor().max(0.0) as usize;
        let bottom = (center_y + radius)
            .ceil()
            .min(self.height().saturating_sub(1) as f32) as usize;

        for y in top..=bottom {
            for x in left..=right {
                let distance = (x as f32 - center_x).hypot(y as f32 - center_y);
                let falloff = smoothstep(1.0 - distance / radius);
                if falloff <= 0.0 {
                    continue;
                }
                let index = y * self.width() + x;
                self.values[index] = (self.values[index] + energy * falloff * 0.46).clamp(0.0, 1.0);
            }
        }

        Ok(())
    }

    /// Performs a small bounded diffusion pass for deposited energy. The
    /// temporary buffer is allocated once per pass, never per sample.
    pub(crate) fn diffuse(&mut self, amount: f32) {
        if self.is_empty() {
            return;
        }

        let amount = amount.clamp(0.0, 1.0);
        let mut next = vec![0.0; self.values.len()];
        for y in 0..self.height() {
            for x in 0..self.width() {
                let index = y * self.width() + x;
                let current = self.values[index];
                let mut weighted = current * 0.42;
                let mut total_weight = 0.42;

                for neighbor_y in y.saturating_sub(1)..=(y + 1).min(self.height() - 1) {
                    for neighbor_x in x.saturating_sub(1)..=(x + 1).min(self.width() - 1) {
                        if neighbor_x == x && neighbor_y == y {
                            continue;
                        }
                        let weight = if neighbor_x == x || neighbor_y == y {
                            0.10
                        } else {
                            0.045
                        };
                        weighted += self.values[neighbor_y * self.width() + neighbor_x] * weight;
                        total_weight += weight;
                    }
                }

                next[index] =
                    (current * (1.0 - amount) + weighted / total_weight * amount).clamp(0.0, 1.0);
            }
        }
        self.values.copy_from_slice(&next);
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

fn smoothstep(value: f32) -> f32 {
    let value = value.clamp(0.0, 1.0);
    value * value * (3.0 - 2.0 * value)
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
