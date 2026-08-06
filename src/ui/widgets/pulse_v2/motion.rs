use std::time::Duration;

use super::{FieldDimensions, FieldError, NormalizedPulseInput, PulseV2Preset, ScalarField};

const TAU: f32 = std::f32::consts::TAU;
const FLOW_LOBES: usize = 6;
const ELASTIC_NODES: usize = 4;
const PHYSIOLOGY_WIDTH: usize = 10;
const PHYSIOLOGY_HEIGHT: usize = 6;
const PHYSIOLOGY_CELLS: usize = PHYSIOLOGY_WIDTH * PHYSIOLOGY_HEIGHT;

#[derive(Debug, Clone, Copy)]
struct MotionParameters {
    particles: usize,
    advection_steps: usize,
    splat_stride: usize,
    step_size: f32,
    density: f32,
    splat_radius: f32,
    margin: f32,
    virtual_margin: f32,
}

fn parameters(preset: PulseV2Preset) -> MotionParameters {
    match preset {
        PulseV2Preset::Compact => MotionParameters {
            particles: 44,
            advection_steps: 22,
            splat_stride: 2,
            step_size: 0.025,
            density: 0.68,
            splat_radius: 0.82,
            margin: 0.08,
            virtual_margin: 0.16,
        },
        PulseV2Preset::Standard => MotionParameters {
            particles: 72,
            advection_steps: 30,
            splat_stride: 2,
            step_size: 0.022,
            density: 0.78,
            splat_radius: 0.76,
            margin: 0.06,
            virtual_margin: 0.20,
        },
        PulseV2Preset::Widescreen => MotionParameters {
            particles: 112,
            advection_steps: 38,
            splat_stride: 2,
            step_size: 0.019,
            density: 0.86,
            splat_radius: 0.72,
            margin: 0.045,
            virtual_margin: 0.24,
        },
        PulseV2Preset::LargeWidescreen => MotionParameters {
            particles: 152,
            advection_steps: 46,
            splat_stride: 2,
            step_size: 0.017,
            density: 0.92,
            splat_radius: 0.68,
            margin: 0.035,
            virtual_margin: 0.27,
        },
    }
}

#[derive(Debug, Clone, Copy)]
struct MotionLobe {
    center: (f32, f32),
    strength: f32,
    falloff: f32,
}

#[derive(Debug, Clone, Copy)]
struct MotionCore {
    position: (f32, f32),
    radius: f32,
    pull: f32,
    reconnection: f32,
    compression: f32,
    breath: f32,
}

#[derive(Debug, Clone, Copy)]
struct MotionParticle {
    position: (f32, f32),
    phase: f32,
    direction: f32,
    depth: f32,
    layer: usize,
    edge: bool,
}

#[derive(Debug, Clone, Copy)]
struct ElasticNode {
    center: (f32, f32),
    radius: f32,
    phase: f32,
    stored_energy: f32,
    tension: f32,
    contraction: f32,
    release: f32,
    recovery: f32,
}

#[derive(Debug, Clone, Copy, Default)]
struct ElasticInfluence {
    center: (f32, f32),
    stored_energy: f32,
    tension: f32,
    contraction: f32,
    release: f32,
    recovery: f32,
}

#[derive(Debug, Clone, Copy, Default, PartialEq)]
struct PhysiologyCell {
    tension: f32,
    contraction: f32,
    recovery: f32,
}

#[derive(Debug, Clone, Copy, Default)]
struct PhysiologyInfluence {
    tension: f32,
    contraction: f32,
    recovery: f32,
    release: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct PhysiologyGrid {
    cells: [PhysiologyCell; PHYSIOLOGY_CELLS],
}

impl PhysiologyGrid {
    fn new(
        core: MotionCore,
        input: NormalizedPulseInput,
        density: f32,
        pressure: f32,
        elapsed_seconds: f32,
        reduced_motion: bool,
        aspect: f32,
    ) -> Self {
        let cells = std::array::from_fn(|index| {
            let x = index % PHYSIOLOGY_WIDTH;
            let y = index / PHYSIOLOGY_WIDTH;
            let position = (
                x as f32 / (PHYSIOLOGY_WIDTH - 1) as f32,
                y as f32 / (PHYSIOLOGY_HEIGHT - 1) as f32,
            );
            let distance =
                (position.0 - core.position.0).hypot((position.1 - core.position.1) * aspect);
            let proximity =
                smoothstep(1.0 - distance / (core.radius * (6.0 + core.breath * 0.5)).max(0.001));
            let activity = (0.20
                + density * 0.22
                + pressure * 0.24
                + input.load() * 0.13
                + input.cpu() * 0.06
                + input.memory() * 0.06
                + input.gpu().unwrap_or(0.0) * 0.05
                + proximity * 0.18)
                .clamp(0.0, 1.0);
            let phase = hash01(index * 197 + 43) * TAU;
            let current = if reduced_motion {
                0.5
            } else {
                temporal_noise(elapsed_seconds, phase, 0.105, index as u32 + 101)
            };
            let previous = if reduced_motion {
                0.5
            } else {
                temporal_noise(elapsed_seconds - 0.58, phase, 0.105, index as u32 + 101)
            };
            let tension = (activity * 0.54 + current * 0.30 + previous * 0.16).clamp(0.0, 1.0);
            let contraction = smoothstep((tension - 0.38) / 0.50).clamp(0.0, 1.0);
            let recovery = ((1.0 - current) * 0.44
                + smoothstep((previous - current + 0.02) / 0.30) * 0.20)
                .clamp(0.0, 1.0);
            PhysiologyCell {
                tension,
                contraction,
                recovery,
            }
        });
        Self { cells }
    }

