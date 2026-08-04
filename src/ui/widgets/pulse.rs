use crate::{
    app::AppState,
    pulse::{PulseHealth, PulseModel},
    theme::Theme,
    ui::renderer::panel,
};

use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Style},
    text::{Line, Span},
    widgets::Paragraph,
};

const TAU: f32 = std::f32::consts::TAU;

const BRAILLE_DOTS: [(u16, u16, u8); 8] = [
    (0, 0, 0),
    (0, 1, 1),
    (0, 2, 2),
    (1, 0, 3),
    (1, 1, 4),
    (1, 2, 5),
    (0, 3, 6),
    (1, 3, 7),
];

const FLOW_LAYERS: usize = 4;
const FIELD_THRESHOLD: f32 = 0.035;
const ACTIVE_ENERGY_EPSILON: f32 = 0.018;
const LIGHTING_BUCKETS: usize = 64;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PulsePreset {
    Compact,
    Standard,
    Widescreen,
    LargeWidescreen,
}

#[derive(Debug, Clone, Copy)]
struct PresetParameters {
    particles: usize,
    advection_steps: usize,
    splat_stride: usize,
    step_size: f32,
    density: f32,
    splat_radius: f32,
    margin: f32,
    virtual_margin: f32,
    diffusion_passes: usize,
}

