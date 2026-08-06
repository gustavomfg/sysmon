use std::time::Duration;

use super::behavior::interpolate_input;
use super::motion::OrganicMotion;
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
        let mut field = ScalarField::try_new(dimensions)?;
        let motion = OrganicMotion::new(dimensions, input, elapsed, reduced_motion);
        motion.deposit_into(&mut field, preset)?;

        let noise = noise::generate(dimensions, input, preset, elapsed, reduced_motion)?;
        let flow = FlowField::from_scalar(&field)?;
        let transported =
            TransportedField::from_organic_motion(&field, &noise, &flow, elapsed, reduced_motion)?;
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