    fn relax(&mut self) {
        for cell in &mut self.cells {
            let release = smoothstep((cell.tension - 0.56) / 0.28) * (1.0 - cell.recovery * 0.55);
            let tension_loss = 0.004 + cell.recovery * 0.006 + release * 0.004;
            cell.tension = (cell.tension * (1.0 - tension_loss)).clamp(0.0, 1.0);
            cell.recovery = (cell.recovery + 0.003 + release * 0.012).clamp(0.0, 1.0);
            cell.contraction = (cell.contraction * 0.988 + cell.tension * 0.012
                - cell.recovery * 0.010)
                .clamp(0.0, 1.0);
        }
    }

    fn stimulate(&mut self, position: (f32, f32), drive: f32) -> PhysiologyInfluence {
        let center_x =
            (position.0.clamp(0.0, 1.0) * (PHYSIOLOGY_WIDTH - 1) as f32).round() as isize;
        let center_y =
            (position.1.clamp(0.0, 1.0) * (PHYSIOLOGY_HEIGHT - 1) as f32).round() as isize;
        let drive = drive.clamp(0.0, 1.0);
        let neighbors = [
            (0_isize, 0_isize, 1.0_f32),
            (-1, 0, 0.24),
            (1, 0, 0.24),
            (0, -1, 0.18),
            (0, 1, 0.18),
        ];

        for (offset_x, offset_y, weight) in neighbors {
            let x = center_x + offset_x;
            let y = center_y + offset_y;
            if x < 0 || y < 0 || x >= PHYSIOLOGY_WIDTH as isize || y >= PHYSIOLOGY_HEIGHT as isize {
                continue;
            }
            let index = y as usize * PHYSIOLOGY_WIDTH + x as usize;
            let cell = &mut self.cells[index];
            let recovery_gate = 1.0 - cell.recovery * 0.48;
            let local_drive = drive * weight;
            cell.tension = (cell.tension + local_drive * 0.024 * recovery_gate
                - cell.recovery * 0.004)
                .clamp(0.0, 1.0);
            cell.contraction =
                (cell.contraction + local_drive * 0.018 * recovery_gate + cell.tension * 0.004
                    - cell.recovery * 0.012)
                    .clamp(0.0, 1.0);
            let release = smoothstep((cell.tension - 0.56) / 0.28) * (1.0 - cell.recovery * 0.55);
            cell.recovery =
                (cell.recovery + (1.0 - drive) * 0.002 + release * 0.010).clamp(0.0, 1.0);
        }

        self.influence(center_x, center_y)
    }

