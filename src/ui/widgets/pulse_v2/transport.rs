use std::time::Duration;

use super::types::{DimensionsError, MAX_FIELD_SAMPLES};
use super::{FieldError, FlowField, NoiseField, PulseV2Preset, ScalarField};

/// The short, flow-guided organization of the deterministic noise texture.
///
/// This is still a scalar, terminal-independent buffer. It does not contain
/// particles or visual primitives, and the original `NoiseField` remains
/// unchanged in the frame.
#[derive(Debug, Clone, PartialEq)]
pub struct TransportedField {
    dimensions: super::FieldDimensions,
    values: Vec<f32>,
}

impl TransportedField {
    pub fn try_new(dimensions: super::FieldDimensions) -> Result<Self, FieldError> {
        if dimensions.sample_count() > MAX_FIELD_SAMPLES {
            return Err(FieldError::Dimensions(DimensionsError::SampleCountTooLarge));
        }

        Ok(Self {
            dimensions,
            values: vec![0.0; dimensions.sample_count()],
        })
    }

    /// Applies a bounded, symmetric directional convolution around each
    /// sample. The local FlowField direction determines the integration axis;
    /// flat regions retain their source value instead of inventing motion.
    pub fn from_fields(
        scalar: &ScalarField,
        noise: &NoiseField,
        flow: &FlowField,
        preset: PulseV2Preset,
        reduced_motion: bool,
    ) -> Result<Self, FieldError> {
        Self::from_fields_at_time(scalar, noise, flow, preset, Duration::ZERO, reduced_motion)
    }

    /// Applies the same local convolution as [`Self::from_fields`], then
    /// continuously advects its sampling origin along the existing flow.
    /// This is the field-level equivalent of V1's moving particles: no
    /// trajectory is stored and no new structure is drawn.
    pub fn from_fields_at_time(
        scalar: &ScalarField,
        noise: &NoiseField,
        flow: &FlowField,
        preset: PulseV2Preset,
        elapsed: Duration,
        reduced_motion: bool,
    ) -> Result<Self, FieldError> {
        let dimensions = scalar.dimensions();
        if noise.dimensions() != dimensions || flow.dimensions() != dimensions {
            return Err(FieldError::DimensionsMismatch);
        }

        let parameters = preset.parameters();
        let mut transported = Self::try_new(dimensions)?;
        let steps = if reduced_motion {
            parameters.transport_steps().min(1)
        } else {
            parameters.transport_steps()
        };
        let elapsed_seconds = if reduced_motion {
            0.0
        } else {
            elapsed.as_secs_f32().min(1_000_000.0)
        };

        for y in 0..dimensions.height() {
            for x in 0..dimensions.width() {
                let index = y * dimensions.width() + x;
                let source = noise.read(x, y).ok_or(FieldError::OutOfBounds)?;
                let energy = scalar.read(x, y).ok_or(FieldError::OutOfBounds)?;
                let local_flow = flow.read(x, y).ok_or(FieldError::OutOfBounds)?;
                let direction_magnitude = local_flow.direction_magnitude();

                if steps == 0 || direction_magnitude <= f32::EPSILON {
                    transported.values[index] = source;
                    continue;
                }

                let (normalized_x, normalized_y) = normalized_position(x, y, dimensions);
                let radius = parameters.transport_radius() * (0.72 + local_flow.intensity() * 0.28);
                let travel_speed = 0.012 + energy * 0.020 + local_flow.intensity() * 0.012;
                let travel = elapsed_seconds * travel_speed;
                let transported_origin = (
                    normalized_x - local_flow.direction_x() * travel,
                    normalized_y - local_flow.direction_y() * travel,
                );
                let advected_source =
                    sample_normalized(noise, transported_origin.0, transported_origin.1);
                let mut weighted_sum = source * 0.20 + advected_source * 0.28;
                let mut total_weight = 0.48;
                for step in 1..=steps {
                    let fraction = step as f32 / steps as f32;
                    let distance = radius * fraction;
                    let weight = 1.0 - fraction * 0.46;
                    let offset_x = local_flow.direction_x() * distance;
                    let offset_y = local_flow.direction_y() * distance;
                    let forward = sample_normalized(
                        noise,
                        transported_origin.0 + offset_x,
                        transported_origin.1 + offset_y,
                    );
                    let backward = sample_normalized(
                        noise,
                        transported_origin.0 - offset_x,
                        transported_origin.1 - offset_y,
                    );
                    weighted_sum += (forward + backward) * weight;
                    total_weight += weight * 2.0;
                }

                let guided = weighted_sum / total_weight;
                let strength = parameters.transport_strength()
                    * (0.42 + energy * 0.58)
                    * if reduced_motion { 0.35 } else { 1.0 };
                transported.values[index] = lerp(source, guided, strength).clamp(0.0, 1.0);
            }
        }

        Ok(transported)
    }

