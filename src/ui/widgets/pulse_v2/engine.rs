use super::{FieldDimensions, FieldError, NormalizedPulseInput, PulseV2Preset, ScalarField};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct PulseV2Engine;

#[derive(Debug, Clone, PartialEq)]
pub struct PulseV2Frame {
    dimensions: FieldDimensions,
    preset: PulseV2Preset,
    input: NormalizedPulseInput,
    field: ScalarField,
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

        Ok(PulseV2Frame {
            dimensions,
            preset,
            input,
            field,
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

    pub fn is_empty(&self) -> bool {
        self.field.as_slice().iter().all(|value| *value == 0.0)
    }
}