    fn influence(&self, center_x: isize, center_y: isize) -> PhysiologyInfluence {
        let neighbors = [
            (0_isize, 0_isize, 1.0_f32),
            (-1, 0, 0.24),
            (1, 0, 0.24),
            (0, -1, 0.18),
            (0, 1, 0.18),
        ];
        let mut total = 0.0;
        let mut tension = 0.0;
        let mut contraction = 0.0;
        let mut recovery = 0.0;
        for (offset_x, offset_y, weight) in neighbors {
            let x = center_x + offset_x;
            let y = center_y + offset_y;
            if x < 0 || y < 0 || x >= PHYSIOLOGY_WIDTH as isize || y >= PHYSIOLOGY_HEIGHT as isize {
                continue;
            }
            let cell = self.cells[y as usize * PHYSIOLOGY_WIDTH + x as usize];
            total += weight;
            tension += cell.tension * weight;
            contraction += cell.contraction * weight;
            recovery += cell.recovery * weight;
        }
        if total <= f32::EPSILON {
            return PhysiologyInfluence::default();
        }
        let inverse = 1.0 / total;
        let tension = (tension * inverse).clamp(0.0, 1.0);
        let recovery = (recovery * inverse).clamp(0.0, 1.0);
        PhysiologyInfluence {
            tension,
            contraction: (contraction * inverse * (1.0 - recovery * 0.38)).clamp(0.0, 1.0),
            recovery,
            release: (smoothstep((tension - 0.56) / 0.28) * (1.0 - recovery * 0.55))
                .clamp(0.0, 1.0),
        }
    }
}

/// V1's organic motion model expressed as a V2 density source.
///
/// The model deliberately stops at normalized positions, velocities and
/// deposition. It does not know terminal cells, colors or rasterization. V2
/// stages remain responsible for organization, lighting and presentation.
#[derive(Debug, Clone, Copy)]
pub(crate) struct OrganicMotion {
    input: NormalizedPulseInput,
    aspect: f32,
    elapsed_seconds: f32,
    intensity: f32,
    pressure: f32,
    density: f32,
    amplitude: f32,
    speed: f32,
    core: MotionCore,
    lobes: [MotionLobe; FLOW_LOBES],
    elasticity: [ElasticNode; ELASTIC_NODES],
    physiology: PhysiologyGrid,
    network_bias: f32,
    reduced_motion: bool,
}

impl OrganicMotion {
    pub(crate) fn new(
        dimensions: FieldDimensions,
        input: NormalizedPulseInput,
        elapsed: Duration,
        reduced_motion: bool,
    ) -> Self {
        let pressure = pressure(input);
        let intensity = (pressure + input.volatility() * 0.10).clamp(0.0, 1.0);
        let density = (0.30 + intensity * 0.70 + input.volatility() * 0.20).clamp(0.0, 1.0);
        let amplitude = 0.75 + pressure * 0.30 + input.volatility() * 0.20;
        let speed = 1.0 + intensity * 1.8 + input.volatility() * 0.45;
        let elapsed_seconds = if reduced_motion {
            0.0
        } else {
            elapsed.as_secs_f32().min(1_000_000.0)
        };
        let breath = if reduced_motion {
            0.5
        } else {
            (elapsed_seconds * speed).sin() * 0.5 + 0.5
        };
        let drift = elapsed_seconds * 0.045;
        let core = MotionCore {
            position: (
                (0.50 + drift.sin() * 0.055 + (input.network() - 0.30) * 0.018).clamp(0.34, 0.66),
                (0.48 + (drift * 0.73 + 0.8).cos() * 0.065 + (input.load() - 0.35) * 0.018)
                    .clamp(0.32, 0.68),
            ),
            radius: (0.105 + input.memory() * 0.035 + pressure * 0.035 + breath * 0.012)
                .clamp(0.095, 0.19),
            pull: (0.17 + input.load() * 0.08 + pressure * 0.15 + breath * 0.018).clamp(0.17, 0.42),
            reconnection: (0.34 + input.network() * 0.18 + input.volatility() * 0.12)
                .clamp(0.34, 0.64),
            compression: (0.64 + input.memory() * 0.22 + pressure * 0.15 + breath * 0.05)
                .clamp(0.64, 1.0),
            breath,
        };
        let aspect =
            (dimensions.height().max(1) as f32 / dimensions.width().max(1) as f32).clamp(0.22, 3.5);
        let anchors = [
            (0.16, 0.24, 0.73, 1.65),
            (0.34, 0.72, -0.51, 1.85),
            (0.52, 0.37, 0.39, 2.10),
            (0.68, 0.68, -0.62, 1.45),
            (0.84, 0.31, 0.47, 1.75),
            (0.90, 0.76, -0.36, 1.60),
        ];
        let lobes = std::array::from_fn(|index| {
            let (anchor_x, anchor_y, lobe_speed, falloff) = anchors[index];
            let phase = hash01(index * 83 + 17) * TAU;
            let lobe_time =
                elapsed_seconds * (0.018 + input.volatility() * 0.014) * lobe_speed + phase;
            MotionLobe {
                center: (
                    (anchor_x + lobe_time.sin() * 0.045 + (core.position.0 - 0.5) * 0.14)
                        .clamp(0.04, 0.96),
                    (anchor_y + (lobe_time * 0.83).cos() * 0.065 + (core.position.1 - 0.5) * 0.12)
                        .clamp(0.04, 0.96),
                ),
                strength: (0.72 + density * 0.42 + input.gpu().unwrap_or(0.0) * 0.08)
                    * if index % 2 == 0 { 1.0 } else { -0.82 },
                falloff,
            }
        });

        let elastic_offsets = [
            (-0.17, -0.10, 0.115, 0.0),
            (-0.06, 0.115, 0.105, 1.7),
            (0.08, -0.075, 0.100, 3.25),
            (0.18, 0.095, 0.115, 4.8),
        ];
        let base_activity = (0.34
            + pressure * 0.34
            + density * 0.20
            + input.load() * 0.12
            + input.volatility() * 0.10)
            .clamp(0.0, 1.0);
        let elasticity = std::array::from_fn(|index| {
            let (offset_x, offset_y, radius, phase) = elastic_offsets[index];
            let variation = 0.86 + hash01(index * 233 + 71) * 0.24;
            let center = (
                (core.position.0 + offset_x * (0.90 + input.memory() * 0.18)).clamp(0.20, 0.80),
                (core.position.1 + offset_y * (0.88 + pressure * 0.16)).clamp(0.18, 0.82),
            );
            let current = if reduced_motion {
                0.52
            } else {
                temporal_noise(elapsed_seconds, phase, 0.18, index as u32 + 17)
            };
            let previous = if reduced_motion {
                0.52
            } else {
                temporal_noise(elapsed_seconds - 0.72, phase, 0.18, index as u32 + 17)
            };
            let drive = (0.40 + current * 0.60) * variation;
            let stored = (0.40 + previous * 0.60) * variation;
            let tension = (base_activity * 0.46 + drive * 0.54).clamp(0.0, 1.0);
            let contraction = smoothstep((tension - 0.40) / 0.48).clamp(0.0, 1.0);
            let release = (smoothstep((stored - drive + 0.035) / 0.22)
                * smoothstep((stored - 0.42) / 0.42))
            .clamp(0.0, 1.0);
            let recovery = ((1.0 - current) * 0.62 + release * 0.38).clamp(0.0, 1.0);
            ElasticNode {
                center,
                radius,
                phase,
                stored_energy: stored.clamp(0.0, 1.0),
                tension,
                contraction,
                release,
                recovery,
            }
        });
        let physiology = PhysiologyGrid::new(
            core,
            input,
            density,
            pressure,
            elapsed_seconds,
            reduced_motion,
            aspect,
        );

        Self {
            input,
            aspect,
            elapsed_seconds,
            intensity,
            pressure,
            density,
            amplitude,
            speed,
            core,
            lobes,
            elasticity,
            physiology,
            network_bias: (input.network() - 0.25).clamp(-0.25, 0.75),
            reduced_motion,
        }
    }