    /// Lightweight transport used by the organic-motion path. The motion
    /// source has already performed the expensive V1-style advection, so the
    /// texture only needs one local flow-guided sample here. The full bounded
    /// convolution above remains available as the standalone transport API.
    pub(crate) fn from_organic_motion(
        scalar: &ScalarField,
        noise: &NoiseField,
        flow: &FlowField,
        elapsed: Duration,
        reduced_motion: bool,
    ) -> Result<Self, FieldError> {
        let dimensions = scalar.dimensions();
        if noise.dimensions() != dimensions || flow.dimensions() != dimensions {
            return Err(FieldError::DimensionsMismatch);
        }

        let mut transported = Self::try_new(dimensions)?;
        let time = if reduced_motion {
            0.0
        } else {
            elapsed.as_secs_f32().min(1_000_000.0)
        };
        for y in 0..dimensions.height() {
            for x in 0..dimensions.width() {
                let index = y * dimensions.width() + x;
                let source = noise.read(x, y).ok_or(FieldError::OutOfBounds)?;
                let energy = scalar.read(x, y).ok_or(FieldError::OutOfBounds)?;
                let local_flow = flow.read(x, y).ok_or(FieldError::OutOfBounds)?;
                if local_flow.direction_magnitude() <= f32::EPSILON {
                    transported.values[index] = source;
                    continue;
                }

                let position = normalized_position(x, y, dimensions);
                let travel = time * (0.004 + energy * 0.008);
                let origin = (
                    position.0 - local_flow.direction_x() * travel,
                    position.1 - local_flow.direction_y() * travel,
                );
                let forward = sample_normalized(
                    noise,
                    origin.0 + local_flow.direction_x() * 0.012,
                    origin.1 + local_flow.direction_y() * 0.012,
                );
                let backward = sample_normalized(
                    noise,
                    origin.0 - local_flow.direction_x() * 0.012,
                    origin.1 - local_flow.direction_y() * 0.012,
                );
                let guided = sample_normalized(noise, origin.0, origin.1) * 0.46
                    + (forward + backward) * 0.27;
                let strength = (0.28 + energy * 0.38) * if reduced_motion { 0.35 } else { 1.0 };
                transported.values[index] = lerp(source, guided, strength).clamp(0.0, 1.0);
            }
        }
        Ok(transported)
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

    pub fn as_slice(&self) -> &[f32] {
        &self.values
    }

    pub fn read(&self, x: usize, y: usize) -> Option<f32> {
        if x >= self.width() || y >= self.height() {
            return None;
        }
        Some(self.values[y * self.width() + x])
    }
}

fn normalized_position(x: usize, y: usize, dimensions: super::FieldDimensions) -> (f32, f32) {
    let width = dimensions.width().saturating_sub(1).max(1) as f32;
    let height = dimensions.height().saturating_sub(1).max(1) as f32;
    (x as f32 / width, y as f32 / height)
}

fn sample_normalized(field: &NoiseField, x: f32, y: f32) -> f32 {
    let max_x = field.width().saturating_sub(1);
    let max_y = field.height().saturating_sub(1);
    if max_x == 0 && max_y == 0 {
        return field.read(0, 0).unwrap_or(0.0);
    }

    let position_x = x.clamp(0.0, 1.0) * max_x as f32;
    let position_y = y.clamp(0.0, 1.0) * max_y as f32;
    let x0 = position_x.floor() as usize;
    let y0 = position_y.floor() as usize;
    let x1 = (x0 + 1).min(max_x);
    let y1 = (y0 + 1).min(max_y);
    let tx = position_x - x0 as f32;
    let ty = position_y - y0 as f32;
    let top = lerp(
        field.read(x0, y0).unwrap_or(0.0),
        field.read(x1, y0).unwrap_or(0.0),
        tx,
    );
    let bottom = lerp(
        field.read(x0, y1).unwrap_or(0.0),
        field.read(x1, y1).unwrap_or(0.0),
        tx,
    );
    lerp(top, bottom, ty).clamp(0.0, 1.0)
}

fn lerp(start: f32, end: f32, amount: f32) -> f32 {
    start + (end - start) * amount
}
