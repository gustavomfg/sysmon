use std::time::Duration;

use super::{
    FieldDimensions, FieldError, NoiseField, NormalizedPulseInput, PulseV2Preset, ScalarField,
    noise,
};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct PulseV2Engine;

#[derive(Debug, Clone, PartialEq)]
pub struct PulseV2Frame {
    dimensions: FieldDimensions,
    preset: PulseV2Preset,
    input: NormalizedPulseInput,
    field: ScalarField,
    noise: NoiseField,
    elapsed: Duration,
    reduced_motion: bool,
}

impl PulseV2Engine {
    pub fn new() -> Self {
        Self
    }

    pub fn empty_frame(
        &self,
        dimensions: FieldDimensions,
        input: NormalizedPulseInput,
    ) -> Result<PulseV2Frame, FieldError> {
        let preset = PulseV2Preset::from_dimensions(dimensions);
        let field = ScalarField::try_new(dimensions)?;
        let noise = NoiseField::try_new(dimensions)?;

        Ok(PulseV2Frame {
            dimensions,
            preset,
            input,
            field,
            noise,
            elapsed: Duration::ZERO,
            reduced_motion: true,
        })
    }

    pub fn generate(
        &self,
        dimensions: FieldDimensions,
        input: NormalizedPulseInput,
        elapsed: Duration,
        reduced_motion: bool,
    ) -> Result<PulseV2Frame, FieldError> {
        let preset = PulseV2Preset::from_dimensions(dimensions);
        let complexity = preset.parameters().complexity();
        let mut field = ScalarField::try_new(dimensions)?;
        let time = if reduced_motion {
            0.0
        } else {
            elapsed.as_secs_f32()
        };

        for y in 0..dimensions.height() {
            for x in 0..dimensions.width() {
                let position = normalized_position(x, y, dimensions);
                let energy = energy_at(position, input, time, dimensions, complexity);
                field.write(x, y, energy)?;
            }
        }

        let noise = noise::generate(dimensions, input, preset, elapsed, reduced_motion)?;

        Ok(PulseV2Frame {
            dimensions,
            preset,
            input,
            field,
            noise,
            elapsed,
            reduced_motion,
        })
    }
}

impl PulseV2Frame {
    pub fn dimensions(&self) -> FieldDimensions {
        self.dimensions
    }

    pub fn preset(&self) -> PulseV2Preset {
        self.preset
    }

    pub fn input(&self) -> NormalizedPulseInput {
        self.input
    }

    pub fn field(&self) -> &ScalarField {
        &self.field
    }

    pub fn noise(&self) -> &NoiseField {
        &self.noise
    }

    pub fn is_empty(&self) -> bool {
        self.field.as_slice().iter().all(|value| *value == 0.0)
            && self.noise.as_slice().iter().all(|value| *value == 0.0)
    }

    pub fn elapsed(&self) -> Duration {
        self.elapsed
    }

    pub fn reduced_motion(&self) -> bool {
        self.reduced_motion
    }
}

fn normalized_position(x: usize, y: usize, dimensions: FieldDimensions) -> (f32, f32) {
    let width = dimensions.width().saturating_sub(1).max(1) as f32;
    let height = dimensions.height().saturating_sub(1).max(1) as f32;
    (x as f32 / width, y as f32 / height)
}

