use std::time::Duration;

use super::types::{DimensionsError, MAX_FIELD_SAMPLES, PresetParameters};
use super::{FieldDimensions, FieldError, NormalizedPulseInput, PulseV2Preset};

/// A terminal-independent, normalized texture buffer.
///
/// Unlike [`super::ScalarField`], this buffer represents deterministic texture
/// rather than energy. Values are always finite and live in `0.0..=1.0`.
#[derive(Debug, Clone, PartialEq)]
pub struct NoiseField {
    dimensions: FieldDimensions,
    values: Vec<f32>,
}

impl NoiseField {
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

    fn write(&mut self, x: usize, y: usize, value: f32) -> Result<(), FieldError> {
        if !value.is_finite() {
            return Err(FieldError::NonFiniteValue);
        }
        if !(0.0..=1.0).contains(&value) {
            return Err(FieldError::ValueOutOfRange);
        }
        if x >= self.width() || y >= self.height() {
            return Err(FieldError::OutOfBounds);
        }

        let width = self.width();
        self.values[y * width + x] = value;
        Ok(())
    }
}

const NOISE_SEED: u32 = 0x51F1_7EAD;

pub(crate) fn generate(
    dimensions: FieldDimensions,
    input: NormalizedPulseInput,
    preset: PulseV2Preset,
    elapsed: Duration,
    reduced_motion: bool,
) -> Result<NoiseField, FieldError> {
    let parameters = preset.parameters();
    let mut field = NoiseField::try_new(dimensions)?;
    let time = if reduced_motion {
        0.0
    } else {
        elapsed.as_secs_f32()
            * parameters.noise_time_speed()
            * (0.82 + input.volatility() * 0.42 + input.cpu() * 0.08)
    };

    let aspect = dimensions.width().max(1) as f32 / dimensions.height().max(1) as f32;
    let vertical_scale = aspect.sqrt().clamp(0.72, 1.42);
    let network_drift = (input.network() - 0.5) * 0.12;
    let pressure_warp = input.memory() * 0.035 + input.load() * 0.025;

    for cell_y in 0..dimensions.height() {
        for cell_x in 0..dimensions.width() {
            let (normalized_x, normalized_y) = normalized_position(cell_x, cell_y, dimensions);
            let sample_x = normalized_x + time * (0.10 + input.network() * 0.04) + network_drift;
            let sample_y = normalized_y * vertical_scale
                + time * (0.055 + input.volatility() * 0.025)
                + pressure_warp * (normalized_x - 0.5);
            let value = sample_multiscale(sample_x, sample_y, time, input, parameters);
            field.write(cell_x, cell_y, value)?;
        }
    }

    Ok(field)
}

fn normalized_position(x: usize, y: usize, dimensions: FieldDimensions) -> (f32, f32) {
    let width = dimensions.width().saturating_sub(1).max(1) as f32;
    let height = dimensions.height().saturating_sub(1).max(1) as f32;
    (x as f32 / width, y as f32 / height)
}

fn sample_multiscale(
    x: f32,
    y: f32,
    time: f32,
    input: NormalizedPulseInput,
    parameters: PresetParameters,
) -> f32 {
    let base = parameters.noise_base_frequency();
    let directional_time = time * (0.25 + input.network() * 0.10);
    let low = value_noise(
        x * base * 0.62 + directional_time,
        y * base * 0.62 - directional_time * 0.46,
        NOISE_SEED ^ 0x10,
    );
    let medium = value_noise(
        x * base * 1.55 + directional_time * 0.72,
        y * base * 1.55 + directional_time * 0.31,
        NOISE_SEED ^ 0x20,
    );
    let high = value_noise(
        x * base * 3.75 - directional_time * 0.48,
        y * base * 3.75 + directional_time * 0.83,
        NOISE_SEED ^ 0x30,
    );

    let mut value = low * 0.29 + medium * 0.54 + high * 0.17;
    if parameters.noise_octaves() >= 4 {
        let extra = value_noise(
            x * base * 6.8 + directional_time * 0.36,
            y * base * 6.8 - directional_time * 0.57,
            NOISE_SEED ^ 0x40,
        );
        value += (extra - 0.5) * parameters.noise_micro_amplitude();
    }
    if parameters.noise_octaves() >= 5 {
        let fine = value_noise(
            x * base * 11.4 - directional_time * 0.21,
            y * base * 11.4 + directional_time * 0.43,
            NOISE_SEED ^ 0x50,
        );
        value += (fine - 0.5) * parameters.noise_micro_amplitude() * 0.45;
    }

    let contrast = 0.90 + input.cpu() * 0.08 + input.volatility() * 0.06;
    (0.5 + (value - 0.5) * contrast).clamp(0.0, 1.0)
}

fn value_noise(x: f32, y: f32, seed: u32) -> f32 {
    let x0 = x.floor() as i32;
    let y0 = y.floor() as i32;
    let tx = smoothstep(x - x0 as f32);
    let ty = smoothstep(y - y0 as f32);
    let top = lerp(
        lattice_value(x0, y0, seed),
        lattice_value(x0 + 1, y0, seed),
        tx,
    );
    let bottom = lerp(
        lattice_value(x0, y0 + 1, seed),
        lattice_value(x0 + 1, y0 + 1, seed),
        tx,
    );
    lerp(top, bottom, ty).clamp(0.0, 1.0)
}

fn lattice_value(x: i32, y: i32, seed: u32) -> f32 {
    let mut value =
        seed ^ (x as u32).wrapping_mul(0x9E37_79B9) ^ (y as u32).wrapping_mul(0x85EB_CA6B);
    value ^= value >> 16;
    value = value.wrapping_mul(0x7FEB_352D);
    value ^= value >> 15;
    value = value.wrapping_mul(0x846C_A68B);
    value ^= value >> 16;
    value as f32 / u32::MAX as f32
}

fn smoothstep(value: f32) -> f32 {
    let value = value.clamp(0.0, 1.0);
    value * value * value * (value * (value * 6.0 - 15.0) + 10.0)
}

fn lerp(start: f32, end: f32, amount: f32) -> f32 {
    start + (end - start) * amount
}