    pub(crate) fn deposit_into(
        self,
        field: &mut ScalarField,
        preset: PulseV2Preset,
    ) -> Result<(), FieldError> {
        if field.is_empty() {
            return Ok(());
        }
        let mut physiology = self.physiology;
        physiology.relax();
        let parameters = parameters(preset);
        let particle_count = ((parameters.particles as f32
            * (0.72 + self.density * 0.28)
            * if self.reduced_motion { 0.68 } else { 1.0 })
        .round() as usize)
            .max(12);

        for particle_index in 0..particle_count {
            let particle = particle_spec(particle_index, particle_count, parameters.margin);
            let lead_steps = if particle.edge || self.reduced_motion {
                0
            } else {
                ((self.elapsed_seconds * (0.65 + particle.depth * 0.55) * 5.0
                    + particle.phase * 11.0)
                    .rem_euclid(92.0)) as usize
            };
            let mut position = self.advect(
                particle.position,
                particle.direction,
                lead_steps,
                parameters.step_size,
                parameters.virtual_margin,
            );
            let mut segment_start = position;

            for step in 0..parameters.advection_steps {
                let next_position = self.advect(
                    position,
                    particle.direction,
                    1,
                    parameters.step_size,
                    parameters.virtual_margin,
                );
                if step % parameters.splat_stride == 0 || step + 1 == parameters.advection_steps {
                    let progress =
                        step as f32 / parameters.advection_steps.saturating_sub(1).max(1) as f32;
                    let (_, mut compression, mut reconnection) =
                        self.convergence(position, particle.direction);
                    let existing_energy = sample_scalar(field, position);
                    let elastic = self.elastic_influence(position, existing_energy);
                    let physiological_drive = (0.18
                        + existing_energy * 2.40
                        + compression * 0.52
                        + self.density * 0.16
                        + reconnection * 0.12)
                        .clamp(0.0, 1.0);
                    let physiological = physiology.stimulate(position, physiological_drive);
                    compression = (compression
                        + elastic.contraction * 0.14
                        + physiological.contraction * 0.08)
                        .clamp(0.0, 1.0);
                    reconnection =
                        (reconnection + elastic.release * 0.16 + physiological.release * 0.10)
                            .clamp(0.0, 1.0);
                    let support = self.support_weight(position);
                    let local_rhythm = if self.reduced_motion {
                        0.79
                    } else {
                        0.68 + ((self.elapsed_seconds * 0.16 + step as f32 * 0.13 + particle.phase)
                            .sin()
                            * 0.5
                            + 0.5)
                            * 0.22
                    };
                    let presence = (0.54
                        + compression * (0.92 + self.core.breath * 0.12)
                        + reconnection * 0.18)
                        .clamp(0.0, 1.55);
                    let edge_boost = if particle.edge { 2.40 } else { 1.0 };
                    let depth_presence = 0.40 + particle.depth * particle.depth * 0.82;
                    let energy =
                        (0.075 + self.density * parameters.density * 0.12 + local_rhythm * 0.045)
                            * (0.86 + self.amplitude * 0.14)
                            * presence
                            * depth_presence
                            * (0.72 + support * 0.38)
                            * edge_boost
                            * (1.0
                                + elastic.tension * 0.045
                                + elastic.release * 0.10
                                + elastic.stored_energy * elastic.tension * 0.035
                                + physiological.tension * 0.025
                                + physiological.release * 0.06);
                    let radius = (parameters.splat_radius
                        + particle.depth * 0.16
                        + compression * 0.52
                        + support * 0.12
                        + elastic.contraction * 0.11
                        - elastic.release * 0.035
                        + physiological.contraction * 0.075
                        - physiological.recovery * 0.018
                        + if particle.edge { 0.42 } else { 0.0 })
                        * self.radius_variation(particle, progress)
                        * (0.98 + elastic.recovery * 0.025);
                    let projected_start = self.project_compression(
                        segment_start,
                        particle.phase,
                        particle.layer,
                        particle.depth,
                        elastic,
                        physiological,
                    );
                    let projected_end = self.project_compression(
                        next_position,
                        particle.phase,
                        particle.layer,
                        particle.depth,
                        elastic,
                        physiological,
                    );
                    self.deposit_segment(field, projected_start, projected_end, energy, radius)?;
                    segment_start = next_position;
                }
                position = next_position;
            }
        }

        self.deposit_atmosphere(field, preset)?;
        self.deposit_return(field, preset)?;
        field.diffuse(0.72 + self.density * 0.04);
        Ok(())
    }

    fn deposit_atmosphere(
        self,
        field: &mut ScalarField,
        preset: PulseV2Preset,
    ) -> Result<(), FieldError> {
        let count = match preset {
            PulseV2Preset::Compact => 1,
            PulseV2Preset::Standard => 2,
            PulseV2Preset::Widescreen => 3,
            PulseV2Preset::LargeWidescreen => 4,
        };
        let steps = (field.width() as f32 * 1.20).round().clamp(24.0, 160.0) as usize;
        for filament in 0..count {
            let phase = hash01(filament * 113 + 41) * TAU;
            let depth = 0.035 + hash01(filament * 127 + 53) * 0.11;
            let mut position = (
                -0.14 - hash01(filament * 139 + 67) * 0.06,
                (0.045
                    + 0.13
                    + hash01(filament * 149 + 79) * 0.11
                    + (self.elapsed_seconds * 0.008 + phase).sin() * 0.008)
                    .clamp(0.045, 0.285),
            );
            let mut start = position;
            for step in 0..steps {
                let progress = step as f32 / steps.max(1) as f32;
                let velocity = self.velocity(position);
                let phase = phase + self.elapsed_seconds * 0.008 + progress * TAU * 0.34;
                let next = (
                    position.0 + 1.26 / steps.max(1) as f32 * (0.88 + velocity.0 * 0.12),
                    (position.1 + velocity.1 * 0.004 + phase.sin() * 0.0016).clamp(0.045, 0.285),
                );
                self.deposit_segment(field, start, next, 0.060, 0.62 + depth * 0.26)?;
                start = next;
                position = next;
            }
        }
        Ok(())
    }

