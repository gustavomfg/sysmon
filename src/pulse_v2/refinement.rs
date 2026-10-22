use super::types::{DimensionsError, MAX_FIELD_SAMPLES};
use super::{FieldError, FlowField, PulseV2Preset, StructuredField, TransportedField};

const ACTIVE_EPSILON: f32 = 0.02;

/// Structural information added after the approved organization stage.
///
/// Every channel is derived from already existing energy and flow. The
/// channels describe how the existing structure can be refined later without
/// introducing terminal primitives or independent trajectories.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct RefinedEnergySample {
    energy: f32,
    thickness: f32,
    branching: f32,
    island: f32,
    dissipation: f32,
}

impl RefinedEnergySample {
    pub fn energy(self) -> f32 {
        self.energy
    }

    pub fn thickness(self) -> f32 {
        self.thickness
    }

    pub fn branching(self) -> f32 {
        self.branching
    }

    pub fn island(self) -> f32 {
        self.island
    }

    pub fn dissipation(self) -> f32 {
        self.dissipation
    }
}

/// A bounded multi-scale structural refinement of [`StructuredField`].
///
/// The field never creates energy outside the active organization already
/// present in the source. It is intentionally separate from the approved
/// Scalar, Noise, Flow, Transported, Structured and Lighting buffers.
#[derive(Debug, Clone, PartialEq)]
pub struct RefinedEnergyField {
    dimensions: super::FieldDimensions,
    values: Vec<RefinedEnergySample>,
}

impl RefinedEnergyField {
    pub fn try_new(dimensions: super::FieldDimensions) -> Result<Self, FieldError> {
        if dimensions.sample_count() > MAX_FIELD_SAMPLES {
            return Err(FieldError::Dimensions(DimensionsError::SampleCountTooLarge));
        }

        Ok(Self {
            dimensions,
            values: vec![RefinedEnergySample::default(); dimensions.sample_count()],
        })
    }

