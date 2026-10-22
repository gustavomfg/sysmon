use super::types::{DimensionsError, MAX_FIELD_SAMPLES};
use super::{FieldError, FlowField, PulseV2Preset, ScalarField, TransportedField};

/// A scalar organization of energy ready for a future lighting stage.
///
/// The field contains no terminal or visual primitives. Its structures emerge
/// from energy, transported texture, local contrast and flow convergence.
#[derive(Debug, Clone, PartialEq)]
pub struct StructuredField {
    dimensions: super::FieldDimensions,
    values: Vec<f32>,
}

impl StructuredField {
    pub fn try_new(dimensions: super::FieldDimensions) -> Result<Self, FieldError> {
        if dimensions.sample_count() > MAX_FIELD_SAMPLES {
            return Err(FieldError::Dimensions(DimensionsError::SampleCountTooLarge));
        }

        Ok(Self {
            dimensions,
            values: vec![0.0; dimensions.sample_count()],
        })
    }

    pub fn from_fields(
        scalar: &ScalarField,
        transported: &TransportedField,
        flow: &FlowField,
        preset: PulseV2Preset,
    ) -> Result<Self, FieldError> {
        let dimensions = scalar.dimensions();
        if transported.dimensions() != dimensions || flow.dimensions() != dimensions {
            return Err(FieldError::DimensionsMismatch);
        }

        let parameters = preset.parameters();
        let mut structured = Self::try_new(dimensions)?;
        let radius = usize::from(parameters.structure_radius());

        for y in 0..dimensions.height() {
            for x in 0..dimensions.width() {
                let index = y * dimensions.width() + x;
                let energy = scalar.read(x, y).ok_or(FieldError::OutOfBounds)?;
                let transported_value = transported.read(x, y).ok_or(FieldError::OutOfBounds)?;
                let local_flow = flow.read(x, y).ok_or(FieldError::OutOfBounds)?;
                let neighborhood = transported_neighborhood_mean(transported, x, y, radius);
                let contrast = local_contrast(transported_value, neighborhood);
                let coherence = flow_coherence(flow, x, y);
                let convergent = local_flow.convergence().max(0.0);
                let divergent = (-local_flow.convergence()).max(0.0);

                let organized_energy = energy * 0.58
                    + transported_value * (0.04 + energy * 0.34)
                    + energy * transported_value * 0.18;
                let contrast_lift =
                    contrast * parameters.structure_contrast_strength() * (0.24 + energy * 0.76);
                let convergence_lift = convergent
                    * parameters.structure_convergence_strength()
                    * (0.34 + energy * 0.66)
                    * (0.45 + transported_value * 0.55);
                let divergence_loss = divergent * 0.055 * (0.35 + energy * 0.65);
                let coherence_factor = 0.82 + coherence * 0.18;

                structured.values[index] =
                    (organized_energy * coherence_factor + contrast_lift + convergence_lift
                        - divergence_loss)
                        .clamp(0.0, 1.0);
            }
        }

        Ok(structured)
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

fn transported_neighborhood_mean(
    field: &TransportedField,
    x: usize,
    y: usize,
    radius: usize,
) -> f32 {
    let min_x = x.saturating_sub(radius);
    let max_x = x
        .saturating_add(radius)
        .min(field.width().saturating_sub(1));
    let min_y = y.saturating_sub(radius);
    let max_y = y
        .saturating_add(radius)
        .min(field.height().saturating_sub(1));
    let mut total = 0.0;
    let mut count = 0;

    for sample_y in min_y..=max_y {
        for sample_x in min_x..=max_x {
            total += field.read(sample_x, sample_y).unwrap_or(0.0);
            count += 1;
        }
    }

    total / count.max(1) as f32
}

fn local_contrast(value: f32, neighborhood: f32) -> f32 {
    let denominator = 0.12 + neighborhood * 0.72;
    ((value - neighborhood) / denominator).clamp(0.0, 1.0)
}

fn flow_coherence(flow: &FlowField, x: usize, y: usize) -> f32 {
    let center = flow.read(x, y).unwrap_or_default();
    let center_magnitude = center.direction_magnitude();
    if center_magnitude <= f32::EPSILON {
        return 0.0;
    }

    let mut agreement = 0.0;
    let mut count = 0;
    for (sample_x, sample_y) in neighboring_coordinates(flow, x, y) {
        let neighbor = flow.read(sample_x, sample_y).unwrap_or_default();
        if neighbor.direction_magnitude() > f32::EPSILON {
            let dot = center.direction_x() * neighbor.direction_x()
                + center.direction_y() * neighbor.direction_y();
            agreement += dot.abs().clamp(0.0, 1.0);
            count += 1;
        }
    }

    if count == 0 {
        0.0
    } else {
        agreement / count as f32
    }
}

fn neighboring_coordinates(
    field: &FlowField,
    x: usize,
    y: usize,
) -> impl Iterator<Item = (usize, usize)> {
    [
        (x.saturating_sub(1), y),
        ((x + 1).min(field.width().saturating_sub(1)), y),
        (x, y.saturating_sub(1)),
        (x, (y + 1).min(field.height().saturating_sub(1))),
    ]
    .into_iter()
    .filter(move |coordinate| *coordinate != (x, y))
}