    fn deposit_return(
        self,
        field: &mut ScalarField,
        preset: PulseV2Preset,
    ) -> Result<(), FieldError> {
        let count = match preset {
            PulseV2Preset::Compact => 1,
            PulseV2Preset::Standard => 2,
            PulseV2Preset::Widescreen => 2,
            PulseV2Preset::LargeWidescreen => 3,
        };
        let steps = (field.width() as f32 * 1.08).round().clamp(24.0, 140.0) as usize;
        for branch in 0..count {
            let phase = hash01(branch * 181 + 97) * TAU;
            let depth = 0.050 + hash01(branch * 193 + 109) * 0.095;
            let mut position = (
                -0.12 - hash01(branch * 199 + 113) * 0.05,
                (0.78 + hash01(branch * 211 + 127) * 0.12).clamp(0.64, 0.96),
            );
            let mut start = position;
            for step in 0..steps {
                let progress = step as f32 / steps.max(1) as f32;
                let velocity = self.velocity(position);
                let slow_phase = phase + self.elapsed_seconds * 0.006 + progress * TAU * 0.18;
                let next = (
                    position.0 + 1.20 / steps.max(1) as f32 * (0.84 + velocity.0 * 0.16),
                    (position.1 + velocity.1 * 0.003 + slow_phase.sin() * 0.0014).clamp(0.64, 0.96),
                );
                self.deposit_segment(field, start, next, 0.047, 0.52 + depth * 0.22)?;
                start = next;
                position = next;
            }
        }
        Ok(())
    }

    fn deposit_segment(
        self,
        field: &mut ScalarField,
        start: (f32, f32),
        end: (f32, f32),
        energy: f32,
        radius: f32,
    ) -> Result<(), FieldError> {
        let wraps = (end.0 - start.0).abs() > 0.5;
        let length = ((end.0 - start.0) * field.width() as f32)
            .hypot((end.1 - start.1) * field.height() as f32);
        let subdivisions = if wraps {
            1
        } else {
            length.ceil().clamp(2.0, 6.0) as usize
        };
        for index in 0..=subdivisions {
            let progress = index as f32 / subdivisions as f32;
            let position = (
                start.0 + (end.0 - start.0) * progress,
                start.1 + (end.1 - start.1) * progress,
            );
            let local_energy = energy * (0.86 - progress * 0.08);
            let velocity = (end.0 - start.0, end.1 - start.1);
            let length = velocity.0.hypot(velocity.1).max(0.001);
            let direction = (velocity.0 / length, velocity.1 / length);
            let offset = (
                direction.0 * radius * 0.42 / field.width().max(1) as f32,
                direction.1 * radius * 0.42 / field.height().max(1) as f32,
            );
            field.deposit_normalized(position, local_energy * 0.78, radius)?;
            field.deposit_normalized(
                (position.0 + offset.0, position.1 + offset.1),
                local_energy * 0.22,
                radius * 0.58,
            )?;
        }
        if (start.0 - end.0).abs() > f32::EPSILON {
            for boundary in [0.0, 1.0] {
                if (start.0 - boundary) * (end.0 - boundary) <= 0.0 {
                    let progress = (boundary - start.0) / (end.0 - start.0);
                    if (0.0..=1.0).contains(&progress) {
                        field.deposit_normalized(
                            (boundary, start.1 + (end.1 - start.1) * progress),
                            energy * 0.82,
                            radius,
                        )?;
                    }
                }
            }
        }
        Ok(())
    }

    fn advect(
        self,
        mut position: (f32, f32),
        direction: f32,
        steps: usize,
        step_size: f32,
        margin: f32,
    ) -> (f32, f32) {
        for _ in 0..steps {
            let velocity = self.velocity(position);
            position.0 = wrap_virtual(position.0 + velocity.0 * step_size * direction, margin);
            position.1 = reflect(
                position.1 + velocity.1 * step_size * direction,
                0.025,
                0.975,
            );
        }
        position
    }

    fn velocity(self, position: (f32, f32)) -> (f32, f32) {
        let mut gradient_x = 0.0;
        let mut gradient_y = 0.0;
        for lobe in self.lobes {
            let dx = position.0 - lobe.center.0;
            let dy = (position.1 - lobe.center.1) * self.aspect;
            let potential = lobe.strength * (-lobe.falloff * (dx * dx + dy * dy)).exp();
            gradient_x += potential * -2.0 * lobe.falloff * dx;
            gradient_y += potential * -2.0 * lobe.falloff * dy;
        }

        let mut vx = gradient_y * 0.18 + 0.72 + self.network_bias * 0.20;
        let mut vy = -gradient_x * 0.18 / self.aspect.max(0.2);
        let dx = self.core.position.0 - position.0;
        let dy = (self.core.position.1 - position.1) * self.aspect;
        let distance = dx.hypot(dy);
        let capture_radius = self.core.radius * (5.2 + self.core.breath * 0.7);
        let capture = (1.0 - smoothstep(distance / capture_radius.max(0.001))).clamp(0.0, 1.0);
        let pull = self.core.pull * capture * (0.92 + self.core.breath * 0.16) * 4.0;
        vx += dx * pull;
        vy += dy * pull;
        let swirl = self.core.pull * capture * 0.12;
        vx += -dy * swirl;
        vy += dx * swirl;

        let exit = smoothstep(
            (distance - self.core.radius * (0.80 + self.core.breath * 0.12))
                / (self.core.radius * 2.4),
        );
        vx += self.core.reconnection * exit * 0.18;
        vy += (self.core.position.1 - position.1) * self.core.reconnection * exit * 0.04;

        let edge_force = if position.0 < 0.14 {
            smoothstep((0.14 - position.0) / 0.20)
        } else if position.0 > 0.86 {
            smoothstep((position.0 - 0.86) / 0.20)
        } else {
            0.0
        };
        vx += edge_force * 0.42;

        let length = vx.hypot(vy).max(0.001);
        (vx / length, vy / length)
    }