fn energy_at(
    position: (f32, f32),
    input: NormalizedPulseInput,
    time: f32,
    dimensions: FieldDimensions,
    complexity: u8,
) -> f32 {
    let (x, y) = position;
    let aspect = dimensions.width().max(1) as f32 / dimensions.height().max(1) as f32;
    let activity = (0.10
        + input.cpu() * 0.46
        + input.memory() * 0.18
        + input.load() * 0.16
        + input.network() * 0.07
        + input.gpu().unwrap_or(0.0) * 0.03
        + input.volatility() * 0.08)
        .clamp(0.0, 1.0);
    let pressure =
        (input.memory() * 0.42 + input.load() * 0.30 + input.cpu() * 0.18 + input.network() * 0.10)
            .clamp(0.0, 1.0);
    let richness = (0.30 + input.gpu().unwrap_or(0.0) * 0.45 + input.cpu() * 0.25).clamp(0.0, 1.0);
    let deformation = input.volatility() * 0.028;
    let phase = time * (0.11 + input.volatility() * 0.16);
    let warped_x = x + (phase * 0.73 + y * 2.1).sin() * deformation;
    let aspect_scale = (1.0 / aspect.sqrt()).clamp(0.65, 1.45);
    let warped_y =
        0.5 + (y + (phase * 0.61 + x * 1.7).cos() * deformation * 0.72 - 0.5) * aspect_scale;

    let dominant = anisotropic_influence(
        warped_x,
        warped_y,
        AnisotropicShape {
            center: (
                0.585 + input.network() * 0.035,
                0.455 - input.load() * 0.018,
            ),
            size: (0.20 + pressure * 0.05, 0.115 - pressure * 0.015),
            rotation: 0.30,
            skew: 0.18,
        },
    );
    let upstream = anisotropic_influence(
        warped_x,
        warped_y,
        AnisotropicShape {
            center: (
                0.31 + input.network() * 0.045,
                0.38 + input.volatility() * 0.025,
            ),
            size: (0.26, 0.16),
            rotation: -0.42,
            skew: 0.12,
        },
    );
    let secondary = anisotropic_influence(
        warped_x,
        warped_y,
        AnisotropicShape {
            center: (
                0.735 + input.network() * 0.025,
                0.675 - input.volatility() * 0.035,
            ),
            size: (0.14 + richness * 0.025, 0.095 + pressure * 0.015),
            rotation: 0.58,
            skew: 0.08,
        },
    );

    let broad_gradient = ((warped_x - 0.38) * (0.22 + input.network() * 0.34)
        + (0.56 - warped_y) * (0.10 + pressure * 0.20))
        .max(0.0);
    let broad = dominant * (0.48 + pressure * 0.20)
        + upstream * (0.20 + activity * 0.08)
        + secondary * (0.10 + richness * 0.15)
        + broad_gradient * 0.10;

    let complexity_scale = f32::from(complexity);
    let medium_phase = phase * 0.74 + warped_x * (2.35 + complexity_scale * 0.22)
        - warped_y * (1.65 + complexity_scale * 0.16);
    let medium_warp =
        (medium_phase.sin() * 0.5 + 0.5) * (0.62 + 0.18 * (warped_y * 3.1 + phase * 0.35).cos());
    let medium = (dominant * medium_warp * (0.30 + activity * 0.24)
        + upstream * (0.18 + richness * 0.12)
        + secondary * (0.12 + input.volatility() * 0.16))
        .max(0.0);

    let micro_phase = (warped_x * (8.4 + complexity_scale * 0.55)
        + warped_y * (5.5 + complexity_scale * 0.30)
        + phase * 0.42)
        .sin()
        * (warped_x * (3.7 + complexity_scale * 0.22) - warped_y * (6.7 + complexity_scale * 0.30)
            + phase * 0.27)
            .cos();
    let micro = (micro_phase * 0.5 + 0.5)
        * (0.012 + complexity_scale * 0.002 + richness * 0.035 + input.volatility() * 0.028);

    let pressure_concentration = dominant.powf(1.35) * (0.18 + pressure * 0.32);
    let raw = broad * (0.42 + activity * 0.38)
        + medium * (0.24 + richness * 0.18)
        + pressure_concentration
        + micro;
    let activation_threshold = (0.12 - activity * 0.03).clamp(0.075, 0.12);
    let activated =
        ((raw - activation_threshold).max(0.0) / (0.40 + pressure * 0.10)).clamp(0.0, 1.0);
    activated.powf(1.24) * (0.72 + activity * 0.42)
}

#[derive(Debug, Clone, Copy)]
struct AnisotropicShape {
    center: (f32, f32),
    size: (f32, f32),
    rotation: f32,
    skew: f32,
}

fn anisotropic_influence(x: f32, y: f32, shape: AnisotropicShape) -> f32 {
    let dx = x - shape.center.0;
    let dy = y - shape.center.1;
    let sin_rotation = shape.rotation.sin();
    let cos_rotation = shape.rotation.cos();
    let rotated_x = dx * cos_rotation - dy * sin_rotation;
    let rotated_y = dx * sin_rotation + dy * cos_rotation;
    let skewed_y = rotated_y - rotated_x * shape.skew;
    let distance = (rotated_x / shape.size.0.max(0.001)).powi(2)
        + (skewed_y / shape.size.1.max(0.001)).powi(2);
    (-distance * 0.72).exp()
}
