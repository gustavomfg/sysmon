use crate::pulse::PulseInput;

pub const MAX_FIELD_DIMENSION: usize = 4_096;
pub const MAX_FIELD_SAMPLES: usize = 1_048_576;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FieldDimensions {
    width: usize,
    height: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DimensionsError {
    SampleCountOverflow,
    DimensionTooLarge,
    SampleCountTooLarge,
}

impl FieldDimensions {
    pub fn new(width: usize, height: usize) -> Result<Self, DimensionsError> {
        let sample_count = width
            .checked_mul(height)
            .ok_or(DimensionsError::SampleCountOverflow)?;

        if width > MAX_FIELD_DIMENSION || height > MAX_FIELD_DIMENSION {
            return Err(DimensionsError::DimensionTooLarge);
        }
        if sample_count > MAX_FIELD_SAMPLES {
            return Err(DimensionsError::SampleCountTooLarge);
        }

        Ok(Self { width, height })
    }

    pub fn width(self) -> usize {
        self.width
    }

    pub fn height(self) -> usize {
        self.height
    }

    pub fn sample_count(self) -> usize {
        self.width * self.height
    }

    pub fn is_empty(self) -> bool {
        self.width == 0 || self.height == 0
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct NormalizedPoint {
    x: f32,
    y: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CoordinateError {
    NonFinite,
    OutOfRange,
}

impl NormalizedPoint {
    pub fn try_new(x: f32, y: f32) -> Result<Self, CoordinateError> {
        if !x.is_finite() || !y.is_finite() {
            return Err(CoordinateError::NonFinite);
        }
        if !(0.0..=1.0).contains(&x) || !(0.0..=1.0).contains(&y) {
            return Err(CoordinateError::OutOfRange);
        }

        Ok(Self { x, y })
    }

    pub fn x(self) -> f32 {
        self.x
    }

    pub fn y(self) -> f32 {
        self.y
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct NormalizedPulseInput {
    cpu: f32,
    memory: f32,
    gpu: Option<f32>,
    network: f32,
    load: f32,
    volatility: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputError {
    NonFinite,
    OutOfRange,
}

impl NormalizedPulseInput {
    pub fn try_new(
        cpu: f32,
        memory: f32,
        gpu: Option<f32>,
        network: f32,
        load: f32,
        volatility: f32,
    ) -> Result<Self, InputError> {
        for value in [cpu, memory, network, load, volatility] {
            validate_normalized(value)?;
        }
        if let Some(value) = gpu {
            validate_normalized(value)?;
        }

        Ok(Self {
            cpu,
            memory,
            gpu,
            network,
            load,
            volatility,
        })
    }

    pub fn cpu(self) -> f32 {
        self.cpu
    }

    pub fn memory(self) -> f32 {
        self.memory
    }

    pub fn gpu(self) -> Option<f32> {
        self.gpu
    }

    pub fn network(self) -> f32 {
        self.network
    }

    pub fn load(self) -> f32 {
        self.load
    }

    pub fn volatility(self) -> f32 {
        self.volatility
    }
}

impl Default for NormalizedPulseInput {
    fn default() -> Self {
        Self {
            cpu: 0.0,
            memory: 0.0,
            gpu: None,
            network: 0.0,
            load: 0.0,
            volatility: 0.0,
        }
    }
}

impl From<PulseInput> for NormalizedPulseInput {
    fn from(input: PulseInput) -> Self {
        Self {
            cpu: sanitize_normalized(input.cpu),
            memory: sanitize_normalized(input.memory),
            gpu: input.gpu.map(sanitize_normalized),
            network: sanitize_normalized(input.network),
            load: sanitize_normalized(input.load),
            volatility: sanitize_normalized(input.volatility),
        }
    }
}

fn validate_normalized(value: f32) -> Result<(), InputError> {
    if !value.is_finite() {
        return Err(InputError::NonFinite);
    }
    if !(0.0..=1.0).contains(&value) {
        return Err(InputError::OutOfRange);
    }
    Ok(())
}

fn sanitize_normalized(value: f32) -> f32 {
    if value.is_finite() {
        value.clamp(0.0, 1.0)
    } else {
        0.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PulseV2Preset {
    Compact,
    Standard,
    Widescreen,
    LargeWidescreen,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PresetParameters {
    sample_limit: usize,
    complexity: u8,
    noise_base_frequency: f32,
    noise_octaves: u8,
    noise_micro_amplitude: f32,
    noise_time_speed: f32,
    transport_steps: u8,
    transport_radius: f32,
    transport_strength: f32,
    structure_radius: u8,
    structure_contrast_strength: f32,
    structure_convergence_strength: f32,
    lighting_radius: u8,
    lighting_contrast_strength: f32,
    lighting_peak_cutoff: f32,
}

impl PulseV2Preset {
    pub fn from_dimensions(dimensions: FieldDimensions) -> Self {
        let width = dimensions.width();
        let height = dimensions.height();

        if width <= 36 || height <= 9 {
            Self::Compact
        } else if width >= 120 && height >= 24 {
            Self::LargeWidescreen
        } else if width >= 70 && height >= 14 {
            Self::Widescreen
        } else {
            Self::Standard
        }
    }

    pub fn parameters(self) -> PresetParameters {
        let (
            sample_limit,
            complexity,
            noise_base_frequency,
            noise_octaves,
            noise_micro_amplitude,
            noise_time_speed,
            transport_steps,
            transport_radius,
            transport_strength,
            structure_radius,
            structure_contrast_strength,
            structure_convergence_strength,
            lighting_radius,
            lighting_contrast_strength,
            lighting_peak_cutoff,
        ) = match self {
            Self::Compact => (
                16_384, 1, 1.20, 3, 0.065, 0.075, 2, 0.032, 0.72, 1, 0.14, 0.15, 1, 0.16, 0.78,
            ),
            Self::Standard => (
                65_536, 2, 1.40, 3, 0.080, 0.090, 3, 0.046, 0.78, 1, 0.18, 0.19, 1, 0.19, 0.74,
            ),
            Self::Widescreen => (
                262_144, 3, 1.58, 4, 0.095, 0.105, 4, 0.060, 0.82, 1, 0.22, 0.23, 1, 0.22, 0.72,
            ),
            Self::LargeWidescreen => (
                MAX_FIELD_SAMPLES,
                4,
                1.76,
                5,
                0.110,
                0.120,
                5,
                0.074,
                0.85,
                2,
                0.25,
                0.27,
                2,
                0.24,
                0.70,
            ),
        };

        PresetParameters {
            sample_limit,
            complexity,
            noise_base_frequency,
            noise_octaves,
            noise_micro_amplitude,
            noise_time_speed,
            transport_steps,
            transport_radius,
            transport_strength,
            structure_radius,
            structure_contrast_strength,
            structure_convergence_strength,
            lighting_radius,
            lighting_contrast_strength,
            lighting_peak_cutoff,
        }
    }
}

impl PresetParameters {
    pub fn sample_limit(self) -> usize {
        self.sample_limit
    }

    pub fn complexity(self) -> u8 {
        self.complexity
    }

    pub(crate) fn noise_base_frequency(self) -> f32 {
        self.noise_base_frequency
    }

    pub(crate) fn noise_octaves(self) -> u8 {
        self.noise_octaves
    }

    pub(crate) fn noise_micro_amplitude(self) -> f32 {
        self.noise_micro_amplitude
    }

    pub(crate) fn noise_time_speed(self) -> f32 {
        self.noise_time_speed
    }

    pub(crate) fn transport_steps(self) -> u8 {
        self.transport_steps
    }

    pub(crate) fn transport_radius(self) -> f32 {
        self.transport_radius
    }

    pub(crate) fn transport_strength(self) -> f32 {
        self.transport_strength
    }

    pub(crate) fn structure_radius(self) -> u8 {
        self.structure_radius
    }

    pub(crate) fn structure_contrast_strength(self) -> f32 {
        self.structure_contrast_strength
    }

    pub(crate) fn structure_convergence_strength(self) -> f32 {
        self.structure_convergence_strength
    }

    pub(crate) fn lighting_radius(self) -> u8 {
        self.lighting_radius
    }

    pub(crate) fn lighting_contrast_strength(self) -> f32 {
        self.lighting_contrast_strength
    }

    pub(crate) fn lighting_peak_cutoff(self) -> f32 {
        self.lighting_peak_cutoff
    }
}