    fn convergence(self, position: (f32, f32), direction: f32) -> (f32, f32, f32) {
        let to_core = (
            self.core.position.0 - position.0,
            (self.core.position.1 - position.1) * self.aspect,
        );
        let distance = to_core.0.hypot(to_core.1);
        let influence = self.core.radius * (4.2 + self.core.breath * 0.7);
        let proximity = (1.0 - smoothstep(distance / influence.max(0.001))).clamp(0.0, 1.0);
        let velocity = self.velocity(position);
        let velocity = (velocity.0 * direction, velocity.1 * direction);
        let toward = if distance > 0.001 {
            ((velocity.0 * to_core.0 + velocity.1 * to_core.1) / distance).max(0.0)
        } else {
            1.0
        };
        let away = if distance > 0.001 {
            ((velocity.0 * -to_core.0 + velocity.1 * -to_core.1) / distance).max(0.0)
        } else {
            0.0
        };
        let compression =
            (proximity * (0.24 + toward * 0.76) * self.core.compression).clamp(0.0, 1.0);
        let outside = smoothstep(
            (distance - self.core.radius * (1.0 + self.core.breath * 0.18))
                / (self.core.radius * 2.2),
        );
        let reconnection =
            (outside * away * (0.22 + proximity * 0.78) * self.core.reconnection).clamp(0.0, 1.0);
        (proximity, compression, reconnection)
    }

    fn elastic_influence(self, position: (f32, f32), loading: f32) -> ElasticInfluence {
        let mut total_weight = 0.0;
        let mut center = (0.0, 0.0);
        let mut stored_energy = 0.0;
        let mut tension = 0.0;
        let mut contraction = 0.0;
        let mut release = 0.0;
        let mut recovery = 0.0;

        for node in self.elasticity {
            let distance =
                (position.0 - node.center.0).hypot((position.1 - node.center.1) * self.aspect);
            let radius = node.radius * (1.0 + self.core.breath * 0.12);
            let weight = smoothstep(1.0 - distance / (radius * 1.85).max(0.001));
            if weight <= 0.0 {
                continue;
            }
            total_weight += weight;
            center.0 += node.center.0 * weight;
            center.1 += node.center.1 * weight;
            stored_energy += node.stored_energy * weight;
            tension += node.tension * weight;
            contraction += node.contraction * weight;
            release += node.release * weight;
            recovery += node.recovery * weight;
        }

        if total_weight <= f32::EPSILON {
            return ElasticInfluence::default();
        }

        let inverse = 1.0 / total_weight;
        let activation = 0.58 + loading.clamp(0.0, 1.0) * 0.42;
        ElasticInfluence {
            center: (center.0 * inverse, center.1 * inverse),
            stored_energy: (stored_energy * inverse * activation).clamp(0.0, 1.0),
            tension: (tension * inverse * activation).clamp(0.0, 1.0),
            contraction: (contraction * inverse * activation).clamp(0.0, 1.0),
            release: (release * inverse * (0.72 + loading.clamp(0.0, 1.0) * 0.28)).clamp(0.0, 1.0),
            recovery: (recovery * inverse).clamp(0.0, 1.0),
        }
    }

    fn project_compression(
        self,
        position: (f32, f32),
        phase: f32,
        layer: usize,
        depth: f32,
        elastic: ElasticInfluence,
        physiology: PhysiologyInfluence,
    ) -> (f32, f32) {
        let center = (
            (self.core.position.0 + 0.026 + self.network_bias * 0.014).clamp(0.34, 0.68),
            (self.core.position.1 - 0.018 + self.network_bias * 0.010).clamp(0.30, 0.70),
        );
        let entry_span = (0.25 + self.core.radius * 0.85 + self.pressure * 0.035).clamp(0.25, 0.38);
        let exit_span = (0.23 + self.core.radius * 0.72 + self.pressure * 0.030).clamp(0.23, 0.36);
        let entry_x = (center.0 - entry_span).clamp(0.12, 0.46);
        let exit_x = (center.0 + exit_span).clamp(0.54, 0.88);
        if position.0 < entry_x || position.0 > exit_x {
            return position;
        }

        let incoming = smoothstep((position.0 - entry_x) / (center.0 - entry_x).max(0.001));
        let outgoing = smoothstep((position.0 - center.0) / (exit_x - center.0).max(0.001));
        let strand = (phase * 0.61 + layer as f32 * 1.17).sin() * 0.72
            + (phase * 1.31 + depth * TAU).cos() * 0.28;
        let skew = 0.058 + self.pressure * 0.024 + self.network_bias.abs() * 0.012;
        let centerline = if position.0 <= center.0 {
            center.1 + skew * (1.0 - incoming)
        } else {
            center.1 + skew * 0.55 * outgoing
        };
        let width_factor = if position.0 <= center.0 {
            1.0 - incoming * 0.78
        } else {
            0.22 + outgoing * 0.78
        };
        let strand_offset = strand * (0.009 + depth * 0.014 * outgoing);
        let mut y = centerline + (position.1 - centerline) * width_factor + strand_offset;
        let fusion_half_width =
            (0.065 + self.core.radius * 0.22 + self.pressure * 0.014).clamp(0.065, 0.11);
        let fusion = 1.0 - smoothstep(((position.0 - center.0).abs() / fusion_half_width).max(0.0));
        let fused_center = center.1 + strand * 0.008;
        y += (fused_center - y) * fusion * 0.46;
        let local_contraction = (elastic.contraction * 0.82 + physiology.contraction * 0.18)
            * (1.0 - physiology.recovery * 0.24);
        let local_center = center.1 + (elastic.center.1 - center.1) * 0.24;
        y = local_center + (y - local_center) * (1.0 - local_contraction * 0.16);
        let release = elastic.release * 0.72 + physiology.release * 0.28;
        let x = position.0
            + if position.0 > center.0 {
                release * (0.006 + outgoing * 0.008)
            } else {
                0.0
            };
        (x, y.clamp(-0.06, 1.06))
    }

