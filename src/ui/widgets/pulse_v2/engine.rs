use std::time::Duration;

use super::behavior::{FieldDynamics, interpolate_input};
use super::{
    FieldDimensions, FieldError, FlowField, LightingField, NoiseField, NormalizedPulseInput,
    PulseV2InputTransition, PulseV2Preset, RefinedEnergyField, ScalarField, StructuredField,
    TransportedField, noise,
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
    flow: FlowField,
    transported: TransportedField,
    structured: StructuredField,
    refined: RefinedEnergyField,
    lighting: LightingField,
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
        let flow = FlowField::from_scalar(&field)?;
        let transported = TransportedField::try_new(dimensions)?;
        let structured = StructuredField::try_new(dimensions)?;
        let refined = RefinedEnergyField::try_new(dimensions)?;
        let lighting = LightingField::try_new(dimensions)?;

        Ok(PulseV2Frame {
            dimensions,
            preset,
            input,
            field,
            noise,
            flow,
            transported,
            structured,
            refined,
            lighting,
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
        self.generate_with_preset(
            dimensions,
            input,
            PulseV2Preset::from_dimensions(dimensions),
            elapsed,
            reduced_motion,
        )
    }

    pub fn generate_with_preset(
        &self,
        dimensions: FieldDimensions,
        input: NormalizedPulseInput,
        preset: PulseV2Preset,
        elapsed: Duration,
        reduced_motion: bool,
    ) -> Result<PulseV2Frame, FieldError> {
        self.generate_internal(dimensions, input, preset, elapsed, reduced_motion)
    }

    /// Generates a frame while easing from a previously displayed input to a
    /// new target. The normal generation API remains stateless and unchanged;
    /// this variant lets a caller preserve visual inertia across snapshots or
    /// scenario changes without putting mutable animation state in the field
    /// pipeline.
    pub fn generate_with_preset_transition(
        &self,
        dimensions: FieldDimensions,
        transition: PulseV2InputTransition,
        preset: PulseV2Preset,
        elapsed: Duration,
        reduced_motion: bool,
    ) -> Result<PulseV2Frame, FieldError> {
        let effective_input = if reduced_motion {
            transition.target()
        } else {
            interpolate_input(
                transition.previous(),
                transition.target(),
                transition.progress(),
            )
        };
        self.generate_internal(dimensions, effective_input, preset, elapsed, reduced_motion)
    }

    fn generate_internal(
        &self,
        dimensions: FieldDimensions,
        input: NormalizedPulseInput,
        preset: PulseV2Preset,
        elapsed: Duration,
        reduced_motion: bool,
    ) -> Result<PulseV2Frame, FieldError> {
        let complexity = preset.parameters().complexity();
        let mut field = ScalarField::try_new(dimensions)?;
        let time = if reduced_motion {
            0.0
        } else {
            elapsed.as_secs_f32()
        };
        let dynamics = FieldDynamics::new(input, elapsed, reduced_motion);

        for y in 0..dimensions.height() {
            for x in 0..dimensions.width() {
                let position = normalized_position(x, y, dimensions);
                let energy = energy_at(position, input, time, dimensions, complexity, dynamics);
                field.write(x, y, energy)?;
            }
        }

        let noise = noise::generate(dimensions, input, preset, elapsed, reduced_motion)?;
        let flow = FlowField::from_scalar(&field)?;
        let transported = TransportedField::from_fields_at_time(
            &field,
            &noise,
            &flow,
            preset,
            elapsed,
            reduced_motion,
        )?;
        let structured = StructuredField::from_fields(&field, &transported, &flow, preset)?;
        let refined = RefinedEnergyField::from_fields(
            &structured,
            &transported,
            &flow,
            preset,
            reduced_motion,
        )?;
        let lighting = LightingField::from_refined(&refined, &flow, input, preset, reduced_motion)?;

        Ok(PulseV2Frame {
            dimensions,
            preset,
            input,
            field,
            noise,
            flow,
            transported,
            structured,
            refined,
            lighting,
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

    pub fn flow(&self) -> &FlowField {
        &self.flow
    }

    pub fn transported(&self) -> &TransportedField {
        &self.transported
    }

    pub fn structured(&self) -> &StructuredField {
        &self.structured
    }

    pub fn refined(&self) -> &RefinedEnergyField {
        &self.refined
    }

    pub fn lighting(&self) -> &LightingField {
        &self.lighting
    }

    pub fn is_empty(&self) -> bool {
        self.field.as_slice().iter().all(|value| *value == 0.0)
            && self.noise.as_slice().iter().all(|value| *value == 0.0)
            && self.flow.as_slice().iter().all(|sample| {
                sample.direction_x() == 0.0
                    && sample.direction_y() == 0.0
                    && sample.intensity() == 0.0
                    && sample.convergence() == 0.0
            })
            && self
                .transported
                .as_slice()
                .iter()
                .all(|value| *value == 0.0)
            && self.structured.as_slice().iter().all(|value| *value == 0.0)
            && self.refined.as_slice().iter().all(|sample| {
                sample.energy() == 0.0
                    && sample.thickness() == 0.0
                    && sample.branching() == 0.0
                    && sample.island() == 0.0
                    && sample.dissipation() == 0.0
            })
            && self.lighting.as_slice().iter().all(|sample| {
                sample.intensity() == 0.0
                    && sample.depth() == 0.0
                    && sample.relative_energy() == 0.0
                    && sample.local_highlight() == 0.0
                    && !sample.is_peak()
            })
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
    dynamics: FieldDynamics,
) -> f32 {
    let (x, y) = position;
    let warp = dynamics.landscape_warp(x, y);
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
    let phase = time * (0.11 + input.volatility() * 0.16) * (0.82 + dynamics.intensity() * 0.22);
    let warped_x = x + (phase * 0.73 + y * 2.1).sin() * deformation + warp.0;
    let aspect_scale = (1.0 / aspect.sqrt()).clamp(0.65, 1.45);
    let warped_y = 0.5
        + (y + (phase * 0.61 + x * 1.7).cos() * deformation * 0.72 + warp.1 - 0.5) * aspect_scale;

    let dominant_offset = dynamics.dominant_offset();
    let dominant_size = dynamics.dominant_size_scale();
    let upstream_offset = dynamics.source_offset(0);
    let secondary_offset = dynamics.source_offset(1);
    let dominant = anisotropic_influence(
        warped_x,
        warped_y,
        AnisotropicShape {
            center: (
                0.585 + input.network() * 0.035 + dominant_offset.0,
                0.455 - input.load() * 0.018 + dominant_offset.1,
            ),
            size: (
                (0.20 + pressure * 0.05) * dominant_size.0,
                (0.115 - pressure * 0.015) * dominant_size.1,
            ),
            rotation: 0.30 + dynamics.reorganization() * 0.035,
            skew: 0.18 + dynamics.reorganization() * 0.012,
        },
    );
    let upstream = anisotropic_influence(
        warped_x,
        warped_y,
        AnisotropicShape {
            center: (
                0.31 + input.network() * 0.045 + upstream_offset.0,
                0.38 + input.volatility() * 0.025 + upstream_offset.1,
            ),
            size: (0.26 * (1.0 + dynamics.breath() * 0.025), 0.16),
            rotation: -0.42 + dynamics.reorganization() * 0.022,
            skew: 0.12,
        },
    );
    let secondary = anisotropic_influence(
        warped_x,
        warped_y,
        AnisotropicShape {
            center: (
                0.735 + input.network() * 0.025 + secondary_offset.0,
                0.675 - input.volatility() * 0.035 + secondary_offset.1,
            ),
            size: (
                (0.14 + richness * 0.025) * (0.98 + dynamics.pulse() * 0.04),
                (0.095 + pressure * 0.015) * (0.96 + dynamics.breath() * 0.06),
            ),
            rotation: 0.58 + dynamics.reorganization() * 0.028,
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
    let ridges = implicit_ridge_energy((warped_x, warped_y), input, dynamics);
    let raw = broad * (0.42 + activity * 0.38)
        + medium * (0.24 + richness * 0.18)
        + pressure_concentration
        + ridges * (0.08 + activity * 0.10)
        + micro;
    let activation_threshold = (0.12 - activity * 0.03).clamp(0.065, 0.12);
    let activated =
        ((raw - activation_threshold).max(0.0) / (0.40 + pressure * 0.10)).clamp(0.0, 1.0);
    (activated.powf(1.24) * (0.72 + activity * 0.42)).clamp(0.0, 1.0)
}

/// Returns the potential of the V1-like moving current pattern without
/// creating paths or particles. Each current is an implicit energy ridge:
/// separate at the boundaries, compressed into the dominant mass, and
/// released into a reorganized exit field. The scalar result is still the
/// only input to the V2 FlowField.
fn implicit_ridge_energy(
    position: (f32, f32),
    input: NormalizedPulseInput,
    dynamics: FieldDynamics,
) -> f32 {
    const LANES: [f32; 6] = [-0.28, -0.17, -0.08, 0.07, 0.16, 0.27];

    let (x, y) = position;
    let dominant_offset = dynamics.dominant_offset();
    let core_x = (0.585 + input.network() * 0.035 + dominant_offset.0).clamp(0.38, 0.72);
    let core_y = (0.455 - input.load() * 0.018 + dominant_offset.1).clamp(0.28, 0.70);
    let approach = ((x - 0.025) / (core_x - 0.025).max(0.001)).clamp(0.0, 1.0);
    let exit = ((x - core_x) / (1.025 - core_x).max(0.001)).clamp(0.0, 1.0);
    let compression = smoothstep(approach);
    let release = smoothstep(exit);
    let pressure = dynamics.pressure();
    let turbulence = 0.010 + input.volatility() * 0.012 + pressure * 0.006;
    let mut energy = 0.0;

    for (index, lane) in LANES.into_iter().enumerate() {
        let lane_phase = dynamics.ridge_phase(lane);
        let curvature = (x * (3.0 + index as f32 * 0.31) + lane_phase).sin() * turbulence;
        let entry_offset = lane * (1.0 - compression * 0.94);
        let exit_offset = lane * (0.12 + release * 0.80);
        let offset = if x <= core_x {
            entry_offset
        } else {
            exit_offset
        };
        let center = core_y + offset + curvature;
        let width = (0.0105 + compression * 0.0045 + (1.0 - release) * 0.0025 + pressure * 0.0025)
            .clamp(0.008, 0.024);
        let distance = (y - center) / width;
        let ridge = (-distance * distance * 0.72).exp();
        let support = (-distance * distance * 0.18).exp() * 0.075;
        let lane_weight = 0.76 + (index as f32 * 0.037) + input.cpu() * 0.08;
        energy += (ridge + support) * lane_weight;
    }

    // Overlap in the compression region forms a short energetic bridge rather
    // than a point. It is still bounded by the existing source field.
    let mass = (-(((x - core_x) / 0.075).powi(2) + ((y - core_y) / 0.055).powi(2)) * 0.72).exp();
    let line_signal = (energy / 2.2).clamp(0.0, 1.0);
    (line_signal * (0.76 + compression * 0.12 + release * 0.06) + mass * (0.12 + pressure * 0.08))
        .clamp(0.0, 1.0)
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

fn smoothstep(value: f32) -> f32 {
    let value = value.clamp(0.0, 1.0);
    value * value * (3.0 - 2.0 * value)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn input() -> NormalizedPulseInput {
        NormalizedPulseInput::try_new(0.42, 0.48, Some(0.30), 0.18, 0.35, 0.08).unwrap()
    }

    fn ridge_peak(x: f32, input: NormalizedPulseInput, dynamics: FieldDynamics) -> f32 {
        (0..=256)
            .map(|index| implicit_ridge_energy((x, index as f32 / 256.0), input, dynamics))
            .fold(0.0, f32::max)
    }

    #[test]
    fn moving_currents_keep_separate_entry_mass_and_reorganized_exit() {
        let input = input();
        let dynamics = FieldDynamics::new(input, Duration::from_secs_f32(2.0), false);
        let entry = ridge_peak(0.04, input, dynamics);
        let mass = ridge_peak(0.59, input, dynamics);
        let exit = ridge_peak(0.94, input, dynamics);

        assert!(entry > 0.10);
        assert!(mass > entry * 0.95);
        assert!(exit > 0.10);
        assert!(mass > exit * 0.95);
        assert_ne!(
            ridge_peak(
                0.30,
                input,
                FieldDynamics::new(input, Duration::ZERO, false)
            ),
            ridge_peak(
                0.30,
                input,
                FieldDynamics::new(input, Duration::from_secs(3), false)
            )
        );
    }
}
