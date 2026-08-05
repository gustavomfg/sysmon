use super::types::MAX_FIELD_SAMPLES;
use super::{FieldError, FlowField, NormalizedPulseInput, PulseV2Preset, StructuredField};

const ACTIVE_EPSILON: f32 = 0.02;

/// Perceptual lighting data without color or terminal-specific state.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct LightingSample {
    intensity: f32,
    depth: f32,
    relative_energy: f32,
    local_highlight: f32,
    peak: bool,
}

impl LightingSample {
    pub fn intensity(self) -> f32 {
        self.intensity
    }

    pub fn depth(self) -> f32 {
        self.depth
    }

    pub fn relative_energy(self) -> f32 {
        self.relative_energy
    }

    pub fn local_highlight(self) -> f32 {
        self.local_highlight
    }

    pub fn is_peak(self) -> bool {
        self.peak
    }
}

/// A bounded perceptual representation derived from the organized field.
#[derive(Debug, Clone, PartialEq)]
pub struct LightingField {
    dimensions: super::FieldDimensions,
    values: Vec<LightingSample>,
}

impl LightingField {
    pub fn try_new(dimensions: super::FieldDimensions) -> Result<Self, FieldError> {
        if dimensions.sample_count() > MAX_FIELD_SAMPLES {
            return Err(FieldError::Dimensions(
                super::DimensionsError::SampleCountTooLarge,
            ));
        }

        Ok(Self {
            dimensions,
            values: vec![LightingSample::default(); dimensions.sample_count()],
        })
    }

    pub fn from_fields(
        structured: &StructuredField,
        flow: &FlowField,
        input: NormalizedPulseInput,
        preset: PulseV2Preset,
        _reduced_motion: bool,
    ) -> Result<Self, FieldError> {
        let dimensions = structured.dimensions();
        if flow.dimensions() != dimensions {
            return Err(FieldError::DimensionsMismatch);
        }

        let mut lighting = Self::try_new(dimensions)?;
        let active_values = structured
            .as_slice()
            .iter()
            .copied()
            .filter(|value| *value > ACTIVE_EPSILON)
            .collect::<Vec<_>>();
        if active_values.is_empty() {
            return Ok(lighting);
        }

        let mut sorted_active = active_values;
        sorted_active.sort_by(f32::total_cmp);
        let distribution = ActiveDistribution::from_sorted(&sorted_active);
        let parameters = preset.parameters();
        let radius = usize::from(parameters.lighting_radius());
        let exposure_gain = 0.94
            + input.cpu() * 0.07
            + input.memory() * 0.06
            + input.load() * 0.05
            + input.gpu().unwrap_or(0.0) * 0.03;
        let contrast_gain = parameters.lighting_contrast_strength() * (0.88 + input.load() * 0.12);

        for y in 0..dimensions.height() {
            for x in 0..dimensions.width() {
                let index = y * dimensions.width() + x;
                let structured_value = structured.read(x, y).ok_or(FieldError::OutOfBounds)?;
                if structured_value <= ACTIVE_EPSILON {
                    continue;
                }

                let relative_energy = distribution.relative(structured_value);
                let neighborhood = structured_neighborhood_mean(structured, x, y, radius);
                let local_highlight = local_highlight(structured_value, neighborhood);
                let flow_sample = flow.read(x, y).ok_or(FieldError::OutOfBounds)?;
                let convergence = flow_sample.convergence().max(0.0);
                let depth = (0.10
                    + relative_energy * 0.52
                    + convergence * 0.22
                    + flow_sample.intensity() * (0.10 + input.gpu().unwrap_or(0.0) * 0.06))
                    .clamp(0.0, 1.0);
                let base_intensity = perceptual_intensity(relative_energy, distribution);
                let intensity = (base_intensity * exposure_gain
                    + local_highlight * contrast_gain * 0.15
                    + convergence * (0.08 + input.load() * 0.06))
                    .clamp(0.0, 1.0);
                let peak_score =
                    relative_energy * 0.56 + local_highlight * 0.24 + convergence * 0.20;
                let peak = structured_value >= distribution.peak_floor
                    && local_highlight > 0.03
                    && convergence > 0.02
                    && depth > 0.55
                    && peak_score >= parameters.lighting_peak_cutoff() - input.volatility() * 0.035;
                let peak_intensity = if peak {
                    (intensity + 0.045 + input.volatility() * 0.035).min(1.0)
                } else {
                    intensity
                };

                lighting.values[index] = LightingSample {
                    intensity: peak_intensity,
                    depth,
                    relative_energy,
                    local_highlight,
                    peak,
                };
            }
        }

        Ok(lighting)
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

    pub fn as_slice(&self) -> &[LightingSample] {
        &self.values
    }

    pub fn read(&self, x: usize, y: usize) -> Option<LightingSample> {
        if x >= self.width() || y >= self.height() {
            return None;
        }
        Some(self.values[y * self.width() + x])
    }
}

#[derive(Debug, Clone, Copy)]
struct ActiveDistribution {
    lower: f32,
    midpoint: f32,
    upper: f32,
    peak_floor: f32,
}

impl ActiveDistribution {
    fn from_sorted(values: &[f32]) -> Self {
        Self {
            lower: percentile(values, 0.10),
            midpoint: percentile(values, 0.50),
            upper: percentile(values, 0.90),
            peak_floor: percentile(values, 0.97),
        }
    }

    fn relative(self, value: f32) -> f32 {
        let span = (self.peak_floor - self.lower).max(f32::EPSILON);
        ((value - self.lower) / span).clamp(0.0, 1.0)
    }
}

fn percentile(values: &[f32], position: f32) -> f32 {
    if values.is_empty() {
        return 0.0;
    }
    let position = position.clamp(0.0, 1.0) * (values.len() - 1) as f32;
    let lower = position.floor() as usize;
    let upper = position.ceil() as usize;
    let fraction = position - lower as f32;
    values[lower] + (values[upper] - values[lower]) * fraction
}

fn perceptual_intensity(relative: f32, distribution: ActiveDistribution) -> f32 {
    let span = (distribution.peak_floor - distribution.lower).max(f32::EPSILON);
    let midpoint = ((distribution.midpoint - distribution.lower) / span).clamp(0.12, 0.88);
    let upper = ((distribution.upper - distribution.lower) / span).clamp(midpoint + 0.05, 0.96);
    if relative <= midpoint {
        let lower_rank = (relative / midpoint.max(f32::EPSILON)).clamp(0.0, 1.0);
        0.12 + lower_rank.powf(0.72) * 0.46
    } else if relative <= upper {
        let middle_rank =
            ((relative - midpoint) / (upper - midpoint).max(f32::EPSILON)).clamp(0.0, 1.0);
        0.58 + middle_rank.powf(0.92) * 0.25
    } else {
        let peak_rank = ((relative - upper) / (1.0 - upper).max(f32::EPSILON)).clamp(0.0, 1.0);
        0.83 + peak_rank.powf(1.18) * 0.09
    }
}

fn structured_neighborhood_mean(field: &StructuredField, x: usize, y: usize, radius: usize) -> f32 {
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

fn local_highlight(value: f32, neighborhood: f32) -> f32 {
    ((value - neighborhood) / (0.10 + neighborhood * 0.80)).clamp(0.0, 1.0)
}