    fn support_weight(self, position: (f32, f32)) -> f32 {
        let potential = self.potential(position);
        let core_distance = (position.0 - self.core.position.0)
            .hypot((position.1 - self.core.position.1) * self.aspect);
        let core_presence =
            smoothstep(1.0 - core_distance / (self.core.radius * (5.6 + self.core.breath * 0.5)));
        (0.52 + potential.tanh() * 0.12 + core_presence * 0.18).clamp(0.22, 0.92)
    }

    fn potential(self, position: (f32, f32)) -> f32 {
        let mut potential = 0.0;
        for lobe in self.lobes {
            let dx = position.0 - lobe.center.0;
            let dy = (position.1 - lobe.center.1) * self.aspect;
            potential += lobe.strength * (-lobe.falloff * (dx * dx + dy * dy)).exp();
        }
        let dx = self.core.position.0 - position.0;
        let dy = (self.core.position.1 - position.1) * self.aspect;
        let distance = dx.hypot(dy);
        let core_scale = (self.core.radius * (4.6 + self.core.breath * 0.6)).max(0.001);
        potential
            + (-((distance / core_scale).powi(2))).exp() * (0.38 + self.core.compression * 0.24)
    }

    fn radius_variation(self, particle: MotionParticle, progress: f32) -> f32 {
        let phase = particle.phase
            + self.elapsed_seconds * (0.035 + particle.depth * 0.025)
            + progress * TAU * (0.28 + particle.depth * 0.16)
            + particle.layer as f32 * 0.37;
        let variation =
            0.995 + phase.sin() * 0.006 + (phase * 0.71 + particle.phase * 0.23).cos() * 0.004;
        let class_radius = if particle.depth >= 0.68 {
            1.04
        } else if particle.depth >= 0.38 {
            1.0
        } else {
            0.96
        };
        (class_radius * variation).clamp(0.68, 1.18)
    }
}

fn sample_scalar(field: &ScalarField, position: (f32, f32)) -> f32 {
    if field.is_empty() || !(0.0..=1.0).contains(&position.0) {
        return 0.0;
    }
    let x = (position.0 * field.width().saturating_sub(1) as f32).round() as usize;
    let y = (position.1.clamp(0.0, 1.0) * field.height().saturating_sub(1) as f32).round() as usize;
    field.read(x, y).unwrap_or(0.0)
}

fn particle_spec(index: usize, count: usize, margin: f32) -> MotionParticle {
    let edge = index < count.min(16);
    let from_left = index.is_multiple_of(2);
    let position_x = if edge {
        if from_left {
            -0.045 - hash01(index * 19 + 3) * 0.020
        } else {
            1.045 + hash01(index * 23 + 5) * 0.020
        }
    } else {
        margin + hash01(index * 31 + 11) * (1.0 - margin * 2.0)
    };
    let position_y =
        (0.10 + hash01(index * 43 + 17) * 0.80 + (hash01(index * 59 + 29) - 0.5) * 0.08)
            .clamp(margin, 1.0 - margin);
    MotionParticle {
        position: (position_x, position_y),
        phase: hash01(index * 71 + 13) * TAU,
        direction: if edge {
            if from_left { 1.0 } else { -1.0 }
        } else if index.is_multiple_of(3) {
            -1.0
        } else {
            1.0
        },
        depth: 0.14 + hash01(index * 67 + 23) * 0.82,
        layer: index % 4,
        edge,
    }
}

fn pressure(input: NormalizedPulseInput) -> f32 {
    let base =
        input.cpu() * 0.27 + input.memory() * 0.30 + input.network() * 0.10 + input.load() * 0.18;
    match input.gpu() {
        Some(gpu) => (base + gpu * 0.15).clamp(0.0, 1.0),
        None => (base + input.cpu() * 0.04 + input.memory() * 0.08 + input.load() * 0.03)
            .clamp(0.0, 1.0),
    }
}

fn hash01(value: usize) -> f32 {
    let mut hash = value as u32;
    hash = hash.wrapping_mul(0x045d_9f3b);
    hash ^= hash >> 16;
    hash = hash.wrapping_mul(0x045d_9f3b);
    hash ^= hash >> 16;
    hash as f32 / u32::MAX as f32
}

fn temporal_noise(time: f32, phase: f32, rate: f32, seed: u32) -> f32 {
    let coordinate = time * rate + phase;
    let cell = coordinate.floor() as i32;
    let fraction = smoothstep(coordinate - cell as f32);
    let start = hash_temporal(cell, seed);
    let end = hash_temporal(cell.wrapping_add(1), seed);
    start + (end - start) * fraction
}

fn hash_temporal(cell: i32, seed: u32) -> f32 {
    let mut hash = (cell as u32).wrapping_mul(0x045d_9f3b).wrapping_add(seed);
    hash ^= hash >> 16;
    hash = hash.wrapping_mul(0x045d_9f3b);
    hash ^= hash >> 16;
    hash as f32 / u32::MAX as f32
}

fn smoothstep(value: f32) -> f32 {
    let value = value.clamp(0.0, 1.0);
    value * value * (3.0 - 2.0 * value)
}

fn wrap_virtual(value: f32, margin: f32) -> f32 {
    let span = 1.0 + margin * 2.0;
    if value < -margin {
        1.0 + margin - (-margin - value).rem_euclid(span)
    } else if value > 1.0 + margin {
        -margin + (value - (1.0 + margin)).rem_euclid(span)
    } else {
        value
    }
}

fn reflect(value: f32, low: f32, high: f32) -> f32 {
    if value < low {
        low + (low - value)
    } else if value > high {
        high - (value - high)
    } else {
        value
    }
    .clamp(low, high)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn input() -> NormalizedPulseInput {
        NormalizedPulseInput::try_new(0.42, 0.48, Some(0.30), 0.18, 0.35, 0.08).unwrap()
    }

    #[test]
    fn organic_motion_is_deterministic_and_moves_structure_not_only_intensity() {
        let dimensions = FieldDimensions::new(96, 48).unwrap();
        let mut first = ScalarField::try_new(dimensions).unwrap();
        let mut same = ScalarField::try_new(dimensions).unwrap();
        let mut later = ScalarField::try_new(dimensions).unwrap();
        OrganicMotion::new(dimensions, input(), Duration::from_secs_f32(2.0), false)
            .deposit_into(&mut first, PulseV2Preset::Widescreen)
            .unwrap();
        OrganicMotion::new(dimensions, input(), Duration::from_secs_f32(2.0), false)
            .deposit_into(&mut same, PulseV2Preset::Widescreen)
            .unwrap();
        OrganicMotion::new(dimensions, input(), Duration::from_secs_f32(6.0), false)
            .deposit_into(&mut later, PulseV2Preset::Widescreen)
            .unwrap();

        assert_eq!(first, same);
        let occupancy_changes = first
            .as_slice()
            .iter()
            .zip(later.as_slice())
            .filter(|(before, after)| (**before > 0.045) != (**after > 0.045))
            .count();
        assert!(occupancy_changes > dimensions.sample_count() / 100);
        assert_ne!(first, later);
    }

    #[test]
    fn elastic_state_is_local_bounded_and_temporally_continuous() {
        let dimensions = FieldDimensions::new(96, 48).unwrap();
        let first = OrganicMotion::new(dimensions, input(), Duration::from_secs_f32(2.0), false);
        let same = OrganicMotion::new(dimensions, input(), Duration::from_secs_f32(2.0), false);
        let nearby = OrganicMotion::new(dimensions, input(), Duration::from_secs_f32(2.08), false);

        let mut tension_min: f32 = 1.0;
        let mut tension_max: f32 = 0.0;
        let mut release_seen = false;
        for (node, identical) in first.elasticity.iter().zip(same.elasticity) {
            assert_eq!(node.center, identical.center);
            assert!((0.0..=1.0).contains(&node.stored_energy));
            assert!((0.0..=1.0).contains(&node.tension));
            assert!((0.0..=1.0).contains(&node.contraction));
            assert!((0.0..=1.0).contains(&node.release));
            assert!((0.0..=1.0).contains(&node.recovery));
            tension_min = tension_min.min(node.tension);
            tension_max = tension_max.max(node.tension);
            release_seen |= node.release > 0.01;
        }

        assert!(tension_max - tension_min > 0.01);
        assert!(release_seen);
        for (before, after) in first.elasticity.iter().zip(nearby.elasticity) {
            assert!((before.tension - after.tension).abs() < 0.20);
            assert!((before.contraction - after.contraction).abs() < 0.25);
            assert!((before.release - after.release).abs() < 0.30);
        }
    }

    #[test]
    fn physiology_memory_propagates_locally_and_recovers() {
        let mut grid = PhysiologyGrid {
            cells: [PhysiologyCell::default(); PHYSIOLOGY_CELLS],
        };
        let influence = grid.stimulate((0.5, 0.5), 0.95);
        let changed = grid
            .cells
            .iter()
            .filter(|cell| cell.tension > 0.0 || cell.contraction > 0.0)
            .count();

        assert!(changed >= 3);
        assert!(changed <= 5);
        assert!(influence.tension > 0.0);
        assert!(influence.contraction > 0.0);

        let mut recovering = PhysiologyGrid {
            cells: [PhysiologyCell {
                tension: 0.90,
                contraction: 0.80,
                recovery: 0.70,
            }; PHYSIOLOGY_CELLS],
        };
        recovering.relax();
        assert!(recovering.cells[0].tension < 0.90);
        assert!(recovering.cells[0].recovery > 0.70);
        assert_eq!(
            OrganicMotion::new(
                FieldDimensions::new(96, 48).unwrap(),
                input(),
                Duration::ZERO,
                true,
            )
            .physiology,
            OrganicMotion::new(
                FieldDimensions::new(96, 48).unwrap(),
                input(),
                Duration::from_secs(30),
                true,
            )
            .physiology
        );
    }

    #[test]
    fn reduced_motion_freezes_the_organic_deposition() {
        let dimensions = FieldDimensions::new(64, 32).unwrap();
        let mut first = ScalarField::try_new(dimensions).unwrap();
        let mut later = ScalarField::try_new(dimensions).unwrap();
        OrganicMotion::new(dimensions, input(), Duration::ZERO, true)
            .deposit_into(&mut first, PulseV2Preset::Standard)
            .unwrap();
        OrganicMotion::new(dimensions, input(), Duration::from_secs(30), true)
            .deposit_into(&mut later, PulseV2Preset::Standard)
            .unwrap();
        assert_eq!(first, later);
    }
}