impl PulsePreset {
    fn parameters(self) -> PresetParameters {
        match self {
            Self::Compact => PresetParameters {
                particles: 44,
                advection_steps: 22,
                splat_stride: 2,
                step_size: 0.025,
                density: 0.68,
                splat_radius: 0.82,
                margin: 0.08,
                virtual_margin: 0.16,
                diffusion_passes: 1,
            },
            Self::Standard => PresetParameters {
                particles: 72,
                advection_steps: 30,
                splat_stride: 2,
                step_size: 0.022,
                density: 0.78,
                splat_radius: 0.76,
                margin: 0.06,
                virtual_margin: 0.20,
                diffusion_passes: 2,
            },
            Self::Widescreen => PresetParameters {
                particles: 112,
                advection_steps: 38,
                splat_stride: 2,
                step_size: 0.019,
                density: 0.86,
                splat_radius: 0.72,
                margin: 0.045,
                virtual_margin: 0.24,
                diffusion_passes: 2,
            },
            Self::LargeWidescreen => PresetParameters {
                particles: 152,
                advection_steps: 46,
                splat_stride: 2,
                step_size: 0.017,
                density: 0.92,
                splat_radius: 0.68,
                margin: 0.035,
                virtual_margin: 0.27,
                diffusion_passes: 3,
            },
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct EnergySample {
    energy: f32,
    layer: usize,
    depth: f32,
}

impl Default for EnergySample {
    fn default() -> Self {
        Self {
            energy: 0.0,
            layer: 0,
            depth: 0.0,
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct LightingRange {
    distant: f32,
    flow: f32,
    energy: f32,
    peak: f32,
}

impl LightingRange {
    fn signals(self, energy: f32) -> (f32, f32, f32, f32) {
        let distant = smoothstep(
            (energy - ACTIVE_ENERGY_EPSILON) / (self.distant - ACTIVE_ENERGY_EPSILON).max(0.018),
        );
        let flow = smoothstep((energy - self.distant) / (self.flow - self.distant).max(0.025));
        let energetic = smoothstep((energy - self.flow) / (self.energy - self.flow).max(0.04));
        let peak = smoothstep((energy - self.peak) / (1.0 - self.peak).max(0.04));
        (distant, flow, energetic, peak)
    }
}

#[derive(Debug, Clone, PartialEq)]
struct FieldFrame {
    width: usize,
    height: usize,
    dot_aspect: f32,
    samples: Vec<EnergySample>,
}

impl FieldFrame {
    fn new(width: usize, height: usize, braille: bool) -> Self {
        let width = width.max(1);
        let height = height.max(1);
        let dot_aspect = if braille { 1.0 } else { 1.8 };

        Self {
            width,
            height,
            dot_aspect,
            samples: vec![EnergySample::default(); width * height],
        }
    }

    fn sample(&self, x: usize, y: usize) -> EnergySample {
        let x = x.min(self.width - 1);
        let y = y.min(self.height - 1);
        self.samples[y * self.width + x]
    }

    fn lighting_range(&self) -> LightingRange {
        let mut histogram = [0_usize; LIGHTING_BUCKETS];
        let mut populated = 0_usize;
        for sample in &self.samples {
            if sample.energy <= ACTIVE_ENERGY_EPSILON {
                continue;
            }
            let bucket = ((sample.energy * histogram.len() as f32) as usize)
                .min(histogram.len().saturating_sub(1));
            histogram[bucket] += 1;
            populated += 1;
        }

        if populated == 0 {
            return LightingRange {
                distant: FIELD_THRESHOLD,
                flow: FIELD_THRESHOLD + 0.04,
                energy: 0.18,
                peak: 0.45,
            };
        }

        let distant =
            histogram_quantile(&histogram, populated, 0.30).clamp(FIELD_THRESHOLD * 0.90, 0.16);
        let flow = histogram_quantile(&histogram, populated, 0.60)
            .max(distant + 0.018)
            .min(0.35);
        let energy = histogram_quantile(&histogram, populated, 0.84)
            .max(flow + 0.045)
            .min(0.70);
        let peak = histogram_quantile(&histogram, populated, 0.995)
            .max(energy + 0.10)
            .min(1.0);
        LightingRange {
            distant,
            flow,
            energy,
            peak,
        }
    }

    fn local_energy(&self, position: (f32, f32), radius: f32) -> f32 {
        self.local_energy_from(&self.samples, position, radius)
    }

    fn local_energy_from(
        &self,
        samples: &[EnergySample],
        position: (f32, f32),
        radius: f32,
    ) -> f32 {
        let center_x = position.0 * self.width.saturating_sub(1) as f32;
        let center_y = position.1 * self.height.saturating_sub(1) as f32;
        let radius = radius.max(1.0);
        let left = (center_x - radius).floor().max(0.0) as usize;
        let right = (center_x + radius)
            .ceil()
            .min(self.width.saturating_sub(1) as f32) as usize;
        let top = (center_y - radius).floor().max(0.0) as usize;
        let bottom = (center_y + radius)
            .ceil()
            .min(self.height.saturating_sub(1) as f32) as usize;
        let mut energy = 0.0;
        let mut weight = 0.0;

        for y in top..=bottom {
            for x in left..=right {
                let distance = ((x as f32 - center_x).powi(2)
                    + ((y as f32 - center_y) * self.dot_aspect).powi(2))
                .sqrt();
                if distance > radius {
                    continue;
                }
                let local_weight = 1.0 - distance / radius;
                energy += samples[y * self.width + x].energy * local_weight;
                weight += local_weight;
            }
        }

        if weight > 0.0 {
            (energy / weight).clamp(0.0, 1.0)
        } else {
            0.0
        }
    }

    fn deposit(
        &mut self,
        position: (f32, f32),
        energy: f32,
        radius: f32,
        layer: usize,
        depth: f32,
    ) {
        if !position.0.is_finite() || !position.1.is_finite() || !(0.0..=1.0).contains(&position.0)
        {
            return;
        }

        let center_x = position.0 * (self.width.saturating_sub(1) as f32);
        let center_y = position.1.clamp(0.0, 1.0) * (self.height.saturating_sub(1) as f32);
        let radius = radius.max(0.25);
        let left = (center_x - radius).floor().max(0.0) as usize;
        let right = (center_x + radius)
            .ceil()
            .min(self.width.saturating_sub(1) as f32) as usize;
        let top = (center_y - radius).floor().max(0.0) as usize;
        let bottom = (center_y + radius)
            .ceil()
            .min(self.height.saturating_sub(1) as f32) as usize;

        for y in top..=bottom {
            for x in left..=right {
                let distance = ((x as f32 - center_x).powi(2)
                    + ((y as f32 - center_y) * self.dot_aspect).powi(2))
                .sqrt();
                let falloff = smoothstep(1.0 - distance / (radius * self.dot_aspect.max(1.0)));
                if falloff <= 0.0 {
                    continue;
                }

                let contribution = energy * falloff * 0.72;
                let index = y * self.width + x;
                let current = self.samples[index];
                self.samples[index].energy = (current.energy + contribution * 0.62).clamp(0.0, 1.0);
                if contribution >= current.energy {
                    self.samples[index].layer = layer;
                    self.samples[index].depth = depth;
                }
            }
        }
    }

    fn deposit_directional(
        &mut self,
        position: (f32, f32),
        velocity: (f32, f32),
        energy: f32,
        radius: f32,
        layer: usize,
        depth: f32,
    ) {
        let velocity_length = velocity.0.hypot(velocity.1).max(0.001);
        let direction = (velocity.0 / velocity_length, velocity.1 / velocity_length);
        let offset = (
            direction.0 * radius * 0.42 / self.width.max(1) as f32,
            direction.1 * radius * 0.42 / self.height.max(1) as f32,
        );
        self.deposit(position, energy * 0.78, radius, layer, depth);
        self.deposit(
            (position.0 + offset.0, position.1 + offset.1),
            energy * 0.22,
            radius * 0.58,
            layer,
            depth,
        );
    }

    fn deposit_segment(
        &mut self,
        start: (f32, f32),
        end: (f32, f32),
        energy: f32,
        radius: f32,
        layer: usize,
        depth: f32,
    ) {
        let wraps_horizontally = (end.0 - start.0).abs() > 0.5;
        let length =
            ((end.0 - start.0) * self.width as f32).hypot((end.1 - start.1) * self.height as f32);
        let subdivisions = if wraps_horizontally {
            1
        } else {
            length.ceil().clamp(2.0, 6.0) as usize
        };

        for step in 0..=subdivisions {
            let progress = step as f32 / subdivisions as f32;
            let position = (
                start.0 + (end.0 - start.0) * progress,
                start.1 + (end.1 - start.1) * progress,
            );
            let local_energy = energy * (0.86 - progress * 0.08);
            self.deposit_directional(
                position,
                (end.0 - start.0, end.1 - start.1),
                local_energy,
                radius,
                layer,
                depth,
            );
        }

        if (start.0 - end.0).abs() > f32::EPSILON {
            for boundary in [0.0, 1.0] {
                if (start.0 - boundary) * (end.0 - boundary) <= 0.0 {
                    let progress = (boundary - start.0) / (end.0 - start.0);
                    if (0.0..=1.0).contains(&progress) {
                        self.deposit(
                            (boundary, start.1 + (end.1 - start.1) * progress),
                            energy * 0.82,
                            radius,
                            layer,
                            depth,
                        );
                    }
                }
            }
        }
    }

    fn diffuse(&mut self, passes: usize, amount: f32) {
        if passes == 0 {
            return;
        }

        let amount = amount.clamp(0.0, 1.0);
        let mut next = vec![EnergySample::default(); self.samples.len()];
        for _ in 0..passes {
            for y in 0..self.height {
                for x in 0..self.width {
                    let index = y * self.width + x;
                    let current = self.samples[index];
                    let mut neighborhood_energy = current.energy * 0.42;
                    let mut neighborhood_weight = 0.42;
                    let mut strongest = current;

                    for neighbor_y in y.saturating_sub(1)..=(y + 1).min(self.height - 1) {
                        for neighbor_x in x.saturating_sub(1)..=(x + 1).min(self.width - 1) {
                            if neighbor_x == x && neighbor_y == y {
                                continue;
                            }
                            let neighbor = self.samples[neighbor_y * self.width + neighbor_x];
                            let neighbor_weight = if neighbor_x == x || neighbor_y == y {
                                0.10
                            } else {
                                0.045
                            };
                            neighborhood_energy += neighbor.energy * neighbor_weight;
                            neighborhood_weight += neighbor_weight;
                            if neighbor.energy > strongest.energy {
                                strongest = neighbor;
                            }
                        }
                    }

                    next[index] = EnergySample {
                        energy: (current.energy * (1.0 - amount)
                            + neighborhood_energy / neighborhood_weight * amount)
                            .clamp(0.0, 1.0),
                        layer: strongest.layer,
                        depth: current.depth * (1.0 - amount) + strongest.depth * amount,
                    };
                }
            }
            self.samples.clone_from(&next);
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct Particle {
    position: (f32, f32),
    phase: f32,
    direction: f32,
    depth: f32,
    layer: usize,
    edge: bool,
}

#[derive(Debug, Clone, Copy)]
struct FlowLobe {
    center: (f32, f32),
    strength: f32,
    falloff: f32,
}

const FLOW_LOBES: usize = 6;

#[derive(Debug, Clone, Copy)]
struct FlowField {
    aspect: f32,
    core: CoreState,
    lobes: [FlowLobe; FLOW_LOBES],
    network_bias: f32,
}

impl FlowField {
    fn new(
        aspect: f32,
        time: f32,
        health: PulseHealth,
        pulse: &PulseModel,
        core: CoreState,
    ) -> Self {
        let input = pulse.input();
        let anchors = [
            (0.16, 0.24, 0.73, 1.65),
            (0.34, 0.72, -0.51, 1.85),
            (0.52, 0.37, 0.39, 2.10),
            (0.68, 0.68, -0.62, 1.45),
            (0.84, 0.31, 0.47, 1.75),
            (0.90, 0.76, -0.36, 1.60),
        ];
        let lobes = std::array::from_fn(|index| {
            let (anchor_x, anchor_y, speed, falloff) = anchors[index];
            let phase = hash01(index * 83 + 17) * TAU;
            let drift = time * (0.018 + health.volatility() * 0.014) * speed + phase;
            FlowLobe {
                center: (
                    (anchor_x + drift.sin() * 0.045 + (core.position.0 - 0.5) * 0.14)
                        .clamp(0.04, 0.96),
                    (anchor_y + (drift * 0.83).cos() * 0.065 + (core.position.1 - 0.5) * 0.12)
                        .clamp(0.04, 0.96),
                ),
                strength: (0.72 + health.density() * 0.42 + input.gpu.unwrap_or(0.0) * 0.08)
                    * if index % 2 == 0 { 1.0 } else { -0.82 },
                falloff,
            }
        });

        Self {
            aspect,
            core,
            lobes,
            network_bias: (input.network - 0.25).clamp(-0.25, 0.75),
        }
    }

    fn potential(&self, position: (f32, f32)) -> f32 {
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
        let core_potential = (-((distance / core_scale).powi(2))).exp();
        potential + core_potential * (0.38 + self.core.compression * 0.24)
    }

    fn velocity(&self, position: (f32, f32)) -> (f32, f32) {
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
        let breathing = 0.92 + self.core.breath * 0.16;
        let pull = self.core.pull * capture * breathing * 4.0;
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

    fn convergence(&self, position: (f32, f32), direction: f32) -> (f32, f32, f32) {
        let to_core = (
            self.core.position.0 - position.0,
            (self.core.position.1 - position.1) * self.aspect,
        );
        let distance = to_core.0.hypot(to_core.1);
        let influence_radius = self.core.radius * (4.2 + self.core.breath * 0.7);
        let proximity = (1.0 - smoothstep(distance / influence_radius.max(0.001))).clamp(0.0, 1.0);
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
        let outside_core = smoothstep(
            (distance - self.core.radius * (1.0 + self.core.breath * 0.18))
                / (self.core.radius * 2.2),
        );
        let reconnection =
            (outside_core * away * (0.22 + proximity * 0.78) * self.core.reconnection)
                .clamp(0.0, 1.0);
        (proximity, compression, reconnection)
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct CoreState {
    position: (f32, f32),
    radius: f32,
    pull: f32,
    reconnection: f32,
    compression: f32,
    breath: f32,
}

#[derive(Debug, Clone, PartialEq)]
struct EnergyLandscape {
    width: usize,
    height: usize,
    values: Vec<f32>,
}

impl EnergyLandscape {
    fn build(
        width: usize,
        height: usize,
        flow: &FlowField,
        health: PulseHealth,
        time: f32,
        reduced_motion: bool,
    ) -> Self {
        let mut landscape = Self {
            width,
            height,
            values: vec![0.0; width * height],
        };
        let motion = if reduced_motion { 0.0 } else { 1.0 };

        for y in 0..height {
            for x in 0..width {
                let position = (
                    x as f32 / width.saturating_sub(1).max(1) as f32,
                    y as f32 / height.saturating_sub(1).max(1) as f32,
                );
                let broad = potential_signal(flow.potential(position));
                let medium_position = warp_position(position, 1.7, time, 0.034 * motion);
                let medium = potential_signal(flow.potential(medium_position));
                let fine_position = warp_position(position, 3.8, time * 0.72, 0.014 * motion);
                let fine = potential_signal(flow.potential(fine_position));
                let core_distance = (position.0 - flow.core.position.0)
                    .hypot((position.1 - flow.core.position.1) * flow.aspect);
                let core_presence = smoothstep(
                    1.0 - core_distance / (flow.core.radius * (5.6 + flow.core.breath * 0.5)),
                );
                let compression = flow
                    .convergence(position, 1.0)
                    .1
                    .max(flow.convergence(position, -1.0).1);
                let value = (broad * 0.46
                    + medium * 0.28
                    + fine * 0.12
                    + core_presence * (0.08 + health.pressure() * 0.04)
                    + compression * 0.06)
                    .clamp(0.0, 1.0);
                landscape.values[y * width + x] = value;
            }
        }

        landscape
    }

    fn sample(&self, position: (f32, f32)) -> f32 {
        let x = position.0.clamp(0.0, 1.0) * self.width.saturating_sub(1) as f32;
        let y = position.1.clamp(0.0, 1.0) * self.height.saturating_sub(1) as f32;
        let x0 = x.floor() as usize;
        let y0 = y.floor() as usize;
        let x1 = (x0 + 1).min(self.width - 1);
        let y1 = (y0 + 1).min(self.height - 1);
        let tx = x.fract();
        let ty = y.fract();
        let top = self.values[y0 * self.width + x0]
            + (self.values[y0 * self.width + x1] - self.values[y0 * self.width + x0]) * tx;
        let bottom = self.values[y1 * self.width + x0]
            + (self.values[y1 * self.width + x1] - self.values[y1 * self.width + x0]) * tx;
        top + (bottom - top) * ty
    }

    fn gradient(&self, position: (f32, f32)) -> (f32, f32) {
        let dx = 1.0 / self.width.saturating_sub(1).max(1) as f32;
        let dy = 1.0 / self.height.saturating_sub(1).max(1) as f32;
        (
            (self.sample((position.0 + dx, position.1))
                - self.sample((position.0 - dx, position.1)))
                * 0.5,
            (self.sample((position.0, position.1 + dy))
                - self.sample((position.0, position.1 - dy)))
                * 0.5,
        )
    }

    fn sampler_weight(&self, position: (f32, f32)) -> f32 {
        let value = self.sample(position);
        let basin = smoothstep((value - 0.30) / 0.44);
        let gradient_value = self.gradient(position);
        let gradient = gradient_value.0.hypot(gradient_value.1);
        let transition = smoothstep((gradient - 0.018) / 0.12);
        (0.38 + basin * 0.46 + transition * 0.10).clamp(0.0, 1.0)
    }

    fn apply_support(&self, field: &mut FieldFrame, health: PulseHealth) {
        let support_energy = 0.004 + health.density() * 0.004;
        for (index, sample) in field.samples.iter_mut().enumerate() {
            let x = index % self.width;
            let y = index / self.width;
            let position = (
                x as f32 / self.width.saturating_sub(1).max(1) as f32,
                y as f32 / self.height.saturating_sub(1).max(1) as f32,
            );
            let value = self.sample(position);
            let basin = smoothstep((value - 0.46) / 0.30);
            let crest = smoothstep((value - 0.64) / 0.22);
            let organization = basin * (0.56 + crest * 0.44);
            let landscape_modulation = (0.58 + basin * 0.48 + crest * 0.08).clamp(0.0, 1.16);
            sample.energy = (sample.energy * landscape_modulation + support_energy * organization)
                .clamp(0.0, 1.0);
        }
    }
}

fn potential_signal(value: f32) -> f32 {
    0.5 + (value * 0.34).tanh() * 0.5
}

fn warp_position(position: (f32, f32), frequency: f32, time: f32, amount: f32) -> (f32, f32) {
    (
        (position.0 + (position.1 * TAU * frequency + time * 0.09).sin() * amount)
            .clamp(-0.08, 1.08),
        (position.1 + (position.0 * TAU * frequency * 0.73 + time * 0.07).cos() * amount * 0.72)
            .clamp(-0.08, 1.08),
    )
}

pub(crate) fn draw(frame: &mut Frame, area: Rect, app: &AppState, theme: &Theme, focused: bool) {
    let pulse = app.pulse();
    let health = pulse.health();
    let title = format!(
        "System Pulse  {}  {:>3.0}%",
        health.state().label(),
        health.intensity() * 100.0
    );
    let lines = if uses_braille(area) {
        braille_lines(area, pulse, theme)
    } else {
        unicode_lines(area, pulse, theme)
    };
    let widget = Paragraph::new(lines)
        .style(Style::default().bg(theme.surface))
        .block(panel(theme, title, focused));
    frame.render_widget(widget, area);
}

fn uses_braille(area: Rect) -> bool {
    area.width >= 24 && area.height >= 7
}

fn braille_lines(area: Rect, pulse: &PulseModel, theme: &Theme) -> Vec<Line<'static>> {
    let width = usize::from(area.width.saturating_sub(2)).max(1);
    let height = usize::from(area.height.saturating_sub(2)).max(1);
    let field = field_for_area(width, height, true, pulse);
    let health = pulse.health();
    let lighting = field.lighting_range();

    (0..height)
        .map(|cell_y| {
            let mut spans = Vec::with_capacity(width);
            for cell_x in 0..width {
                let mut mask = 0_u8;
                let mut strongest = EnergySample::default();

                for (dot_x, dot_y, bit) in BRAILLE_DOTS {
                    let x = cell_x * 2 + usize::from(dot_x);
                    let y = cell_y * 4 + usize::from(dot_y);
                    let sample = field.sample(x, y);
                    if sample.energy > FIELD_THRESHOLD {
                        mask |= 1 << bit;
                    }
                    if sample.energy > strongest.energy {
                        strongest = sample;
                    }
                }

                let symbol = char::from_u32(0x2800 + u32::from(mask)).unwrap_or(' ');
                spans.push(Span::styled(
                    symbol.to_string(),
                    sample_style(theme, health, strongest, lighting),
                ));
            }
            Line::from(spans)
        })
        .collect()
}

fn unicode_lines(area: Rect, pulse: &PulseModel, theme: &Theme) -> Vec<Line<'static>> {
    let width = usize::from(area.width.saturating_sub(2)).max(1);
    let height = usize::from(area.height.saturating_sub(2)).max(1);
    let field = field_for_area(width, height, false, pulse);
    let health = pulse.health();
    let lighting = field.lighting_range();

    (0..height)
        .map(|cell_y| {
            let spans = (0..width)
                .map(|cell_x| {
                    let sample = field.sample(cell_x * 2 + 1, cell_y * 2 + 1);
                    let symbol = unicode_symbol(sample, lighting);
                    Span::styled(
                        symbol.to_string(),
                        sample_style(theme, health, sample, lighting),
                    )
                })
                .collect::<Vec<_>>();
            Line::from(spans)
        })
        .collect()
}

fn animation_time(pulse: &PulseModel) -> f32 {
    if pulse.reduced_motion() {
        0.0
    } else {
        pulse.time_seconds()
    }
}

fn field_for_area(width: usize, height: usize, braille: bool, pulse: &PulseModel) -> FieldFrame {
    let grid_width = width.saturating_mul(2);
    let grid_height = if braille {
        height.saturating_mul(4)
    } else {
        height.saturating_mul(2)
    };
    let preset = preset_for(width, height);
    build_field(
        grid_width.max(1),
        grid_height.max(1),
        braille,
        preset,
        pulse.health(),
        animation_time(pulse),
        pulse,
    )
}

fn core_state(time: f32, health: PulseHealth, pulse: &PulseModel) -> CoreState {
    let input = pulse.input();
    let motion = if pulse.reduced_motion() { 0.0 } else { 1.0 };
    let drift = time * 0.045;
    let breath = if pulse.reduced_motion() {
        0.5
    } else {
        pulse.breath()
    };
    let position = (
        (0.50 + drift.sin() * 0.055 * motion + (input.network - 0.30) * 0.018).clamp(0.34, 0.66),
        (0.48 + (drift * 0.73 + 0.8).cos() * 0.065 * motion + (input.load - 0.35) * 0.018)
            .clamp(0.32, 0.68),
    );

    CoreState {
        position,
        radius: (0.105 + input.memory * 0.035 + health.pressure() * 0.035 + breath * 0.012)
            .clamp(0.095, 0.19),
        pull: (0.17 + input.load * 0.08 + health.pressure() * 0.15 + breath * 0.018)
            .clamp(0.17, 0.42),
        reconnection: (0.34 + input.network * 0.18 + health.volatility() * 0.12).clamp(0.34, 0.64),
        compression: (0.64 + input.memory * 0.22 + health.pressure() * 0.15 + breath * 0.05)
            .clamp(0.64, 1.0),
        breath,
    }
}

fn preset_for(width: usize, height: usize) -> PulsePreset {
    if width <= 36 || height <= 9 {
        PulsePreset::Compact
    } else if width >= 120 && height >= 24 {
        PulsePreset::LargeWidescreen
    } else if width >= 70 && height >= 14 {
        PulsePreset::Widescreen
    } else {
        PulsePreset::Standard
    }
}

fn build_field(
    width: usize,
    height: usize,
    braille: bool,
    preset: PulsePreset,
    health: PulseHealth,
    time: f32,
    pulse: &PulseModel,
) -> FieldFrame {
    let mut field = FieldFrame::new(width, height, braille);
    let parameters = preset.parameters();
    let field_time =
        time + pulse.phase() * 0.025 + pulse.input().network * 0.018 + pulse.input().load * 0.012;
    let core = core_state(field_time, health, pulse);
    let aspect = domain_aspect(width, height, braille);
    let flow = FlowField::new(aspect, field_time, health, pulse, core);
    let landscape = EnergyLandscape::build(
        width,
        height,
        &flow,
        health,
        field_time,
        pulse.reduced_motion(),
    );
    let particle_count = active_particle_count(parameters.particles, health, pulse);

    for particle_index in 0..particle_count {
        let particle = particle_spec(particle_index, particle_count, parameters.margin);
        let lead_steps = if particle.edge || pulse.reduced_motion() {
            0
        } else {
            ((field_time * (0.65 + particle.depth * 0.55) * 15.0 + particle.phase * 11.0)
                .rem_euclid(92.0)) as usize
        };
        let mut position = advect_position(
            particle.position,
            particle.direction,
            lead_steps,
            &flow,
            parameters.step_size,
            parameters.virtual_margin,
        );
        let mut segment_start = position;

        for step in 0..parameters.advection_steps {
            let next_position = advect_position(
                position,
                particle.direction,
                1,
                &flow,
                parameters.step_size,
                parameters.virtual_margin,
            );
            if step % parameters.splat_stride == 0 || step + 1 == parameters.advection_steps {
                let (_, compression, reconnection) = flow.convergence(position, particle.direction);
                let sampler_weight = landscape.sampler_weight(position);
                let rhythm = 0.68
                    + (((field_time * 0.16 + step as f32 * 0.13 + particle.phase).sin() * 0.5)
                        + 0.5)
                        * 0.22;
                let presence =
                    (0.54 + compression * (0.92 + core.breath * 0.12) + reconnection * 0.18)
                        .clamp(0.0, 1.55);
                let edge_boost = if particle.edge { 2.40 } else { 1.0 };
                let depth_presence = 0.40 + particle.depth * particle.depth * 0.82;
                let energy =
                    (0.075 + health.density() * parameters.density * 0.12 + rhythm * 0.045)
                        * (0.86 + health.amplitude() * 0.14)
                        * presence
                        * depth_presence
                        * (0.72 + sampler_weight * 0.38)
                        * edge_boost;
                let glint = compression > 0.12 && hash01(particle_index * 97 + step * 13) > 0.985;
                let energy = if glint {
                    energy + 0.065 + compression * 0.12
                } else {
                    energy
                };
                let radius = parameters.splat_radius
                    + particle.depth * 0.16
                    + compression * 0.52
                    + sampler_weight * 0.12
                    + if particle.edge { 0.42 } else { 0.0 }
                    + if glint { 0.18 } else { 0.0 };
                let depth = (particle.depth * (0.46 + compression * 0.36 + reconnection * 0.18))
                    .clamp(0.0, 1.0);
                field.deposit_segment(
                    segment_start,
                    next_position,
                    energy,
                    radius,
                    particle.layer,
                    depth,
                );
                segment_start = next_position;
            }
            position = next_position;
        }
    }

    landscape.apply_support(&mut field, health);
    let diffusion_amount = 0.13 + health.density() * 0.07;
    field.diffuse(parameters.diffusion_passes, diffusion_amount);

    let convergence = detect_convergence(&field, &flow, health);
    illuminate_convergence(&mut field, &flow, convergence);
    field.diffuse(1, 0.07 + convergence.score * 0.05);
    apply_reconnection(&mut field, &flow, &landscape, convergence);
    apply_discharges(
        &mut field,
        &flow,
        convergence,
        health,
        field_time,
        pulse,
        parameters.step_size,
    );

    field
}

fn active_particle_count(base: usize, health: PulseHealth, pulse: &PulseModel) -> usize {
    let density_factor = 0.72 + health.density() * 0.28;
    let reduced_factor = if pulse.reduced_motion() { 0.68 } else { 1.0 };
    ((base as f32 * density_factor * reduced_factor).round() as usize).max(12)
}

fn particle_spec(index: usize, count: usize, margin: f32) -> Particle {
    let edge = index < count.min(16);
    let from_left = index.is_multiple_of(2);
    let x = if edge {
        if from_left {
            -0.045 - hash01(index * 19 + 3) * 0.020
        } else {
            1.045 + hash01(index * 23 + 5) * 0.020
        }
    } else {
        margin + hash01(index * 31 + 11) * (1.0 - margin * 2.0)
    };
    let y = (0.10 + hash01(index * 43 + 17) * 0.80 + (hash01(index * 59 + 29) - 0.5) * 0.08)
        .clamp(margin, 1.0 - margin);

    Particle {
        position: (x, y),
        phase: hash01(index * 71 + 13) * TAU,
        direction: if edge {
            if from_left { 1.0 } else { -1.0 }
        } else if index.is_multiple_of(3) {
            -1.0
        } else {
            1.0
        },
        depth: 0.14 + hash01(index * 67 + 23) * 0.82,
        layer: index % FLOW_LAYERS,
        edge,
    }
}

fn advect_position(
    mut position: (f32, f32),
    direction: f32,
    steps: usize,
    flow: &FlowField,
    step_size: f32,
    virtual_margin: f32,
) -> (f32, f32) {
    for _ in 0..steps {
        let velocity = flow.velocity(position);
        position.0 = wrap_virtual(
            position.0 + velocity.0 * step_size * direction,
            virtual_margin,
        );
        position.1 += velocity.1 * step_size * direction;
        position.1 = reflect(position.1, 0.025, 0.975);
    }
    position
}

#[derive(Debug, Clone, Copy)]
struct ConvergenceState {
    density: f32,
    score: f32,
}

fn detect_convergence(
    field: &FieldFrame,
    flow: &FlowField,
    health: PulseHealth,
) -> ConvergenceState {
    let density = field.local_energy(flow.core.position, 5.0);
    let field_score = smoothstep((density - 0.006) / 0.022);
    let pressure = smoothstep((health.pressure() - 0.25) / 0.55);
    ConvergenceState {
        density,
        score: (field_score * (0.72 + pressure * 0.28)).clamp(0.0, 1.0),
    }
}

fn illuminate_convergence(field: &mut FieldFrame, flow: &FlowField, convergence: ConvergenceState) {
    if convergence.score <= 0.0 {
        return;
    }

    let density_factor = 0.74 + smoothstep(convergence.density / 0.06) * 0.26;
    let lighting = field.lighting_range();
    let source = field.samples.clone();
    let mut bloom = vec![0.0; field.samples.len()];
    for y in 0..field.height {
        for x in 0..field.width {
            let index = y * field.width + x;
            let mut sample = source[index];
            if sample.energy <= ACTIVE_ENERGY_EPSILON {
                continue;
            }
            let position = (
                x as f32 / field.width.saturating_sub(1).max(1) as f32,
                y as f32 / field.height.saturating_sub(1).max(1) as f32,
            );
            let forward = flow.convergence(position, 1.0);
            let reverse = flow.convergence(position, -1.0);
            let proximity = forward.0.max(reverse.0);
            let local_convergence = forward.1.max(reverse.1);
            let local_reconnection = forward.2.max(reverse.2);
            let local_average = field.local_energy_from(&source, position, 2.2);
            let local_ratio = sample.energy / (local_average + ACTIVE_ENERGY_EPSILON * 0.8);
            let local_contrast = smoothstep((local_ratio - 0.95) / 1.25);
            let coherence = (local_convergence * 0.56
                + proximity * 0.18
                + local_reconnection * 0.08
                + local_contrast * 0.18)
                .clamp(0.0, 1.0);
            let gain = sample.energy
                * coherence
                * convergence.score
                * density_factor
                * (0.18 + local_contrast * 0.08);
            sample.energy = (sample.energy + gain).clamp(0.0, 1.0);
            sample.depth = (sample.depth
                + (local_convergence * 0.15 + local_contrast * 0.05) * convergence.score)
                .clamp(0.0, 1.0);
            field.samples[index] = sample;

            let peak_signal =
                smoothstep((sample.energy - lighting.peak) / (1.0 - lighting.peak).max(0.04));
            let bloom_amount = peak_signal * coherence * convergence.score * 0.024;
            if bloom_amount > 0.0 {
                if x > 0 {
                    bloom[index - 1] += bloom_amount * 0.24;
                }
                if x + 1 < field.width {
                    bloom[index + 1] += bloom_amount * 0.24;
                }
                if y > 0 {
                    bloom[index - field.width] += bloom_amount * 0.18;
                }
                if y + 1 < field.height {
                    bloom[index + field.width] += bloom_amount * 0.18;
                }
            }
        }
    }

    for (sample, contribution) in field.samples.iter_mut().zip(bloom) {
        sample.energy = (sample.energy + contribution.min(0.018)).clamp(0.0, 1.0);
    }
}

fn apply_reconnection(
    field: &mut FieldFrame,
    flow: &FlowField,
    landscape: &EnergyLandscape,
    convergence: ConvergenceState,
) {
    if convergence.score < 0.16 {
        return;
    }

    let source = field.samples.clone();
    for y in 0..field.height {
        for x in 0..field.width {
            let index = y * field.width + x;
            let mut sample = source[index];
            if sample.energy <= ACTIVE_ENERGY_EPSILON {
                continue;
            }
            let position = (
                x as f32 / field.width.saturating_sub(1).max(1) as f32,
                y as f32 / field.height.saturating_sub(1).max(1) as f32,
            );
            let forward = flow.convergence(position, 1.0);
            let reverse = flow.convergence(position, -1.0);
            let reconnection = forward.2.max(reverse.2);
            let basin = smoothstep((landscape.sample(position) - 0.34) / 0.42);
            let organization = (reconnection * (0.58 + basin * 0.42)).clamp(0.0, 1.0);
            let gain = sample.energy * organization * convergence.score * 0.075;
            sample.energy = (sample.energy + gain).clamp(0.0, 1.0);
            field.samples[index] = sample;
        }
    }
}

fn apply_discharges(
    field: &mut FieldFrame,
    flow: &FlowField,
    convergence: ConvergenceState,
    health: PulseHealth,
    time: f32,
    pulse: &PulseModel,
    step_size: f32,
) {
    if pulse.reduced_motion() || health.volatility() < 0.06 || convergence.score < 0.42 {
        return;
    }

    let volatility = smoothstep((health.volatility() - 0.06) / 0.34);
    let cadence = ((time * 1.12 + flow.core.breath * 1.7).sin() * 0.5) + 0.5;
    let burst = smoothstep((cadence - 0.78) / 0.22);
    let discharge = convergence.score * volatility * burst;
    if discharge < 0.10 {
        return;
    }

    for branch in 0..2 {
        let branch_angle = if branch == 0 { -0.48 } else { 0.48 };
        let mut position = flow.core.position;
        for step in 0..4 {
            let velocity = rotate(
                flow.velocity(position),
                branch_angle * (1.0 - step as f32 * 0.2),
            );
            position = (
                position.0 + velocity.0 * step_size * (1.15 + discharge * 0.25),
                position.1 + velocity.1 * step_size * (1.15 + discharge * 0.25),
            );
            field.deposit(
                position,
                discharge * (0.065 - step as f32 * 0.012),
                0.40,
                FLOW_LAYERS,
                1.0,
            );
        }
    }
}

fn rotate(vector: (f32, f32), angle: f32) -> (f32, f32) {
    let (sin, cos) = angle.sin_cos();
    (
        vector.0 * cos - vector.1 * sin,
        vector.0 * sin + vector.1 * cos,
    )
}

fn domain_aspect(width: usize, height: usize, braille: bool) -> f32 {
    let dot_aspect = if braille { 1.0 } else { 1.8 };
    (height as f32 / width.max(1) as f32 * dot_aspect).clamp(0.22, 3.5)
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

fn hash01(value: usize) -> f32 {
    let mut hash = value as u32;
    hash = hash.wrapping_mul(0x045d_9f3b);
    hash ^= hash >> 16;
    hash = hash.wrapping_mul(0x045d_9f3b);
    hash ^= hash >> 16;
    (hash as f32) / u32::MAX as f32
}

fn unicode_symbol(sample: EnergySample, lighting: LightingRange) -> char {
    let (distant, flow, energetic, peak) = lighting.signals(sample.energy);
    if peak > 0.72 {
        '•'
    } else if energetic > 0.52 {
        '∙'
    } else if flow > 0.24 || distant > 0.45 {
        '·'
    } else {
        ' '
    }
}

fn sample_style(
    theme: &Theme,
    health: PulseHealth,
    sample: EnergySample,
    lighting: LightingRange,
) -> Style {
    let (distant, flow, energetic, peak) = lighting.signals(sample.energy);
    let layer_tint = match sample.layer % FLOW_LAYERS {
        0 => theme.secondary,
        1 => theme.primary,
        2 => theme.pulse_medium,
        _ => theme.pulse_high,
    };
    let system_color = health_color(theme, health.intensity());
    let mut color = blend_color(theme.pulse_low, layer_tint, 0.18 + sample.depth * 0.20);
    color = blend_color(color, theme.pulse_high, flow * 0.72);
    color = blend_color(color, theme.pulse_medium, energetic * 0.52);
    color = blend_color(
        color,
        system_color,
        (0.10 + energetic * 0.24 + peak * 0.14).clamp(0.0, 0.62),
    );
    let warm_state = smoothstep((health.intensity() - 0.46) / 0.40);
    color = blend_color(color, theme.warning, warm_state * energetic * 0.42);
    color = blend_color(
        color,
        theme.danger,
        smoothstep((health.intensity() - 0.72) / 0.28) * (0.18 + energetic * 0.52),
    );
    color = blend_color(color, theme.text, peak * (0.42 + sample.depth * 0.42));

    let level_signal = distant * 0.08 + flow * 0.24 + energetic * 0.36 + peak * 0.78;
    let depth_signal = 0.38 + sample.depth * 0.62;
    let brightness = (0.028 + level_signal) * depth_signal;
    Style::default().fg(scale_color(color, brightness))
}

fn histogram_quantile<const N: usize>(histogram: &[usize; N], total: usize, quantile: f32) -> f32 {
    let target = ((total.saturating_sub(1)) as f32 * quantile.clamp(0.0, 1.0)).round() as usize;
    let mut accumulated = 0_usize;
    for (bucket, count) in histogram.iter().copied().enumerate() {
        accumulated += count;
        if accumulated > target {
            return (bucket as f32 + 0.5) / N as f32;
        }
    }
    1.0
}

fn health_color(theme: &Theme, intensity: f32) -> Color {
    let intensity = intensity.clamp(0.0, 1.0);
    let calm = blend_color(
        theme.pulse_high,
        theme.pulse_medium,
        smoothstep(intensity / 0.35),
    );
    let active = blend_color(calm, theme.primary, smoothstep((intensity - 0.20) / 0.30));
    let busy = blend_color(active, theme.warning, smoothstep((intensity - 0.48) / 0.25));
    blend_color(busy, theme.danger, smoothstep((intensity - 0.72) / 0.28))
}

fn smoothstep(value: f32) -> f32 {
    let value = value.clamp(0.0, 1.0);
    value * value * (3.0 - 2.0 * value)
}

fn blend_color(from: Color, to: Color, amount: f32) -> Color {
    let amount = amount.clamp(0.0, 1.0);
    match (from, to) {
        (Color::Rgb(fr, fg, fb), Color::Rgb(tr, tg, tb)) => Color::Rgb(
            interpolate_channel(fr, tr, amount),
            interpolate_channel(fg, tg, amount),
            interpolate_channel(fb, tb, amount),
        ),
        (_, target) => target,
    }
}

fn interpolate_channel(from: u8, to: u8, amount: f32) -> u8 {
    (from as f32 + (to as f32 - from as f32) * amount)
        .round()
        .clamp(0.0, 255.0) as u8
}

fn scale_color(color: Color, brightness: f32) -> Color {
    let brightness = brightness.clamp(0.0, 1.0);
    match color {
        Color::Rgb(red, green, blue) => Color::Rgb(
            (red as f32 * brightness).round() as u8,
            (green as f32 * brightness).round() as u8,
            (blue as f32 * brightness).round() as u8,
        ),
        other => other,
    }
}

#[cfg(test)]
mod tests {
    use std::collections::VecDeque;

    use ratatui::layout::Rect;

    use super::{
        ACTIVE_ENERGY_EPSILON, ConvergenceState, EnergyLandscape, EnergySample, FIELD_THRESHOLD,
        FieldFrame, FlowField, PulsePreset, apply_discharges, braille_lines, build_field,
        core_state, detect_convergence, domain_aspect, field_for_area, illuminate_convergence,
        preset_for, unicode_lines, uses_braille,
    };
    use crate::{
        pulse::{HealthState, PulseInput, PulseModel},
        theme::Theme,
    };

    fn pulse() -> PulseModel {
        pulse_with_input(PulseInput {
            cpu: 0.6,
            memory: 0.7,
            gpu: Some(0.4),
            network: 0.3,
            load: 0.5,
            volatility: 0.2,
        })
    }

    fn pulse_with_input(input: PulseInput) -> PulseModel {
        let mut pulse = PulseModel::new(false);
        pulse.set_input(input);
        pulse
    }

    #[test]
    fn selects_braille_for_a_usable_pulse_area() {
        assert!(uses_braille(Rect::new(0, 0, 32, 10)));
        assert!(!uses_braille(Rect::new(0, 0, 20, 6)));
    }

    #[test]
    fn selects_intentional_compositions_for_representative_areas() {
        assert_eq!(preset_for(30, 8), PulsePreset::Compact);
        assert_eq!(preset_for(50, 12), PulsePreset::Standard);
        assert_eq!(preset_for(80, 20), PulsePreset::Widescreen);
        assert_eq!(preset_for(140, 40), PulsePreset::LargeWidescreen);
    }

    #[test]
    fn energy_landscape_has_broad_medium_and_fine_structure() {
        let pulse = pulse();
        let health = pulse.health();
        let core = core_state(0.0, health, &pulse);
        let flow = FlowField::new(domain_aspect(160, 80, true), 0.0, health, &pulse, core);
        let landscape = EnergyLandscape::build(160, 80, &flow, health, 0.0, false);
        let minimum = landscape.values.iter().copied().fold(1.0, f32::min);
        let maximum = landscape.values.iter().copied().fold(0.0, f32::max);
        let fine_variation = landscape
            .values
            .chunks(landscape.width)
            .map(|row| {
                row.windows(2)
                    .map(|pair| (pair[1] - pair[0]).abs())
                    .sum::<f32>()
            })
            .sum::<f32>();
        let broad_variation = (0..landscape.height)
            .step_by(8)
            .flat_map(|y| (0..landscape.width).step_by(8).map(move |x| (x, y)))
            .filter(|&(x, y)| x + 8 < landscape.width && y + 8 < landscape.height)
            .map(|(x, y)| {
                let current = landscape.values[y * landscape.width + x];
                let next = landscape.values[(y + 8) * landscape.width + x + 8];
                (next - current).abs()
            })
            .sum::<f32>();

        assert!(maximum - minimum > 0.12);
        assert!(fine_variation > 1.0);
        assert!(broad_variation > 0.20);
    }

    #[test]
    fn energy_landscape_drifts_without_rebuilding_its_topology() {
        let pulse = pulse();
        let health = pulse.health();
        let core = core_state(0.0, health, &pulse);
        let flow = FlowField::new(domain_aspect(160, 80, true), 0.0, health, &pulse, core);
        let first = EnergyLandscape::build(160, 80, &flow, health, 0.0, false);
        let later = EnergyLandscape::build(160, 80, &flow, health, 0.05, false);
        let average_change = first
            .values
            .iter()
            .zip(&later.values)
            .map(|(first, later)| (first - later).abs())
            .sum::<f32>()
            / first.values.len() as f32;

        assert!(average_change > 0.0);
        assert!(average_change < 0.08);
    }

    #[test]
    fn landscape_support_forms_fused_energy_components() {
        let field = field_for_area(80, 20, true, &pulse());
        let components = active_components(&field);
        let active = field
            .samples
            .iter()
            .filter(|sample| sample.energy > FIELD_THRESHOLD)
            .count();
        let largest = components.iter().copied().max().unwrap_or(0);

        assert!(active > field.samples.len() / 8);
        assert!(largest > active / 8);
        assert!(components.len() < active / 3);
    }

    #[test]
    fn renders_both_terminal_native_field_variants() {
        let pulse = pulse();
        let theme = Theme::default();

        let braille = braille_lines(Rect::new(0, 0, 32, 10), &pulse, &theme);
        let unicode = unicode_lines(Rect::new(0, 0, 20, 6), &pulse, &theme);
        let field = field_for_area(28, 8, true, &pulse);
        assert!(!braille.is_empty());
        assert!(!unicode.is_empty());
        assert!(braille.iter().all(|line| line.spans.len() == 30));
        assert!(unicode.iter().all(|line| line.spans.len() == 18));
        assert!(
            field
                .samples
                .iter()
                .filter(|sample| sample.energy > 0.05)
                .count()
                > 32
        );
        assert!(braille.iter().any(|line| {
            line.spans.iter().any(|span| {
                span.content
                    .chars()
                    .any(|character| character != '\u{2800}')
            })
        }));
        assert!(unicode.iter().any(|line| {
            line.spans
                .iter()
                .any(|span| span.content.chars().any(|character| character != ' '))
        }));
    }

    #[test]
    fn field_is_deterministic_bounded_and_spatially_non_uniform() {
        let pulse = pulse();
        let first = field_for_area(80, 20, true, &pulse);
        let second = field_for_area(80, 20, true, &pulse);
        assert_eq!(first, second);
        assert!(first.samples.iter().any(|sample| sample.energy > 0.0));
        assert!(first.samples.iter().any(|sample| sample.energy > 0.35));
        assert!(first.samples.iter().any(|sample| sample.energy < 0.05));
        assert!(
            first
                .samples
                .iter()
                .all(|sample| (0.0..=1.0).contains(&sample.energy))
        );
    }

    #[test]
    fn energy_forms_coherent_filaments_with_intentional_silence() {
        let field = field_for_area(80, 20, true, &pulse());
        let active = field
            .samples
            .iter()
            .filter(|sample| sample.energy > FIELD_THRESHOLD)
            .count();
        let adjacent = (0..field.height)
            .flat_map(|y| (0..field.width).map(move |x| (x, y)))
            .filter(|&(x, y)| {
                let sample = field.sample(x, y);
                if sample.energy <= FIELD_THRESHOLD {
                    return false;
                }
                let right = x + 1 < field.width && field.sample(x + 1, y).energy > FIELD_THRESHOLD;
                let below = y + 1 < field.height && field.sample(x, y + 1).energy > FIELD_THRESHOLD;
                right || below
            })
            .count();
        let quiet = field
            .samples
            .iter()
            .filter(|sample| sample.energy < FIELD_THRESHOLD * 0.55)
            .count();
        let lighting = field.lighting_range();
        let peaks = field
            .samples
            .iter()
            .filter(|sample| sample.energy >= lighting.peak)
            .count();

        assert!(active > field.samples.len() / 8);
        assert!(adjacent * 2 > active);
        assert!(quiet > field.samples.len() / 4);
        assert!(peaks > 0);
        assert!(peaks * 12 < active);
        assert!(lighting.distant < lighting.flow);
        assert!(lighting.flow < lighting.energy);
        assert!(lighting.energy < lighting.peak);
    }

    #[test]
    fn adjacent_times_preserve_a_shared_energy_structure() {
        let pulse = pulse();
        let first = build_field(
            160,
            80,
            true,
            PulsePreset::Widescreen,
            pulse.health(),
            0.0,
            &pulse,
        );
        let next = build_field(
            160,
            80,
            true,
            PulsePreset::Widescreen,
            pulse.health(),
            0.05,
            &pulse,
        );
        let shared = first
            .samples
            .iter()
            .zip(&next.samples)
            .filter(|(first, next)| first.energy > FIELD_THRESHOLD && next.energy > FIELD_THRESHOLD)
            .count();
        let active = first
            .samples
            .iter()
            .filter(|sample| sample.energy > FIELD_THRESHOLD)
            .count();

        assert!(shared * 3 > active);
    }

    #[test]
    fn exposure_preserves_distant_midtones_and_rare_peaks() {
        let field = build_field(
            160,
            80,
            true,
            PulsePreset::Widescreen,
            pulse().health(),
            0.0,
            &pulse(),
        );
        let lighting = field.lighting_range();
        let mut distant = 0;
        let mut flow = 0;
        let mut energy = 0;
        let mut peaks = 0;
        let mut silence = 0;

        for sample in &field.samples {
            let (distant_signal, flow_signal, energy_signal, peak_signal) =
                lighting.signals(sample.energy);
            if sample.energy <= ACTIVE_ENERGY_EPSILON {
                silence += 1;
            } else {
                distant += usize::from(distant_signal > 0.18);
                flow += usize::from(flow_signal > 0.18);
                energy += usize::from(energy_signal > 0.18);
                peaks += usize::from(peak_signal > 0.80);
            }
        }
        assert!(silence > field.samples.len() / 5);
        assert!(distant > field.samples.len() / 30);
        assert!(flow > field.samples.len() / 30);
        assert!(energy > field.samples.len() / 80);
        assert!(peaks > 0);
        assert!(peaks * 8 < field.samples.len());
    }

    #[test]
    fn one_core_is_deterministic_slow_and_bounded() {
        let pulse = pulse();
        let health = pulse.health();
        let first = core_state(2.0, health, &pulse);
        let same = core_state(2.0, health, &pulse);
        let later = core_state(90.0, health, &pulse);

        assert_eq!(first, same);
        assert_ne!(first.position, later.position);
        assert!((0.34..=0.66).contains(&first.position.0));
        assert!((0.32..=0.68).contains(&first.position.1));
        assert!((0.095..=0.19).contains(&first.radius));

        let mut reduced = pulse;
        reduced.set_reduced_motion(true);
        assert_eq!(
            core_state(0.0, reduced.health(), &reduced),
            core_state(90.0, reduced.health(), &reduced)
        );
    }

    #[test]
    fn density_accumulates_around_the_single_core() {
        let pulse = pulse();
        let health = pulse.health();
        let field = build_field(240, 120, true, PulsePreset::Widescreen, health, 0.0, &pulse);
        let core = core_state(0.0, health, &pulse);
        let near = field.local_energy(core.position, 7.0);
        let distant = field.local_energy((0.10, 0.18), 7.0);
        let flow = FlowField::new(domain_aspect(240, 120, true), 0.0, health, &pulse, core);
        let convergence = detect_convergence(&field, &flow, health);
        assert!(
            near > distant,
            "core density {near} should exceed distant density {distant}"
        );
        assert!(convergence.density > distant);
        assert!(convergence.score > 0.0);
    }

    #[test]
    fn convergence_amplifies_existing_core_energy_and_local_contrast() {
        let pulse = pulse();
        let health = pulse.health();
        let core = core_state(0.0, health, &pulse);
        let flow = FlowField::new(domain_aspect(160, 80, true), 0.0, health, &pulse, core);
        let mut field = FieldFrame::new(160, 80, true);
        field.deposit(core.position, 0.34, 3.5, 0, 0.85);
        field.deposit((0.12, 0.18), 0.07, 2.0, 1, 0.20);
        let core_before = field.local_energy(core.position, 3.5);
        let distant_before = field.local_energy((0.12, 0.18), 2.0);

        let unchanged = field.clone();
        illuminate_convergence(
            &mut field,
            &flow,
            ConvergenceState {
                density: 0.34,
                score: 1.0,
            },
        );
        let core_after = field.local_energy(core.position, 3.5);
        let distant_after = field.local_energy((0.12, 0.18), 2.0);
        assert!(core_after > core_before);
        assert!(core_after - core_before > distant_after - distant_before);

        let mut disabled = unchanged.clone();
        illuminate_convergence(
            &mut disabled,
            &flow,
            ConvergenceState {
                density: 0.34,
                score: 0.0,
            },
        );
        assert_eq!(disabled, unchanged);
    }

    #[test]
    fn calm_stays_alive_and_busier_states_gain_energy_without_new_composition() {
        let calm = pulse_with_input(PulseInput {
            cpu: 0.12,
            memory: 0.20,
            gpu: None,
            network: 0.02,
            load: 0.10,
            volatility: 0.0,
        });
        let busy = pulse_with_input(PulseInput {
            cpu: 0.72,
            memory: 0.76,
            gpu: Some(0.45),
            network: 0.22,
            load: 0.62,
            volatility: 0.12,
        });
        let critical = pulse_with_input(PulseInput {
            cpu: 0.96,
            memory: 0.94,
            gpu: Some(0.92),
            network: 0.75,
            load: 0.96,
            volatility: 0.80,
        });

        assert_eq!(calm.health().state(), HealthState::Calm);
        assert_eq!(busy.health().state(), HealthState::Busy);
        assert_eq!(critical.health().state(), HealthState::Critical);

        let calm_field = field_for_area(80, 20, true, &calm);
        let busy_field = field_for_area(80, 20, true, &busy);
        let critical_field = field_for_area(80, 20, true, &critical);
        let calm_active = calm_field
            .samples
            .iter()
            .filter(|sample| sample.energy > FIELD_THRESHOLD)
            .count();
        let busy_active = busy_field
            .samples
            .iter()
            .filter(|sample| sample.energy > FIELD_THRESHOLD)
            .count();
        let critical_active = critical_field
            .samples
            .iter()
            .filter(|sample| sample.energy > FIELD_THRESHOLD)
            .count();

        assert!(calm_active > calm_field.samples.len() / 20);
        assert!(mean_energy(&busy_field) > mean_energy(&calm_field));
        assert!(mean_energy(&critical_field) > mean_energy(&calm_field));
        assert!(busy_active > 0 && critical_active > 0);
        assert!(
            calm_field
                .samples
                .iter()
                .any(|sample| sample.energy < FIELD_THRESHOLD)
        );
        assert!(
            critical_field
                .samples
                .iter()
                .any(|sample| sample.energy < FIELD_THRESHOLD)
        );
    }

    #[test]
    fn peak_energy_is_concentrated_near_the_single_core() {
        let pulse = pulse();
        let health = pulse.health();
        let field = build_field(240, 120, true, PulsePreset::Widescreen, health, 0.0, &pulse);
        let core = core_state(0.0, health, &pulse);
        let mut core_peak: f32 = 0.0;
        let mut distant_peak: f32 = 0.0;
        for (index, sample) in field.samples.iter().enumerate() {
            let x = index % field.width;
            let y = index / field.width;
            let position = (
                x as f32 / field.width.saturating_sub(1).max(1) as f32,
                y as f32 / field.height.saturating_sub(1).max(1) as f32,
            );
            let distance = (position.0 - core.position.0)
                .hypot((position.1 - core.position.1) * field.dot_aspect);
            if distance < core.radius * 2.5 {
                core_peak = core_peak.max(sample.energy);
            } else {
                distant_peak = distant_peak.max(sample.energy);
            }
        }
        assert!(core_peak > 0.0);
        assert!(core_peak > distant_peak);
    }

    #[test]
    fn energy_remains_present_before_and_after_reconnection() {
        let pulse = pulse();
        let health = pulse.health();
        let field = build_field(240, 120, true, PulsePreset::Widescreen, health, 0.0, &pulse);
        let core = core_state(0.0, health, &pulse);
        let before = field.local_energy((core.position.0 - 0.16, core.position.1), 12.0);
        let after = field.local_energy((core.position.0 + 0.16, core.position.1), 12.0);

        assert!(before > 0.0);
        assert!(after > 0.0);
    }

    #[test]
    fn discharges_require_density_volatility_and_motion() {
        let mut calm = PulseModel::new(false);
        calm.set_input(PulseInput {
            cpu: 0.15,
            memory: 0.20,
            gpu: None,
            network: 0.0,
            load: 0.10,
            volatility: 0.0,
        });
        let calm_core = core_state(0.0, calm.health(), &calm);
        let calm_flow = FlowField::new(
            domain_aspect(160, 80, true),
            0.0,
            calm.health(),
            &calm,
            calm_core,
        );
        let mut calm_field = FieldFrame::new(160, 80, true);
        calm_field.deposit(calm_core.position, 1.0, 8.0, 0, 1.0);
        let calm_before = calm_field.clone();
        apply_discharges(
            &mut calm_field,
            &calm_flow,
            ConvergenceState {
                density: 1.0,
                score: 1.0,
            },
            calm.health(),
            0.65,
            &calm,
            0.02,
        );
        assert_eq!(calm_field, calm_before);

        let mut active = PulseModel::new(false);
        active.set_input(PulseInput {
            cpu: 0.85,
            memory: 0.90,
            gpu: Some(0.80),
            network: 0.70,
            load: 0.85,
            volatility: 0.90,
        });
        let active_core = core_state(0.0, active.health(), &active);
        let active_flow = FlowField::new(
            domain_aspect(160, 80, true),
            0.0,
            active.health(),
            &active,
            active_core,
        );
        let mut active_field = FieldFrame::new(160, 80, true);
        active_field.deposit(active_core.position, 1.0, 8.0, 0, 1.0);
        let active_convergence = ConvergenceState {
            density: 1.0,
            score: 1.0,
        };
        let active_before = active_field.clone();
        apply_discharges(
            &mut active_field,
            &active_flow,
            active_convergence,
            active.health(),
            0.65,
            &active,
            0.02,
        );
        assert_ne!(active_field, active_before);

        let mut reduced = active;
        reduced.set_reduced_motion(true);
        let reduced_core = core_state(0.0, reduced.health(), &reduced);
        let reduced_flow = FlowField::new(
            domain_aspect(160, 80, true),
            0.0,
            reduced.health(),
            &reduced,
            reduced_core,
        );
        let reduced_before = active_field.clone();
        apply_discharges(
            &mut active_field,
            &reduced_flow,
            active_convergence,
            reduced.health(),
            0.65,
            &reduced,
            0.02,
        );
        assert_eq!(active_field, reduced_before);
    }

    #[test]
    fn virtual_domain_keeps_both_edges_connected_for_all_presets() {
        let pulse = pulse();
        let health = pulse.health();
        let presets = [
            PulsePreset::Compact,
            PulsePreset::Standard,
            PulsePreset::Widescreen,
            PulsePreset::LargeWidescreen,
        ];

        for preset in presets {
            let field = build_field(160, 80, true, preset, health, 0.0, &pulse);
            let left_edge = edge_energy(&field, true);
            let right_edge = edge_energy(&field, false);

            assert!(left_edge > 0, "left edge is empty for {preset:?}");
            assert!(right_edge > 0, "right edge is empty for {preset:?}");
            assert_eq!(field.samples.len(), field.width * field.height);
            assert!(
                field
                    .samples
                    .iter()
                    .all(|sample| (0.0..=1.0).contains(&sample.energy))
            );
        }
    }

    #[test]
    fn guarantees_real_edge_coverage_across_health_and_time() {
        let presets = [
            PulsePreset::Standard,
            PulsePreset::Widescreen,
            PulsePreset::LargeWidescreen,
        ];
        let inputs = [
            (
                PulseInput {
                    cpu: 0.05,
                    memory: 0.10,
                    gpu: None,
                    network: 0.0,
                    load: 0.05,
                    volatility: 0.0,
                },
                HealthState::Calm,
            ),
            (
                PulseInput {
                    cpu: 0.45,
                    memory: 0.50,
                    gpu: None,
                    network: 0.10,
                    load: 0.30,
                    volatility: 0.0,
                },
                HealthState::Active,
            ),
            (
                PulseInput {
                    cpu: 0.70,
                    memory: 0.70,
                    gpu: Some(0.30),
                    network: 0.20,
                    load: 0.60,
                    volatility: 0.0,
                },
                HealthState::Busy,
            ),
        ];

        for (input, expected_state) in inputs {
            let mut pulse = PulseModel::new(false);
            pulse.set_input(input);
            assert_eq!(pulse.health().state(), expected_state);

            for preset in presets {
                for time in [0.0, 0.7, 1.9] {
                    let field = build_field(160, 80, true, preset, pulse.health(), time, &pulse);
                    let left_column = column_energy(&field, 0);
                    let right_column = column_energy(&field, field.width - 1);
                    let left_band = edge_energy(&field, true);
                    let right_band = edge_energy(&field, false);

                    assert!(
                        left_column > 0,
                        "left column is empty for {preset:?} at {time}"
                    );
                    assert!(
                        right_column > 0,
                        "right column is empty for {preset:?} at {time}"
                    );
                    assert!(
                        left_band > 8,
                        "left band is too sparse for {preset:?} at {time}"
                    );
                    assert!(
                        right_band > 8,
                        "right band is too sparse for {preset:?} at {time}"
                    );
                }
            }
        }

        let mut reduced = PulseModel::new(true);
        reduced.set_input(inputs[1].0);
        for preset in presets {
            let field = build_field(160, 80, true, preset, reduced.health(), 0.0, &reduced);
            assert!(column_energy(&field, 0) > 0);
            assert!(column_energy(&field, field.width - 1) > 0);
        }
    }

    #[test]
    fn moving_field_changes_while_reduced_motion_is_stable() {
        let pulse = pulse();
        let preset = preset_for(80, 20);
        let health = pulse.health();
        let first = build_field(160, 80, true, preset, health, 0.0, &pulse);
        let later = build_field(160, 80, true, preset, health, 1.0, &pulse);
        assert_ne!(first, later);

        let mut reduced = pulse;
        reduced.set_reduced_motion(true);
        let reduced_first = field_for_area(80, 20, true, &reduced);
        let reduced_later = field_for_area(80, 20, true, &reduced);
        assert_eq!(reduced_first, reduced_later);
    }

    #[test]
    fn energy_samples_have_a_safe_default() {
        assert_eq!(EnergySample::default().energy, 0.0);
    }

    #[test]
    fn rasterizes_segments_between_particle_positions() {
        let mut field = super::FieldFrame::new(48, 24, true);
        field.deposit_segment((0.18, 0.5), (0.38, 0.5), 0.5, 1.0, 0, 0.5);
        assert!(
            field
                .samples
                .iter()
                .filter(|sample| sample.energy > 0.05)
                .count()
                > 3
        );
    }

    fn mean_energy(field: &FieldFrame) -> f32 {
        field
            .samples
            .iter()
            .map(|sample| sample.energy)
            .sum::<f32>()
            / field.samples.len() as f32
    }

    fn active_components(field: &FieldFrame) -> Vec<usize> {
        let mut visited = vec![false; field.samples.len()];
        let mut components = Vec::new();

        for start in 0..field.samples.len() {
            if visited[start] || field.samples[start].energy <= FIELD_THRESHOLD {
                continue;
            }
            let mut queue = VecDeque::from([start]);
            visited[start] = true;
            let mut size = 0;

            while let Some(index) = queue.pop_front() {
                size += 1;
                let x = index % field.width;
                let y = index / field.width;
                for neighbor_y in y.saturating_sub(1)..=(y + 1).min(field.height - 1) {
                    for neighbor_x in x.saturating_sub(1)..=(x + 1).min(field.width - 1) {
                        let neighbor = neighbor_y * field.width + neighbor_x;
                        if !visited[neighbor] && field.samples[neighbor].energy > FIELD_THRESHOLD {
                            visited[neighbor] = true;
                            queue.push_back(neighbor);
                        }
                    }
                }
            }

            components.push(size);
        }

        components
    }

    fn edge_energy(field: &FieldFrame, left: bool) -> usize {
        field
            .samples
            .chunks(field.width)
            .map(|row| {
                let edge = if left {
                    &row[..4.min(row.len())]
                } else {
                    &row[row.len().saturating_sub(4)..]
                };
                edge.iter()
                    .filter(|sample| sample.energy > FIELD_THRESHOLD)
                    .count()
            })
            .sum()
    }

    fn column_energy(field: &FieldFrame, column: usize) -> usize {
        field
            .samples
            .chunks(field.width)
            .filter(|row| row[column].energy > FIELD_THRESHOLD)
            .count()
    }
}
