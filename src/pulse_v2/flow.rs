use super::{FieldError, ScalarField};

/// A local flow sample derived from the scalar energy landscape.
///
/// Direction is a normalized uphill direction when the scalar field has a
/// usable gradient. Flat regions intentionally contain a zero vector. The
/// tangent is exposed as the perpendicular basis of that same direction; it
/// is not an independent flow source.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct FlowSample {
    direction_x: f32,
    direction_y: f32,
    intensity: f32,
    convergence: f32,
}

impl FlowSample {
    pub fn direction_x(self) -> f32 {
        self.direction_x
    }

    pub fn direction_y(self) -> f32 {
        self.direction_y
    }

    pub fn tangent_x(self) -> f32 {
        -self.direction_y
    }

    pub fn tangent_y(self) -> f32 {
        self.direction_x
    }

    pub fn intensity(self) -> f32 {
        self.intensity
    }

    /// Positive values indicate local convergence; negative values indicate
    /// local divergence. The range is `-1.0..=1.0`.
    pub fn convergence(self) -> f32 {
        self.convergence
    }

    pub fn direction_magnitude(self) -> f32 {
        self.direction_x.hypot(self.direction_y)
    }
}

/// A terminal-independent vector field derived exclusively from a
/// [`ScalarField`]. It contains no particles, trajectories or rasterization
/// data.
#[derive(Debug, Clone, PartialEq)]
pub struct FlowField {
    dimensions: super::FieldDimensions,
    values: Vec<FlowSample>,
}

impl FlowField {
    pub fn from_scalar(field: &ScalarField) -> Result<Self, FieldError> {
        let dimensions = field.dimensions();
        let mut values = vec![FlowSample::default(); dimensions.sample_count()];

        for y in 0..dimensions.height() {
            for x in 0..dimensions.width() {
                let index = y * dimensions.width() + x;
                let energy = field.read(x, y).ok_or(FieldError::OutOfBounds)?;
                let gradient = field.gradient(x, y).ok_or(FieldError::OutOfBounds)?;
                let (direction_x, direction_y, gradient_magnitude) =
                    normalized_direction(gradient.x(), gradient.y());
                let intensity =
                    (gradient_magnitude.min(1.0) * (0.45 + energy * 0.55)).clamp(0.0, 1.0);

                values[index] = FlowSample {
                    direction_x,
                    direction_y,
                    intensity,
                    convergence: 0.0,
                };
            }
        }

        for y in 0..dimensions.height() {
            for x in 0..dimensions.width() {
                let index = y * dimensions.width() + x;
                let divergence = directional_divergence(&values, dimensions, x, y);
                values[index].convergence = (-divergence * 0.75).clamp(-1.0, 1.0);
            }
        }

        Ok(Self { dimensions, values })
    }

    pub fn dimensions(&self) -> super::FieldDimensions {
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

    pub fn as_slice(&self) -> &[FlowSample] {
        &self.values
    }

    pub fn read(&self, x: usize, y: usize) -> Option<FlowSample> {
        if x >= self.width() || y >= self.height() {
            return None;
        }
        Some(self.values[y * self.width() + x])
    }
}

fn normalized_direction(x: f32, y: f32) -> (f32, f32, f32) {
    let magnitude = x.hypot(y);
    if magnitude <= f32::EPSILON || !magnitude.is_finite() {
        return (0.0, 0.0, 0.0);
    }

    (x / magnitude, y / magnitude, magnitude)
}

fn directional_divergence(
    values: &[FlowSample],
    dimensions: super::FieldDimensions,
    x: usize,
    y: usize,
) -> f32 {
    let width = dimensions.width();
    let height = dimensions.height();
    if width <= 1 && height <= 1 {
        return 0.0;
    }

    let sample =
        |sample_x: usize, sample_y: usize| -> FlowSample { values[sample_y * width + sample_x] };
    let center = sample(x, y);
    let horizontal = axis_difference(x, width, center.direction_x, |coordinate| {
        sample(coordinate, y).direction_x
    });
    let vertical = axis_difference(y, height, center.direction_y, |coordinate| {
        sample(x, coordinate).direction_y
    });

    horizontal + vertical
}

fn axis_difference<F>(coordinate: usize, length: usize, center: f32, sample: F) -> f32
where
    F: Fn(usize) -> f32,
{
    if length <= 1 {
        return 0.0;
    }

    if coordinate == 0 {
        sample(1) - center
    } else if coordinate + 1 == length {
        center - sample(coordinate - 1)
    } else {
        (sample(coordinate + 1) - sample(coordinate - 1)) * 0.5
    }
}