    /// Refines existing organization using bounded macro, medium and micro
    /// neighborhoods. No feature is placed at a predetermined coordinate.
    pub fn from_fields(
        structured: &StructuredField,
        transported: &TransportedField,
        flow: &FlowField,
        preset: PulseV2Preset,
        reduced_motion: bool,
    ) -> Result<Self, FieldError> {
        let dimensions = structured.dimensions();
        if transported.dimensions() != dimensions || flow.dimensions() != dimensions {
            return Err(FieldError::DimensionsMismatch);
        }

        let (macro_radius, medium_radius) = refinement_radii(preset);
        let detail_scale = if reduced_motion { 0.72 } else { 1.0 };
        // Turbulence is a local reorganization pass, not a new source of
        // energy. Reduced motion keeps the approved composition intact while
        // removing almost all of the secondary motion inside active regions.
        let turbulence_scale = if reduced_motion { 0.12 } else { 1.0 };
        let mut refined = Self::try_new(dimensions)?;

        for y in 0..dimensions.height() {
            for x in 0..dimensions.width() {
                let index = y * dimensions.width() + x;
                let base = structured.read(x, y).ok_or(FieldError::OutOfBounds)?;
                if base <= ACTIVE_EPSILON {
                    continue;
                }

                let transported_value = transported.read(x, y).ok_or(FieldError::OutOfBounds)?;
                let macro_mean = structured_ring_mean(structured, x, y, macro_radius);
                let medium_mean = structured_ring_mean(structured, x, y, medium_radius);
                let texture_mean = transported_ring_mean(transported, x, y, 1);
                let flow_sample = flow.read(x, y).ok_or(FieldError::OutOfBounds)?;
                let coherence = (flow_sample.intensity() * 0.62
                    + (1.0 - flow_sample.convergence().abs()) * 0.38)
                    .clamp(0.0, 1.0);
                let convergence = flow_sample.convergence().max(0.0);
                let divergence = (-flow_sample.convergence()).max(0.0);
                let gate = smoothstep((base - ACTIVE_EPSILON) / 0.22);

                // Positive differences expose ridges at different spatial
                // scales. Negative differences are retained as dissipation,
                // not turned into artificial dark holes.
                let macro_ridge = (base - macro_mean).max(0.0);
                let medium_ridge = (base - medium_mean).max(0.0);
                let local_texture = (transported_value - texture_mean).abs();
                let local_dissipation = (macro_mean - base).max(0.0) * gate;

                // A side-aware sample exposes short offshoots only where the
                // existing flow and nearby energy already support them.
                let lateral_energy = lateral_transport_mean(transported, x, y, flow_sample);
                let (lateral_structured, lateral_asymmetry) =
                    lateral_structured_support(structured, x, y, flow_sample);
                let lateral_density = (lateral_structured * 0.68 + lateral_energy * 0.32)
                    * gate
                    * (0.82 + coherence * 0.18);
                let mass_score = (base * 0.42
                    + macro_mean * 0.18
                    + medium_mean * 0.16
                    + lateral_density * 0.16
                    + convergence * 0.08)
                    .clamp(0.0, 1.0);
                let mass_factor =
                    smoothstep((mass_score - 0.28) / 0.28) * (0.84 + coherence * 0.16) * gate;
                let mass_irregularity = (lateral_asymmetry * 0.48
                    + local_texture * 0.30
                    + turbulence_hint(base, macro_mean, medium_mean) * 0.22)
                    .clamp(0.0, 1.0);
                let density_support = lateral_density
                    * (0.045 + mass_factor * 0.12)
                    * (0.82 + (1.0 - lateral_asymmetry) * 0.18);
                let lateral_signal = (lateral_energy - texture_mean).max(0.0);
                let turbulence = coherent_turbulence(
                    transported_value,
                    texture_mean,
                    lateral_energy,
                    convergence,
                    coherence,
                    gate,
                    turbulence_scale,
                );
                let signed_variation = coherent_variation(
                    transported_value,
                    texture_mean,
                    lateral_energy,
                    macro_mean,
                    medium_mean,
                ) * gate
                    * turbulence_scale;

                // These are short, flow-aligned reconnections: they only
                // amplify side support that already exists in the transport
                // buffer. The result changes local organization without
                // opening a new corridor in a silent region.
                let micro_reconnection = turbulence
                    * (lateral_signal * 0.24 + local_texture * 0.14 + convergence * 0.10);
                let branching = (lateral_signal * (0.48 + (1.0 - coherence) * 0.27)
                    + local_texture * 0.22
                    + convergence * 0.10)
                    * gate
                    * detail_scale
                    + micro_reconnection * detail_scale;

                // Islands are local energy pockets, while reinforcement is
                // allowed to grow only from an already active corridor.
                let island = (medium_ridge * 0.58
                    + macro_ridge * 0.20
                    + local_texture * 0.22
                    + turbulence * 0.08
                    + density_support * 0.22)
                    * gate
                    * (0.56 + convergence * 0.24 + coherence * 0.20)
                    * detail_scale;
                let reinforcement = macro_ridge * 0.22
                    + medium_ridge * 0.48
                    + local_texture * 0.18
                    + branching * 0.12
                    + density_support * 0.30
                    + mass_factor * (0.035 + mass_irregularity * 0.018)
                    + convergence * 0.08;
                let dissipation = (local_dissipation * 0.72
                    + divergence * base * 0.18
                    + turbulence * (0.012 + divergence * 0.018))
                    * detail_scale;

                // Thickness varies with organization and local support rather
                // than with a fixed corridor width. The main energy remains
                // dominant so global composition is not rebuilt. The signed
                // term creates slow breathing and contraction instead of a
                // uniformly thicker corridor.
                let thickness = (gate
                    * (0.44
                        + base * 0.20
                        + coherence * 0.18
                        + convergence * 0.10
                        + medium_ridge * 0.22
                        - dissipation * 0.12)
                    + signed_variation * (0.16 + turbulence * 0.06)
                    + turbulence * 0.018)
                    .clamp(0.0, 1.0);
                let refined_energy = (base
                    + gate
                        * detail_scale
                        * (reinforcement * 0.095 + island * 0.035 + branching * 0.025)
                    - dissipation * 0.025
                    + signed_variation * (0.022 + convergence * 0.012)
                    + density_support * 0.035
                    + mass_factor * (0.006 + base * 0.012) * (0.90 + mass_irregularity * 0.10))
                    .clamp(0.0, 1.0);

                refined.values[index] = RefinedEnergySample {
                    energy: refined_energy,
                    thickness,
                    branching: branching.clamp(0.0, 1.0),
                    island: island.clamp(0.0, 1.0),
                    dissipation: dissipation.clamp(0.0, 1.0),
                };
            }
        }

        Ok(refined)
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

    pub fn as_slice(&self) -> &[RefinedEnergySample] {
        &self.values
    }

    pub fn read(&self, x: usize, y: usize) -> Option<RefinedEnergySample> {
        if x >= self.width() || y >= self.height() {
            return None;
        }
        Some(self.values[y * self.width() + x])
    }
}

fn refinement_radii(preset: PulseV2Preset) -> (usize, usize) {
    match preset {
        PulseV2Preset::Compact => (2, 1),
        PulseV2Preset::Standard => (3, 1),
        PulseV2Preset::Widescreen => (4, 2),
        PulseV2Preset::LargeWidescreen => (5, 2),
    }
}

fn structured_ring_mean(field: &StructuredField, x: usize, y: usize, radius: usize) -> f32 {
    let samples = [
        (x.saturating_sub(radius), y),
        (
            x.saturating_add(radius)
                .min(field.width().saturating_sub(1)),
            y,
        ),
        (x, y.saturating_sub(radius)),
        (
            x,
            y.saturating_add(radius)
                .min(field.height().saturating_sub(1)),
        ),
    ];
    samples
        .into_iter()
        .map(|(sample_x, sample_y)| field.read(sample_x, sample_y).unwrap_or(0.0))
        .sum::<f32>()
        / samples.len() as f32
}

fn transported_ring_mean(field: &TransportedField, x: usize, y: usize, radius: usize) -> f32 {
    let samples = [
        (x.saturating_sub(radius), y),
        (
            x.saturating_add(radius)
                .min(field.width().saturating_sub(1)),
            y,
        ),
        (x, y.saturating_sub(radius)),
        (
            x,
            y.saturating_add(radius)
                .min(field.height().saturating_sub(1)),
        ),
    ];
    samples
        .into_iter()
        .map(|(sample_x, sample_y)| field.read(sample_x, sample_y).unwrap_or(0.0))
        .sum::<f32>()
        / samples.len() as f32
}

fn lateral_transport_mean(
    field: &TransportedField,
    x: usize,
    y: usize,
    flow: super::FlowSample,
) -> f32 {
    let ((first_x, first_y), (second_x, second_y)) =
        lateral_coordinates(field.width(), field.height(), x, y, flow);

    (field.read(first_x, first_y).unwrap_or(0.0) + field.read(second_x, second_y).unwrap_or(0.0))
        * 0.5
}

fn lateral_structured_support(
    field: &StructuredField,
    x: usize,
    y: usize,
    flow: super::FlowSample,
) -> (f32, f32) {
    let ((first_x, first_y), (second_x, second_y)) =
        lateral_coordinates(field.width(), field.height(), x, y, flow);
    let first = field.read(first_x, first_y).unwrap_or(0.0);
    let second = field.read(second_x, second_y).unwrap_or(0.0);
    ((first + second) * 0.5, (first - second).abs())
}

fn lateral_coordinates(
    width: usize,
    height: usize,
    x: usize,
    y: usize,
    flow: super::FlowSample,
) -> ((usize, usize), (usize, usize)) {
    let tangent_x = -flow.direction_y();
    let tangent_y = flow.direction_x();
    if tangent_x.abs() >= tangent_y.abs() {
        (
            (x.saturating_sub(1), y),
            (x.saturating_add(1).min(width.saturating_sub(1)), y),
        )
    } else {
        (
            (x, y.saturating_sub(1)),
            (x, y.saturating_add(1).min(height.saturating_sub(1))),
        )
    }
}

fn turbulence_hint(base: f32, macro_mean: f32, medium_mean: f32) -> f32 {
    (base - medium_mean).abs() * 0.58 + (medium_mean - macro_mean).abs() * 0.42
}

/// Measures smooth, already-existing shear around a sample. The side-aware
/// transport sample is important here: it makes the modulation follow the
/// local flow instead of introducing a screen-aligned pattern.
fn coherent_turbulence(
    transported_value: f32,
    texture_mean: f32,
    lateral_energy: f32,
    convergence: f32,
    coherence: f32,
    gate: f32,
    scale: f32,
) -> f32 {
    let lateral_shear = (lateral_energy - transported_value).abs();
    let texture_shear = (transported_value - texture_mean).abs();
    let compression = convergence * 0.65 + (1.0 - coherence) * 0.35;
    (lateral_shear * 0.50 + texture_shear * 0.28 + compression * 0.22).clamp(0.0, 1.0)
        * gate
        * scale
}

/// A signed low-amplitude variation used for local contraction and recovery.
/// Its inputs are neighborhood differences, so it is continuous across the
/// existing corridor and cannot create an independent structure in silence.
fn coherent_variation(
    transported_value: f32,
    texture_mean: f32,
    lateral_energy: f32,
    macro_mean: f32,
    medium_mean: f32,
) -> f32 {
    ((lateral_energy - transported_value) * 0.56
        + (transported_value - texture_mean) * 0.28
        + (medium_mean - macro_mean) * 0.16)
        .clamp(-1.0, 1.0)
}

fn smoothstep(value: f32) -> f32 {
    let value = value.clamp(0.0, 1.0);
    value * value * (3.0 - 2.0 * value)
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::super::NoiseField;
    use super::*;

    fn input() -> super::super::NormalizedPulseInput {
        super::super::NormalizedPulseInput::try_new(0.42, 0.48, Some(0.30), 0.18, 0.35, 0.08)
            .unwrap()
    }

    fn generated(seconds: f32, reduced_motion: bool) -> super::super::PulseV2Frame {
        super::super::PulseV2Engine::new()
            .generate_with_preset(
                super::super::FieldDimensions::new(96, 48).unwrap(),
                input(),
                PulseV2Preset::Widescreen,
                Duration::from_secs_f32(seconds),
                reduced_motion,
            )
            .unwrap()
    }

    fn weighted_centroid(values: &[f32], width: usize, height: usize) -> (f32, f32) {
        let mut total = 0.0;
        let mut x_total = 0.0;
        let mut y_total = 0.0;
        for (index, value) in values.iter().copied().enumerate() {
            total += value;
            x_total += (index % width) as f32 * value;
            y_total += (index / width) as f32 * value;
        }
        if total <= f32::EPSILON {
            return (0.0, 0.0);
        }
        (
            x_total / total / width.max(1) as f32,
            y_total / total / height.max(1) as f32,
        )
    }

    fn mean_energy_delta(
        first: &super::super::PulseV2Frame,
        second: &super::super::PulseV2Frame,
    ) -> f32 {
        first
            .refined()
            .as_slice()
            .iter()
            .zip(second.refined().as_slice())
            .map(|(left, right)| (left.energy() - right.energy()).abs())
            .sum::<f32>()
            / first.refined().as_slice().len().max(1) as f32
    }

    fn high_energy_bounds(
        field: &RefinedEnergyField,
    ) -> Option<(usize, usize, usize, usize, usize)> {
        let maximum = field
            .as_slice()
            .iter()
            .map(|sample| sample.energy())
            .fold(0.0, f32::max);
        let threshold = maximum * 0.72;
        let mut min_x = field.width();
        let mut min_y = field.height();
        let mut max_x = 0;
        let mut max_y = 0;
        let mut count = 0;

        for (index, sample) in field.as_slice().iter().enumerate() {
            if sample.energy() < threshold {
                continue;
            }
            let x = index % field.width().max(1);
            let y = index / field.width().max(1);
            min_x = min_x.min(x);
            min_y = min_y.min(y);
            max_x = max_x.max(x);
            max_y = max_y.max(y);
            count += 1;
        }

        (count > 0).then_some((min_x, min_y, max_x, max_y, count))
    }

    fn largest_connected_component(field: &RefinedEnergyField, threshold: f32) -> usize {
        let mut visited = vec![false; field.as_slice().len()];
        let mut largest = 0;
        for index in 0..field.as_slice().len() {
            if visited[index] || field.as_slice()[index].energy() < threshold {
                continue;
            }
            let mut stack = vec![index];
            visited[index] = true;
            let mut size = 0;
            while let Some(current) = stack.pop() {
                size += 1;
                let x = current % field.width().max(1);
                let y = current / field.width().max(1);
                for (neighbor_x, neighbor_y) in [
                    (x.saturating_sub(1), y),
                    (x.saturating_add(1).min(field.width().saturating_sub(1)), y),
                    (x, y.saturating_sub(1)),
                    (x, y.saturating_add(1).min(field.height().saturating_sub(1))),
                ] {
                    let neighbor = neighbor_y * field.width() + neighbor_x;
                    if !visited[neighbor] && field.as_slice()[neighbor].energy() >= threshold {
                        visited[neighbor] = true;
                        stack.push(neighbor);
                    }
                }
            }
            largest = largest.max(size);
        }
        largest
    }

    fn source(
        seconds: f32,
        reduced_motion: bool,
    ) -> (StructuredField, TransportedField, FlowField, PulseV2Preset) {
        let dimensions = super::super::FieldDimensions::new(96, 48).unwrap();
        let scalar = super::super::ScalarField::try_new(dimensions).unwrap();
        let noise = NoiseField::try_new(dimensions).unwrap();
        let flow = FlowField::from_scalar(&scalar).unwrap();
        let transported = TransportedField::from_fields(
            &scalar,
            &noise,
            &flow,
            PulseV2Preset::Widescreen,
            reduced_motion,
        )
        .unwrap();
        let structured =
            StructuredField::from_fields(&scalar, &transported, &flow, PulseV2Preset::Widescreen)
                .unwrap();
        let _ = (seconds, input());
        (structured, transported, flow, PulseV2Preset::Widescreen)
    }

    #[test]
    fn refinement_is_bounded_and_deterministic() {
        let (structured, transported, flow, preset) = source(0.0, false);
        let first =
            RefinedEnergyField::from_fields(&structured, &transported, &flow, preset, false)
                .unwrap();
        let same = RefinedEnergyField::from_fields(&structured, &transported, &flow, preset, false)
            .unwrap();

        assert_eq!(first, same);
        assert!(first.as_slice().iter().all(|sample| {
            [
                sample.energy(),
                sample.thickness(),
                sample.branching(),
                sample.island(),
                sample.dissipation(),
            ]
            .into_iter()
            .all(|value| value.is_finite() && (0.0..=1.0).contains(&value))
        }));
    }

    #[test]
    fn refinement_preserves_silence_and_adds_local_diversity() {
        let dimensions = super::super::FieldDimensions::new(17, 9).unwrap();
        let mut scalar = super::super::ScalarField::try_new(dimensions).unwrap();
        let mut noise = NoiseField::try_new(dimensions).unwrap();
        scalar.fill(0.0).unwrap();
        noise.fill(0.2).unwrap();
        for y in 2..7 {
            for x in 3..14 {
                scalar.write(x, y, 0.46).unwrap();
            }
        }
        noise.write(8, 4, 1.0).unwrap();
        let flow = FlowField::from_scalar(&scalar).unwrap();
        let transported =
            TransportedField::from_fields(&scalar, &noise, &flow, PulseV2Preset::Standard, false)
                .unwrap();
        let structured =
            StructuredField::from_fields(&scalar, &transported, &flow, PulseV2Preset::Standard)
                .unwrap();
        let refined = RefinedEnergyField::from_fields(
            &structured,
            &transported,
            &flow,
            PulseV2Preset::Standard,
            false,
        )
        .unwrap();

        assert!(refined.read(0, 0).unwrap().energy() == 0.0);
        assert!(
            refined
                .as_slice()
                .iter()
                .any(|sample| sample.energy() > 0.0)
        );
        let minimum_thickness = refined
            .as_slice()
            .iter()
            .map(|sample| sample.thickness())
            .fold(f32::INFINITY, f32::min);
        let maximum_thickness = refined
            .as_slice()
            .iter()
            .map(|sample| sample.thickness())
            .fold(f32::NEG_INFINITY, f32::max);
        assert!(maximum_thickness - minimum_thickness > 0.01);
        assert!(
            refined
                .as_slice()
                .iter()
                .any(|sample| sample.island() > 0.0)
        );
    }

    #[test]
    fn refinement_keeps_large_silent_regions_and_supports_multiple_scales() {
        let dimensions = super::super::FieldDimensions::new(48, 24).unwrap();
        let mut scalar = super::super::ScalarField::try_new(dimensions).unwrap();
        let mut noise = NoiseField::try_new(dimensions).unwrap();
        for y in 7..17 {
            for x in 8..40 {
                let broad = 0.25 + ((x as f32 - 24.0).abs() / 30.0).sin().abs() * 0.35;
                scalar.write(x, y, broad).unwrap();
            }
        }
        noise.fill(0.35).unwrap();
        for x in 12..36 {
            noise
                .write(x, 12, if x % 3 == 0 { 0.95 } else { 0.25 })
                .unwrap();
        }
        let flow = FlowField::from_scalar(&scalar).unwrap();
        let transported = TransportedField::from_fields(
            &scalar,
            &noise,
            &flow,
            PulseV2Preset::LargeWidescreen,
            false,
        )
        .unwrap();
        let structured = StructuredField::from_fields(
            &scalar,
            &transported,
            &flow,
            PulseV2Preset::LargeWidescreen,
        )
        .unwrap();
        let refined = RefinedEnergyField::from_fields(
            &structured,
            &transported,
            &flow,
            PulseV2Preset::LargeWidescreen,
            false,
        )
        .unwrap();
        let silent = refined
            .as_slice()
            .iter()
            .filter(|sample| sample.energy() <= ACTIVE_EPSILON)
            .count();
        let active = refined
            .as_slice()
            .iter()
            .filter(|sample| sample.energy() > ACTIVE_EPSILON)
            .count();

        assert!(silent > active / 4);
        assert!(
            refined
                .as_slice()
                .iter()
                .any(|sample| sample.branching() > 0.0)
        );
        assert!(
            refined
                .as_slice()
                .iter()
                .any(|sample| sample.dissipation() > 0.0)
        );
    }

    #[test]
    fn reduced_motion_is_static_and_dimensions_are_safe() {
        let dimensions = super::super::FieldDimensions::new(9, 5).unwrap();
        let structured = StructuredField::try_new(dimensions).unwrap();
        let transported = TransportedField::try_new(dimensions).unwrap();
        let flow = FlowField::from_scalar(&super::super::ScalarField::try_new(dimensions).unwrap())
            .unwrap();
        let first = RefinedEnergyField::from_fields(
            &structured,
            &transported,
            &flow,
            PulseV2Preset::Compact,
            true,
        )
        .unwrap();
        let later = RefinedEnergyField::from_fields(
            &structured,
            &transported,
            &flow,
            PulseV2Preset::Compact,
            true,
        )
        .unwrap();

        assert_eq!(first, later);
        assert_eq!(first.read(9, 0), None);
        assert!(
            RefinedEnergyField::try_new(super::super::FieldDimensions::new(0, 0).unwrap())
                .unwrap()
                .is_empty()
        );
    }

    #[test]
    fn coherent_turbulence_preserves_composition_and_silence() {
        let frame = generated(4.0, false);
        let dimensions = frame.dimensions();
        let structured = frame.structured().as_slice();
        let refined = frame.refined().as_slice();
        let mut source_values = Vec::with_capacity(structured.len());
        let mut refined_values = Vec::with_capacity(refined.len());
        let mut source_active = 0;
        let mut refined_active = 0;
        let mut local_change = 0.0;

        for (base, sample) in structured.iter().copied().zip(refined) {
            source_values.push(base);
            refined_values.push(sample.energy());
            if base > ACTIVE_EPSILON {
                source_active += 1;
                local_change += (sample.energy() - base).abs();
            } else {
                assert_eq!(*sample, RefinedEnergySample::default());
            }
            if sample.energy() > ACTIVE_EPSILON {
                refined_active += 1;
            }
        }

        let source_centroid =
            weighted_centroid(&source_values, dimensions.width(), dimensions.height());
        let refined_centroid =
            weighted_centroid(&refined_values, dimensions.width(), dimensions.height());
        let centroid_shift = ((source_centroid.0 - refined_centroid.0).powi(2)
            + (source_centroid.1 - refined_centroid.1).powi(2))
        .sqrt();

        assert!(source_active > 0);
        assert!(refined_active <= source_active);
        assert!(local_change > 0.0);
        assert!(centroid_shift < 0.08);
    }

    #[test]
    fn coherent_turbulence_adds_small_local_reconnections() {
        let frame = generated(7.0, false);
        let samples = frame.refined().as_slice();
        let branching_values: Vec<f32> = samples.iter().map(|sample| sample.branching()).collect();
        let thickness_values: Vec<f32> = samples.iter().map(|sample| sample.thickness()).collect();
        let branching_max = branching_values.iter().copied().fold(0.0, f32::max);
        let thickness_range = thickness_values.iter().copied().fold(0.0, f32::max)
            - thickness_values.iter().copied().fold(1.0, f32::min);
        let active_branching = branching_values
            .iter()
            .filter(|value| **value > ACTIVE_EPSILON)
            .count();

        assert!(active_branching > 0);
        assert!(branching_max <= 1.0);
        assert!(thickness_range > 0.01);
        assert!(
            samples
                .iter()
                .filter(|sample| sample.energy() > ACTIVE_EPSILON)
                .all(|sample| sample.branching() < 0.95 || sample.energy() > 0.5)
        );
    }

    #[test]
    fn coherent_turbulence_is_temporally_continuous_and_reduced_motion_quiet() {
        let first = generated(3.0, false);
        let adjacent = generated(3.03, false);
        let static_first = generated(3.0, true);
        let static_later = generated(30.0, true);

        let adjacent_delta = mean_energy_delta(&first, &adjacent);
        assert!(adjacent_delta > 0.0);
        assert!(adjacent_delta < 0.08);
        assert_eq!(static_first.refined(), static_later.refined());

        let dynamic_branching: f32 = first
            .refined()
            .as_slice()
            .iter()
            .map(|sample| sample.branching())
            .sum();
        let reduced_branching: f32 = static_first
            .refined()
            .as_slice()
            .iter()
            .map(|sample| sample.branching())
            .sum();
        assert!(dynamic_branching > 0.0);
        assert!(reduced_branching <= dynamic_branching);
    }

    #[test]
    fn density_forms_an_irregular_dominant_mass_with_lateral_support() {
        let frame = generated(5.0, false);
        let refined = frame.refined();
        let (min_x, min_y, max_x, max_y, count) = high_energy_bounds(refined).unwrap();
        let mass_width = max_x - min_x + 1;
        let mass_height = max_y - min_y + 1;
        let mirror_difference = refined
            .as_slice()
            .chunks(refined.width().max(1))
            .map(|row| {
                row.iter()
                    .enumerate()
                    .map(|(x, sample)| {
                        (sample.energy() - row[row.len().saturating_sub(x + 1)].energy()).abs()
                    })
                    .sum::<f32>()
            })
            .sum::<f32>();
        let supported_cells = (0..refined.height())
            .flat_map(|y| (0..refined.width()).map(move |x| (x, y)))
            .filter(|&(x, y)| {
                let flow = frame.flow().read(x, y).unwrap_or_default();
                let (support, _) = lateral_structured_support(frame.structured(), x, y, flow);
                support > ACTIVE_EPSILON
                    && frame.structured().read(x, y).unwrap_or(0.0) > ACTIVE_EPSILON
            })
            .count();

        assert!(count >= 6);
        assert!(mass_width >= 3 && mass_height >= 3);
        assert!(mass_width > mass_height / 2);
        assert!(mirror_difference > 0.05);
        assert!(supported_cells > count);
    }

    #[test]
    fn density_connects_existing_regions_without_filling_silence() {
        let frame = generated(9.0, false);
        let structured = frame.structured();
        let refined = frame.refined();
        let threshold = refined
            .as_slice()
            .iter()
            .map(|sample| sample.energy())
            .fold(0.0, f32::max)
            * 0.35;
        let largest = largest_connected_component(refined, threshold);
        let mut active_before = 0;
        let mut active_after = 0;

        for (base, sample) in structured
            .as_slice()
            .iter()
            .copied()
            .zip(refined.as_slice())
        {
            if base > ACTIVE_EPSILON {
                active_before += 1;
            } else {
                assert_eq!(*sample, RefinedEnergySample::default());
            }
            if sample.energy() > ACTIVE_EPSILON {
                active_after += 1;
            }
        }

        assert!(largest >= refined.as_slice().len() / 25);
        assert!(active_after <= active_before);
        assert!(refined.as_slice().iter().any(|sample| {
            sample.energy() > ACTIVE_EPSILON
                && sample.energy()
                    < refined
                        .as_slice()
                        .iter()
                        .map(|other| other.energy())
                        .fold(0.0, f32::max)
        }));
    }
}
