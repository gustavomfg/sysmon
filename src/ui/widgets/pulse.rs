use std::{
    cell::RefCell,
    time::{Duration, Instant},
};

use crate::{
    app::AppState,
    pulse::{PulseHealth, PulseInput, PulseModel},
    theme::Theme,
    ui::renderer::panel,
};

use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Modifier, Style},
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
const ATMOSPHERE_FLOOR: f32 = 0.30;
const ATMOSPHERE_CEILING: f32 = 0.045;
const RETURN_FLOOR: f32 = 0.64;
const RETURN_CEILING: f32 = 0.96;
const ATMOSPHERE_LAYER_BASE: usize = FLOW_LAYERS;
const RETURN_LAYER_BASE: usize = FLOW_LAYERS * 2;
const AMBIENT_DUST_LAYER: usize = FLOW_LAYERS * 3;
const AMBIENT_TRACER_LAYER: usize = AMBIENT_DUST_LAYER + 1;
const AMBIENT_GLINT_LAYER: usize = AMBIENT_DUST_LAYER + 2;
const PULSE_FIELD_BUDGET: Duration = Duration::from_millis(12);
const PULSE_FIELD_RECOVERY_BUDGET: Duration = Duration::from_millis(6);

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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct LayoutKey {
    width: usize,
    height: usize,
    braille: bool,
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct PulseStateKey {
    input: PulseInput,
    reduced_motion: bool,
    render_revision: u64,
    render_time: u32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct FieldCacheKey {
    layout: LayoutKey,
    preset: PulsePreset,
    state: PulseStateKey,
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct RasterCacheKey {
    field: FieldCacheKey,
    theme: Theme,
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
        if energy <= FIELD_THRESHOLD {
            return (0.0, 0.0, 0.0, 0.0);
        }

        let distant = smoothstep(
            (energy - ACTIVE_ENERGY_EPSILON) / (self.distant - ACTIVE_ENERGY_EPSILON).max(0.018),
        );
        let flow = smoothstep((energy - self.distant) / (self.flow - self.distant).max(0.025));
        let energetic = smoothstep((energy - self.flow) / (self.energy - self.flow).max(0.04));
        let peak = smoothstep((energy - self.peak) / (1.0 - self.peak).max(0.04));
        (distant.max(0.16), flow, energetic, peak)
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

struct PulseRenderCache {
    layout: Option<LayoutKey>,
    selected_preset: Option<PulsePreset>,
    last_adaptation_revision: Option<u64>,
    field_key: Option<FieldCacheKey>,
    field: Option<FieldFrame>,
    raster_key: Option<RasterCacheKey>,
    raster: Option<Vec<Line<'static>>>,
    last_field_cost: Duration,
}

impl PulseRenderCache {
    const fn new() -> Self {
        Self {
            layout: None,
            selected_preset: None,
            last_adaptation_revision: None,
            field_key: None,
            field: None,
            raster_key: None,
            raster: None,
            last_field_cost: Duration::ZERO,
        }
    }

    fn ensure_field(&mut self, width: usize, height: usize, braille: bool, pulse: &PulseModel) {
        let layout = LayoutKey {
            width,
            height,
            braille,
        };
        let area_width = width.saturating_div(2).max(1);
        let area_height = if braille {
            height.saturating_div(4).max(1)
        } else {
            height.saturating_div(2).max(1)
        };
        let base_preset = preset_for(area_width, area_height);
        let state = PulseStateKey {
            input: pulse.input(),
            reduced_motion: pulse.reduced_motion(),
            render_revision: pulse.render_revision(),
            render_time: pulse.render_time_seconds().to_bits(),
        };
        if self.layout != Some(layout) {
            self.layout = Some(layout);
            self.selected_preset = Some(base_preset);
            self.last_adaptation_revision = Some(state.render_revision);
            self.last_field_cost = Duration::ZERO;
            self.field_key = None;
            self.field = None;
            self.raster_key = None;
            self.raster = None;
        } else if self.last_adaptation_revision != Some(state.render_revision) {
            self.last_adaptation_revision = Some(state.render_revision);
            let selected_preset = self.selected_preset.unwrap_or(base_preset);
            self.selected_preset = Some(adaptive_preset(
                selected_preset,
                base_preset,
                self.last_field_cost,
            ));
        } else if self.selected_preset.is_none() {
            self.selected_preset = Some(base_preset);
        }

        let preset = self.selected_preset.unwrap_or(base_preset);
        let key = FieldCacheKey {
            layout,
            preset,
            state,
        };
        if self.field_key == Some(key) {
            return;
        }

        let started = Instant::now();
        let field = build_field(
            width,
            height,
            braille,
            preset,
            pulse.health(),
            animation_time(pulse),
            pulse,
        );
        self.last_field_cost = started.elapsed();
        self.field_key = Some(key);
        self.field = Some(field);
        self.raster_key = None;
        self.raster = None;
    }
}

thread_local! {
    static PULSE_RENDER_CACHE: RefCell<PulseRenderCache> =
        const { RefCell::new(PulseRenderCache::new()) };
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FilamentClass {
    Primary,
    Secondary,
    Ghost,
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct FilamentProfile {
    class: FilamentClass,
    radius_scale: f32,
}

fn filament_profile(particle: Particle, progress: f32, time: f32) -> FilamentProfile {
    let class = if particle.depth >= 0.68 {
        FilamentClass::Primary
    } else if particle.depth >= 0.38 {
        FilamentClass::Secondary
    } else {
        FilamentClass::Ghost
    };
    let progress = progress.clamp(0.0, 1.0);
    let phase = particle.phase
        + time * (0.035 + particle.depth * 0.025)
        + progress * TAU * (0.28 + particle.depth * 0.16)
        + particle.layer as f32 * 0.37;
    let broad_variation = phase.sin();
    let fine_variation = (phase * 0.71 + particle.phase * 0.23).cos();
    let radius_variation =
        (0.995 + broad_variation * 0.006 + fine_variation * 0.004).clamp(0.985, 1.005);
    let class_radius = match class {
        FilamentClass::Primary => 1.04,
        FilamentClass::Secondary => 1.00,
        FilamentClass::Ghost => 0.96,
    };

    FilamentProfile {
        class,
        radius_scale: (class_radius * radius_variation).clamp(0.68, 1.18),
    }
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

#[derive(Debug, Clone, Copy)]
struct CompressionZone {
    center: (f32, f32),
    entry_x: f32,
    exit_x: f32,
    fusion_half_width: f32,
    skew: f32,
}

impl CompressionZone {
    fn from(flow: &FlowField, health: PulseHealth) -> Self {
        let pressure = health.pressure();
        let center = (
            (flow.core.position.0 + 0.026 + flow.network_bias * 0.014).clamp(0.34, 0.68),
            (flow.core.position.1 - 0.018 + flow.network_bias * 0.010).clamp(0.30, 0.70),
        );
        let entry_span = (0.25 + flow.core.radius * 0.85 + pressure * 0.035).clamp(0.25, 0.38);
        let exit_span = (0.23 + flow.core.radius * 0.72 + pressure * 0.030).clamp(0.23, 0.36);

        Self {
            center,
            entry_x: (center.0 - entry_span).clamp(0.12, 0.46),
            exit_x: (center.0 + exit_span).clamp(0.54, 0.88),
            fusion_half_width: (0.065 + flow.core.radius * 0.22 + pressure * 0.014)
                .clamp(0.065, 0.11),
            skew: 0.058 + pressure * 0.024 + flow.network_bias.abs() * 0.012,
        }
    }

    fn project(&self, position: (f32, f32), phase: f32, layer: usize, depth: f32) -> (f32, f32) {
        if position.0 < self.entry_x || position.0 > self.exit_x {
            return position;
        }

        let incoming =
            smoothstep((position.0 - self.entry_x) / (self.center.0 - self.entry_x).max(0.001));
        let outgoing =
            smoothstep((position.0 - self.center.0) / (self.exit_x - self.center.0).max(0.001));
        let strand = ((phase * 0.61 + layer as f32 * 1.17).sin() * 0.72
            + (phase * 1.31 + depth * TAU).cos() * 0.28)
            .clamp(-1.0, 1.0);
        let centerline = if position.0 <= self.center.0 {
            self.center.1 + self.skew * (1.0 - incoming)
        } else {
            self.center.1 + self.skew * 0.55 * outgoing
        };
        let width_factor = if position.0 <= self.center.0 {
            1.0 - incoming * 0.78
        } else {
            0.22 + outgoing * 0.78
        };
        let strand_offset = strand * (0.009 + depth * 0.014 * outgoing);
        let mut y = centerline + (position.1 - centerline) * width_factor + strand_offset;

        let fusion = 1.0
            - smoothstep(((position.0 - self.center.0).abs() / self.fusion_half_width).max(0.0));
        let fused_center = self.center.1 + strand * 0.008;
        y = y + (fused_center - y) * fusion * 0.46;
        (position.0, y.clamp(-0.06, 1.06))
    }
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
    let grid_width = width.saturating_mul(2).max(1);
    let grid_height = height.saturating_mul(4).max(1);
    let health = pulse.health();
    cached_raster(grid_width, grid_height, true, pulse, *theme, |field| {
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
                        sample_style(theme, health, strongest, lighting, mask.count_ones()),
                    ));
                }
                Line::from(spans)
            })
            .collect()
    })
}

fn unicode_lines(area: Rect, pulse: &PulseModel, theme: &Theme) -> Vec<Line<'static>> {
    let width = usize::from(area.width.saturating_sub(2)).max(1);
    let height = usize::from(area.height.saturating_sub(2)).max(1);
    let grid_width = width.saturating_mul(2).max(1);
    let grid_height = height.saturating_mul(2).max(1);
    let health = pulse.health();
    cached_raster(grid_width, grid_height, false, pulse, *theme, |field| {
        let lighting = field.lighting_range();

        (0..height)
            .map(|cell_y| {
                let spans = (0..width)
                    .map(|cell_x| {
                        let sample = field.sample(cell_x * 2 + 1, cell_y * 2 + 1);
                        let symbol = unicode_symbol(sample, lighting);
                        Span::styled(
                            symbol.to_string(),
                            sample_style(theme, health, sample, lighting, 1),
                        )
                    })
                    .collect::<Vec<_>>();
                Line::from(spans)
            })
            .collect()
    })
}

fn animation_time(pulse: &PulseModel) -> f32 {
    if pulse.reduced_motion() {
        0.0
    } else {
        pulse.render_time_seconds()
    }
}

#[cfg(test)]
fn field_for_area(width: usize, height: usize, braille: bool, pulse: &PulseModel) -> FieldFrame {
    let grid_width = width.saturating_mul(2);
    let grid_height = if braille {
        height.saturating_mul(4)
    } else {
        height.saturating_mul(2)
    };
    with_cached_field(
        grid_width.max(1),
        grid_height.max(1),
        braille,
        pulse,
        |field| field.clone(),
    )
}

#[cfg(test)]
fn with_cached_field<T>(
    width: usize,
    height: usize,
    braille: bool,
    pulse: &PulseModel,
    render: impl FnOnce(&FieldFrame) -> T,
) -> T {
    PULSE_RENDER_CACHE.with(|cache| {
        let mut cache = cache.borrow_mut();
        cache.ensure_field(width, height, braille, pulse);
        render(
            cache
                .field
                .as_ref()
                .expect("pulse field cache must contain the ensured field"),
        )
    })
}

fn cached_raster(
    width: usize,
    height: usize,
    braille: bool,
    pulse: &PulseModel,
    theme: Theme,
    render: impl FnOnce(&FieldFrame) -> Vec<Line<'static>>,
) -> Vec<Line<'static>> {
    PULSE_RENDER_CACHE.with(|cache| {
        let mut cache = cache.borrow_mut();
        cache.ensure_field(width, height, braille, pulse);
        let Some(field_key) = cache.field_key else {
            return Vec::new();
        };
        let raster_key = RasterCacheKey {
            field: field_key,
            theme,
        };
        if cache.raster_key == Some(raster_key) {
            return cache.raster.clone().unwrap_or_default();
        }

        let lines = render(
            cache
                .field
                .as_ref()
                .expect("pulse field cache must contain the ensured field"),
        );
        cache.raster_key = Some(raster_key);
        cache.raster = Some(lines.clone());
        lines
    })
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

fn adaptive_preset(
    selected: PulsePreset,
    base: PulsePreset,
    last_field_cost: Duration,
) -> PulsePreset {
    if last_field_cost > PULSE_FIELD_BUDGET {
        preset_at_rank(preset_rank(selected).saturating_sub(1))
    } else if last_field_cost <= PULSE_FIELD_RECOVERY_BUDGET {
        preset_at_rank((preset_rank(selected) + 1).min(preset_rank(base)))
    } else {
        selected
    }
}

fn preset_rank(preset: PulsePreset) -> u8 {
    match preset {
        PulsePreset::Compact => 0,
        PulsePreset::Standard => 1,
        PulsePreset::Widescreen => 2,
        PulsePreset::LargeWidescreen => 3,
    }
}

fn preset_at_rank(rank: u8) -> PulsePreset {
    match rank {
        0 => PulsePreset::Compact,
        1 => PulsePreset::Standard,
        2 => PulsePreset::Widescreen,
        _ => PulsePreset::LargeWidescreen,
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
    let compression_zone = CompressionZone::from(&flow, health);
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
                let progress =
                    step as f32 / parameters.advection_steps.saturating_sub(1).max(1) as f32;
                let morphology = filament_profile(particle, progress, field_time);
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
                let radius = (parameters.splat_radius
                    + particle.depth * 0.16
                    + compression * 0.52
                    + sampler_weight * 0.12
                    + if particle.edge { 0.42 } else { 0.0 }
                    + if glint { 0.18 } else { 0.0 })
                    * morphology.radius_scale;
                let depth = (particle.depth * (0.46 + compression * 0.36 + reconnection * 0.18))
                    .clamp(0.0, 1.0);
                let visual_start = compression_zone.project(
                    segment_start,
                    particle.phase,
                    particle.layer,
                    particle.depth,
                );
                let visual_end = compression_zone.project(
                    next_position,
                    particle.phase,
                    particle.layer,
                    particle.depth,
                );
                field.deposit_segment(
                    visual_start,
                    visual_end,
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
    let atmosphere = build_atmospheric_layer(&field, preset, health, field_time, pulse, &flow);
    merge_atmospheric_layer(&mut field, &atmosphere);
    let return_layer = build_return_layer(&field, preset, health, field_time, pulse, &flow);
    merge_return_layer(&mut field, &return_layer);
    let atmospheric_particles =
        build_atmospheric_particles(&field, preset, health, field_time, pulse, &flow);
    rasterize_atmospheric_particles(&mut field, &atmospheric_particles);

    field
}

fn build_atmospheric_layer(
    field: &FieldFrame,
    preset: PulsePreset,
    health: PulseHealth,
    time: f32,
    pulse: &PulseModel,
    flow: &FlowField,
) -> FieldFrame {
    let mut atmosphere = FieldFrame::new(field.width, field.height, field.dot_aspect == 1.0);
    let filament_count = match preset {
        PulsePreset::Compact => 1,
        PulsePreset::Standard => 2,
        PulsePreset::Widescreen => 3,
        PulsePreset::LargeWidescreen => 4,
    };
    let steps = (field.width as f32 * 1.20).round().clamp(40.0, 220.0) as usize;
    let motion = if pulse.reduced_motion() { 0.0 } else { 1.0 };
    let energy = 0.13 + health.density() * 0.015;

    for filament in 0..filament_count {
        let phase = hash01(filament * 113 + 41) * TAU;
        let depth = 0.035 + hash01(filament * 127 + 53) * 0.11;
        let mut position = (
            -0.14 - hash01(filament * 139 + 67) * 0.06,
            (ATMOSPHERE_CEILING
                + 0.13
                + hash01(filament * 149 + 79) * 0.11
                + (time * 0.008 * motion + phase).sin() * 0.008 * motion)
                .clamp(ATMOSPHERE_CEILING, ATMOSPHERE_FLOOR - 0.015),
        );
        let mut segment_start = position;

        for step in 0..steps {
            let progress = step as f32 / steps as f32;
            let velocity = flow.velocity(position);
            let slow_phase = phase + time * 0.008 * motion + progress * TAU * 0.34;
            let lateral_drift = (slow_phase + velocity.1 * 0.9).sin() * 0.0016 * motion;
            let next_position = (
                position.0 + (1.26 / steps as f32) * (0.88 + velocity.0 * 0.12),
                (position.1 + velocity.1 * 0.004 * motion + lateral_drift)
                    .clamp(ATMOSPHERE_CEILING, ATMOSPHERE_FLOOR - 0.015),
            );
            let local_energy = energy * (0.86 + hash01(filament * 173 + step * 7) * 0.16);
            let radius = 0.62 + depth * 0.26;
            atmosphere.deposit_segment(
                segment_start,
                next_position,
                local_energy,
                radius,
                filament,
                depth,
            );
            segment_start = next_position;
            position = next_position;
        }
    }

    atmosphere.diffuse(1, 0.10);
    atmosphere
}

fn merge_atmospheric_layer(field: &mut FieldFrame, atmosphere: &FieldFrame) {
    let upper_height = ((field.height as f32 * ATMOSPHERE_FLOOR).ceil() as usize).min(field.height);
    for y in 0..upper_height {
        for x in 0..field.width {
            let index = y * field.width + x;
            let distant = atmosphere.samples[index];
            if distant.energy <= 0.0 {
                continue;
            }

            let contribution = (distant.energy * 0.86).clamp(0.0, 0.13);
            let sample = &mut field.samples[index];
            let existing_energy = sample.energy;
            sample.energy = (existing_energy + contribution).clamp(0.0, 1.0);
            if distant.energy > existing_energy {
                sample.layer = ATMOSPHERE_LAYER_BASE + distant.layer % FLOW_LAYERS;
                sample.depth = distant.depth;
            }
        }
    }
}

fn build_return_layer(
    field: &FieldFrame,
    preset: PulsePreset,
    health: PulseHealth,
    time: f32,
    pulse: &PulseModel,
    flow: &FlowField,
) -> FieldFrame {
    let mut return_layer = FieldFrame::new(field.width, field.height, field.dot_aspect == 1.0);
    let branch_count = match preset {
        PulsePreset::Compact => 1,
        PulsePreset::Standard => 2,
        PulsePreset::Widescreen => 2,
        PulsePreset::LargeWidescreen => 3,
    };
    let steps = (field.width as f32 * 1.35).round().clamp(48.0, 240.0) as usize;
    let motion = if pulse.reduced_motion() { 0.0 } else { 1.0 };
    let energy = 0.078 + health.density() * 0.010;
    let outlet = flow.velocity((flow.core.position.0 + 0.08, flow.core.position.1));
    let origin_y =
        (flow.core.position.1 + 0.17 + outlet.1 * 0.035).clamp(RETURN_FLOOR + 0.02, 0.82);

    for branch in 0..branch_count {
        let phase = hash01(branch * 181 + 97) * TAU;
        let depth = 0.050 + hash01(branch * 193 + 109) * 0.095;
        let direction = if branch % 3 == 0 { -1.0 } else { 1.0 };
        let lateral_offset = (hash01(branch * 211 + 127) - 0.5) * 0.12;
        let mut position = (
            flow.core.position.0 + outlet.0 * 0.06 + direction * 0.028,
            (origin_y + lateral_offset).clamp(RETURN_FLOOR, RETURN_CEILING - 0.025),
        );
        let mut segment_start = position;

        for step in 0..steps {
            let progress = step as f32 / steps as f32;
            let velocity = flow.velocity(position);
            let slow_phase = phase + time * 0.006 * motion + progress * TAU * 0.18;
            let broad_wave = (slow_phase + velocity.0 * 0.35).sin() * 0.0028 * motion;
            let next_position = (
                position.0 + direction * (1.30 / steps as f32) * (0.86 + velocity.0.abs() * 0.06),
                (position.1
                    + velocity.1 * 0.0025 * motion
                    + broad_wave
                    + (origin_y - position.1) * 0.004)
                    .clamp(RETURN_FLOOR, RETURN_CEILING - 0.025),
            );
            let local_energy = energy * (0.84 + hash01(branch * 227 + step * 11) * 0.12);
            let radius = 0.72 + depth * 0.22;
            return_layer.deposit_segment(
                segment_start,
                next_position,
                local_energy,
                radius,
                branch,
                depth,
            );
            segment_start = next_position;
            position = next_position;
        }
    }

    return_layer.diffuse(1, 0.08);
    return_layer
}

fn merge_return_layer(field: &mut FieldFrame, return_layer: &FieldFrame) {
    let lower_height = ((field.height as f32 * RETURN_FLOOR).floor() as usize).min(field.height);
    for y in lower_height..field.height {
        for x in 0..field.width {
            let index = y * field.width + x;
            let distant = return_layer.samples[index];
            if distant.energy <= 0.0 {
                continue;
            }

            let contribution = (distant.energy * 0.74).clamp(0.0, 0.08);
            let sample = &mut field.samples[index];
            let existing_energy = sample.energy;
            sample.energy = (existing_energy + contribution).clamp(0.0, 1.0);
            if distant.energy > existing_energy {
                sample.layer = RETURN_LAYER_BASE + distant.layer % FLOW_LAYERS;
                sample.depth = distant.depth;
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AtmosphericParticleKind {
    Dust,
    Tracer,
    Glint,
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct AtmosphericParticle {
    position: (f32, f32),
    energy: f32,
    radius: f32,
    depth: f32,
    kind: AtmosphericParticleKind,
}

fn atmospheric_particle_budget(preset: PulsePreset) -> (usize, usize, usize) {
    match preset {
        PulsePreset::Compact => (5, 1, 0),
        PulsePreset::Standard => (8, 2, 1),
        PulsePreset::Widescreen => (12, 3, 3),
        PulsePreset::LargeWidescreen => (16, 4, 5),
    }
}

fn build_atmospheric_particles(
    field: &FieldFrame,
    preset: PulsePreset,
    health: PulseHealth,
    time: f32,
    pulse: &PulseModel,
    flow: &FlowField,
) -> Vec<AtmosphericParticle> {
    let (dust_count, tracer_count, glint_count) = atmospheric_particle_budget(preset);
    let capacity = dust_count + tracer_count + glint_count;
    let mut particles = Vec::with_capacity(capacity);
    let motion = if pulse.reduced_motion() { 0.0 } else { 1.0 };
    let core = flow.core.position;

    for index in 0..dust_count {
        let phase = hash01(index * 271 + 31) * TAU;
        let cluster = index / 3;
        let cluster_band = cluster % 4;
        let cluster_anchor = match cluster_band {
            0 => (
                0.12 + hash01(cluster * 401 + 139) * 0.76,
                0.11 + hash01(cluster * 409 + 149) * 0.12,
            ),
            1 => (
                0.12 + hash01(cluster * 419 + 157) * 0.76,
                0.78 + hash01(cluster * 421 + 163) * 0.12,
            ),
            2 => (
                (core.0 + (hash01(cluster * 431 + 167) - 0.5) * 0.18).clamp(0.16, 0.84),
                (core.1 + (hash01(cluster * 433 + 173) - 0.5) * 0.18).clamp(0.34, 0.66),
            ),
            _ => (
                0.18 + hash01(cluster * 439 + 179) * 0.64,
                0.67 + hash01(cluster * 443 + 181) * 0.12,
            ),
        };
        let cluster_spread_x = 0.045 + hash01(cluster * 449 + 191) * 0.025;
        let cluster_spread_y = 0.026 + hash01(cluster * 457 + 193) * 0.018;
        let seed = (
            (cluster_anchor.0 + (hash01(index * 277 + 43) - 0.5) * cluster_spread_x)
                .clamp(0.08, 0.92),
            (cluster_anchor.1 + (hash01(index * 281 + 47) - 0.5) * cluster_spread_y)
                .clamp(0.055, 0.945),
        );
        let drift = time * 0.006 * motion + phase;
        let position = (
            (seed.0 + drift.sin() * 0.018).clamp(0.02, 0.98),
            (seed.1 + (drift * 0.73).cos() * 0.012 * motion).clamp(0.035, 0.965),
        );
        particles.push(AtmosphericParticle {
            position,
            energy: 0.086 + hash01(index * 293 + 61) * 0.032,
            radius: 0.28 + hash01(index * 307 + 67) * 0.12,
            depth: 0.018 + hash01(index * 311 + 71) * 0.075,
            kind: AtmosphericParticleKind::Dust,
        });
    }

    for index in 0..tracer_count {
        let seed = (
            0.10 + hash01(index * 331 + 79) * 0.80,
            0.18 + hash01(index * 337 + 83) * 0.64,
        );
        let anchor = atmospheric_energy_anchor(field, seed, index);
        let position = advect_atmospheric_position(
            anchor,
            if index.is_multiple_of(2) { 1.0 } else { -1.0 },
            time,
            0.007 + hash01(index * 347 + 89) * 0.003,
            flow,
            0.06,
            motion,
        );
        let local_energy = field.local_energy(position, 2.4);
        particles.push(AtmosphericParticle {
            position,
            energy: 0.125 + local_energy.clamp(0.0, 0.28) * 0.060,
            radius: 0.38 + hash01(index * 353 + 97) * 0.12,
            depth: 0.14 + hash01(index * 359 + 101) * 0.15,
            kind: AtmosphericParticleKind::Tracer,
        });
    }

    for index in 0..glint_count {
        let phase = hash01(index * 367 + 103) * TAU;
        let angle = phase + health.volatility() * 0.35;
        let radius = flow.core.radius * (1.15 + hash01(index * 373 + 107) * 0.85);
        let seed = (
            (core.0 + angle.cos() * radius).clamp(0.04, 0.96),
            (core.1 + angle.sin() * radius * 0.72).clamp(0.06, 0.94),
        );
        if field.local_energy(seed, 2.6) <= FIELD_THRESHOLD {
            continue;
        }
        let position = advect_atmospheric_position(
            seed,
            if index.is_multiple_of(2) { 1.0 } else { -1.0 },
            time,
            0.0015,
            flow,
            0.04,
            motion,
        );
        let local_energy = field.local_energy(position, 2.6);
        if local_energy <= FIELD_THRESHOLD {
            continue;
        }
        particles.push(AtmosphericParticle {
            position,
            energy: 0.255 + local_energy.clamp(0.0, 0.40) * 0.14,
            radius: 0.52 + hash01(index * 379 + 109) * 0.16,
            depth: 0.62 + hash01(index * 383 + 113) * 0.22,
            kind: AtmosphericParticleKind::Glint,
        });
    }

    particles
}

fn atmospheric_energy_anchor(field: &FieldFrame, seed: (f32, f32), index: usize) -> (f32, f32) {
    let mut best = seed;
    let mut best_energy = field.local_energy(seed, 2.4);
    for candidate in 0..7 {
        let candidate_position = (
            (seed.0 + (hash01(index * 389 + candidate * 17 + 127) - 0.5) * 0.30)
                .clamp(0.035, 0.965),
            (seed.1 + (hash01(index * 397 + candidate * 19 + 131) - 0.5) * 0.24)
                .clamp(0.055, 0.945),
        );
        let candidate_energy = field.local_energy(candidate_position, 2.4);
        if candidate_energy > best_energy {
            best = candidate_position;
            best_energy = candidate_energy;
        }
    }
    best
}

fn advect_atmospheric_position(
    mut position: (f32, f32),
    direction: f32,
    time: f32,
    speed: f32,
    flow: &FlowField,
    virtual_margin: f32,
    motion: f32,
) -> (f32, f32) {
    let distance = time.max(0.0) * speed * motion;
    let integration_steps = 8;
    for _ in 0..integration_steps {
        let velocity = flow.velocity(position);
        position.0 = wrap_virtual(
            position.0 + velocity.0 * distance / integration_steps as f32 * direction,
            virtual_margin,
        );
        position.1 += velocity.1 * distance / integration_steps as f32 * direction;
        position.1 = reflect(position.1, 0.035, 0.965);
    }
    position
}

fn rasterize_atmospheric_particles(field: &mut FieldFrame, particles: &[AtmosphericParticle]) {
    for particle in particles {
        let layer = match particle.kind {
            AtmosphericParticleKind::Dust => AMBIENT_DUST_LAYER,
            AtmosphericParticleKind::Tracer => AMBIENT_TRACER_LAYER,
            AtmosphericParticleKind::Glint => AMBIENT_GLINT_LAYER,
        };
        field.deposit(
            particle.position,
            particle.energy,
            particle.radius,
            layer,
            particle.depth,
        );
    }
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
    let (_distant, _flow, energetic, peak) = lighting.signals(sample.energy);
    if peak > 0.72 {
        '•'
    } else if energetic > 0.52 {
        '∙'
    } else if sample.energy > FIELD_THRESHOLD {
        '·'
    } else {
        ' '
    }
}

fn perceptual_intensity(energy: f32, lighting: LightingRange, dot_count: u32) -> f32 {
    if energy <= FIELD_THRESHOLD {
        return 0.0;
    }

    let (distant, flow, energetic, peak) = lighting.signals(energy);
    let range = (lighting.peak - lighting.distant).max(0.16);
    let normalized = ((energy - lighting.distant) / range).clamp(0.0, 1.0);
    let gamma_lift = normalized.powf(0.64);
    let sparse_cell_lift = if dot_count <= 2 { 0.055 } else { 0.0 };
    let intensity = 0.30
        + distant * 0.06
        + gamma_lift * 0.20
        + flow * 0.19
        + energetic * 0.25
        + peak * 0.34
        + sparse_cell_lift;
    intensity.clamp(0.0, 0.96)
}

fn hierarchy_luminance(sample: EnergySample) -> f32 {
    let depth = sample.depth.clamp(0.0, 1.0);
    if sample.layer >= AMBIENT_GLINT_LAYER {
        return (0.88 + depth * 0.08).clamp(0.0, 1.0);
    }
    if sample.layer >= AMBIENT_TRACER_LAYER {
        return (0.70 + depth * 0.10).clamp(0.0, 1.0);
    }
    if sample.layer >= AMBIENT_DUST_LAYER {
        return (0.52 + depth * 0.08).clamp(0.0, 1.0);
    }
    if sample.layer >= RETURN_LAYER_BASE {
        return (0.70 + depth * 0.12).clamp(0.0, 1.0);
    }
    if sample.layer >= ATMOSPHERE_LAYER_BASE {
        return (0.56 + depth * 0.10).clamp(0.0, 1.0);
    }

    // Main-field depth is produced by compression, convergence and reconnection.
    // It therefore supplies the natural lift that guides the eye to the core.
    (0.86 + depth * 0.14).clamp(0.0, 1.0)
}

fn sample_style(
    theme: &Theme,
    health: PulseHealth,
    sample: EnergySample,
    lighting: LightingRange,
    dot_count: u32,
) -> Style {
    let (_distant, flow, energetic, peak) = lighting.signals(sample.energy);
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

    let intensity = perceptual_intensity(sample.energy, lighting, dot_count);
    let color_lift = (0.035 + flow * 0.055 + energetic * 0.12 + peak * 0.24).clamp(0.0, 0.42);
    color = blend_color(color, theme.text, color_lift);
    let brightness = (intensity * hierarchy_luminance(sample)).clamp(0.0, 0.96);
    let mut style = Style::default().fg(scale_color(color, brightness));
    if peak > 0.76 || (energetic > 0.82 && dot_count >= 3) {
        style = style.add_modifier(Modifier::BOLD);
    }
    style
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

    use ratatui::{layout::Rect, style::Modifier};

    use super::{
        ACTIVE_ENERGY_EPSILON, AMBIENT_DUST_LAYER, AMBIENT_GLINT_LAYER, ATMOSPHERE_FLOOR,
        ATMOSPHERE_LAYER_BASE, AtmosphericParticleKind, CompressionZone, ConvergenceState,
        EnergyLandscape, EnergySample, FIELD_THRESHOLD, FieldFrame, FilamentClass, FlowField,
        LightingRange, Particle, PulsePreset, RETURN_FLOOR, RETURN_LAYER_BASE, apply_discharges,
        atmospheric_particle_budget, braille_lines, build_atmospheric_layer,
        build_atmospheric_particles, build_field, build_return_layer, core_state,
        detect_convergence, domain_aspect, field_for_area, filament_profile, hierarchy_luminance,
        illuminate_convergence, perceptual_intensity, preset_for, rasterize_atmospheric_particles,
        sample_style, unicode_lines, unicode_symbol, uses_braille,
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
    fn filament_morphology_has_stable_tiers_and_continuous_variation() {
        let particle = |depth| Particle {
            position: (0.24, 0.42),
            phase: 0.7,
            direction: 1.0,
            depth,
            layer: 1,
            edge: false,
        };
        let primary = filament_profile(particle(0.86), 0.42, 0.0);
        let secondary = filament_profile(particle(0.50), 0.42, 0.0);
        let ghost = filament_profile(particle(0.20), 0.42, 0.0);

        assert_eq!(primary.class, FilamentClass::Primary);
        assert_eq!(secondary.class, FilamentClass::Secondary);
        assert_eq!(ghost.class, FilamentClass::Ghost);
        assert!(primary.radius_scale > secondary.radius_scale);
        assert!(secondary.radius_scale > ghost.radius_scale);

        for depth in [0.20, 0.50, 0.86] {
            let particle = particle(depth);
            let previous = filament_profile(particle, 0.42, 0.0);
            let next = filament_profile(particle, 0.43, 0.0);
            assert!((previous.radius_scale - next.radius_scale).abs() < 0.02);
            assert!((0.68..=1.18).contains(&next.radius_scale));
        }

        let pulse = pulse();
        let first = field_for_area(80, 20, true, &pulse);
        let same = field_for_area(80, 20, true, &pulse);
        assert_eq!(first, same);
        assert!(
            first
                .samples
                .iter()
                .all(|sample| { (0.0..=1.0).contains(&sample.energy) && sample.depth.is_finite() })
        );
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
    fn lighting_hierarchy_keeps_the_core_above_ambient_planes() {
        let energy = 0.34;
        let upper = hierarchy_luminance(EnergySample {
            energy,
            layer: ATMOSPHERE_LAYER_BASE,
            depth: 0.10,
        });
        let lower = hierarchy_luminance(EnergySample {
            energy,
            layer: RETURN_LAYER_BASE,
            depth: 0.12,
        });
        let main = hierarchy_luminance(EnergySample {
            energy,
            layer: 0,
            depth: 0.16,
        });
        let compression = hierarchy_luminance(EnergySample {
            energy,
            layer: 0,
            depth: 0.82,
        });
        let dust = hierarchy_luminance(EnergySample {
            energy,
            layer: AMBIENT_DUST_LAYER,
            depth: 0.06,
        });
        let glint = hierarchy_luminance(EnergySample {
            energy,
            layer: AMBIENT_GLINT_LAYER,
            depth: 0.80,
        });

        assert!(upper < lower);
        assert!(lower < main);
        assert!(main < compression);
        assert!(dust < main);
        assert!(glint < compression);

        let pulse = pulse();
        let health = pulse.health();
        let field = build_field(240, 120, true, PulsePreset::Widescreen, health, 0.0, &pulse);
        let core = core_state(0.0, health, &pulse);
        let lighting = field.lighting_range();
        let core_peak = field
            .samples
            .iter()
            .enumerate()
            .filter(|(index, _)| {
                let x = *index % field.width;
                let y = *index / field.width;
                let position = (
                    x as f32 / field.width.saturating_sub(1).max(1) as f32,
                    y as f32 / field.height.saturating_sub(1).max(1) as f32,
                );
                (position.0 - core.position.0)
                    .hypot((position.1 - core.position.1) * field.dot_aspect)
                    < core.radius * 2.5
            })
            .map(|(_, sample)| {
                perceptual_intensity(sample.energy, lighting, 8) * hierarchy_luminance(*sample)
            })
            .fold(0.0, f32::max);
        let ambient_peak = field
            .samples
            .iter()
            .filter(|sample| sample.layer >= ATMOSPHERE_LAYER_BASE)
            .map(|sample| {
                perceptual_intensity(sample.energy, lighting, 8) * hierarchy_luminance(*sample)
            })
            .fold(0.0, f32::max);

        assert!(core_peak > ambient_peak);
    }

    #[test]
    fn calm_lighting_recovers_midtone_visibility_without_lighting_silence() {
        let calm = pulse_with_input(PulseInput {
            cpu: 0.12,
            memory: 0.20,
            gpu: None,
            network: 0.02,
            load: 0.10,
            volatility: 0.0,
        });
        let field = field_for_area(80, 20, true, &calm);
        let lighting = field.lighting_range();
        let active = field
            .samples
            .iter()
            .filter(|sample| sample.energy > FIELD_THRESHOLD)
            .map(|sample| perceptual_intensity(sample.energy, lighting, 8))
            .collect::<Vec<_>>();
        let minimum = active.iter().copied().fold(1.0, f32::min);
        let average = active.iter().sum::<f32>() / active.len().max(1) as f32;
        let midtones = active
            .iter()
            .filter(|intensity| (0.30..=0.72).contains(*intensity))
            .count();
        let silence = field
            .samples
            .iter()
            .filter(|sample| perceptual_intensity(sample.energy, lighting, 8) == 0.0)
            .count();

        assert!(minimum >= 0.21);
        assert!(average > 0.26);
        assert!(midtones > active.len() / 10);
        assert!(silence > field.samples.len() / 5);
    }

    #[test]
    fn brighter_states_gain_visual_energy_without_losing_headroom() {
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
        let calm_field = field_for_area(80, 20, true, &calm);
        let busy_field = field_for_area(80, 20, true, &busy);
        let critical_field = field_for_area(80, 20, true, &critical);
        let calm_mean = mean_perceptual_intensity(&calm_field);
        let busy_mean = mean_perceptual_intensity(&busy_field);
        let critical_mean = mean_perceptual_intensity(&critical_field);
        let busy_max = max_perceptual_intensity(&busy_field);
        let critical_max = max_perceptual_intensity(&critical_field);

        assert!(busy_mean > calm_mean);
        assert!(critical_mean > calm_mean);
        assert!(busy_max <= 0.96);
        assert!(critical_max <= 0.96);
        assert!(rare_peak_count(&critical_field) < critical_field.samples.len() / 5);
    }

    #[test]
    fn sparse_braille_cells_receive_visibility_without_becoming_peaks() {
        let theme = Theme::default();
        let lighting = LightingRange {
            distant: 0.12,
            flow: 0.24,
            energy: 0.42,
            peak: 0.76,
        };
        let sample = EnergySample {
            energy: 0.16,
            layer: 0,
            depth: 0.25,
        };
        let sparse = perceptual_intensity(sample.energy, lighting, 1);
        let dense = perceptual_intensity(sample.energy, lighting, 8);
        let sparse_style = sample_style(&theme, pulse().health(), sample, lighting, 1);
        let peak_style = sample_style(
            &theme,
            pulse().health(),
            EnergySample {
                energy: 0.92,
                ..sample
            },
            lighting,
            4,
        );

        assert!(sparse > dense);
        assert!(sparse > 0.20);
        assert!(sparse_style.fg.is_some());
        assert!(peak_style.add_modifier.contains(Modifier::BOLD));
        assert_eq!(unicode_symbol(EnergySample::default(), lighting), ' ');
        assert_eq!(unicode_symbol(sample, lighting), '·');
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
    fn compression_zone_reduces_spread_then_reopens_it() {
        let pulse = pulse();
        let health = pulse.health();
        let core = core_state(0.0, health, &pulse);
        let flow = FlowField::new(domain_aspect(240, 120, true), 0.0, health, &pulse, core);
        let zone = CompressionZone::from(&flow, health);
        let y_values = [0.20, 0.34, 0.48, 0.62, 0.78];
        let incoming_x = zone.entry_x + (zone.center.0 - zone.entry_x) * 0.72;
        let fused_x = zone.center.0;
        let outgoing_x = zone.center.0 + (zone.exit_x - zone.center.0) * 0.72;
        let spread = |x: f32| {
            let projected = y_values
                .iter()
                .map(|y| zone.project((x, *y), 1.2, 1, 0.6))
                .map(|position| position.1);
            let (minimum, maximum) =
                projected.fold((1.0_f32, 0.0_f32), |(min, max), y| (min.min(y), max.max(y)));
            maximum - minimum
        };

        assert!(zone.exit_x - zone.entry_x > zone.fusion_half_width * 4.0);
        assert!(zone.center.0 > 0.5);
        assert!(spread(fused_x) < spread(incoming_x) * 0.45);
        assert!(spread(outgoing_x) > spread(fused_x) * 2.0);
    }

    #[test]
    fn compression_forms_a_continuous_bridge_and_reorients_the_exit() {
        let pulse = pulse();
        let health = pulse.health();
        let field = build_field(240, 120, true, PulsePreset::Widescreen, health, 0.0, &pulse);
        let core = core_state(0.0, health, &pulse);
        let flow = FlowField::new(domain_aspect(240, 120, true), 0.0, health, &pulse, core);
        let zone = CompressionZone::from(&flow, health);
        let samples = [
            zone.center.0 - zone.fusion_half_width * 2.0,
            zone.center.0 - zone.fusion_half_width,
            zone.center.0,
            zone.center.0 + zone.fusion_half_width,
        ];
        let bridge = samples
            .iter()
            .map(|x| field.local_energy((*x, zone.center.1), 4.5))
            .collect::<Vec<_>>();
        assert!(bridge.iter().all(|energy| *energy > FIELD_THRESHOLD));
        assert!(bridge[2] >= bridge[0] * 0.82);
        assert!(bridge[2] >= bridge[3] * 0.82);

        let incoming_a = zone.project((zone.entry_x + 0.06, zone.center.1 + 0.16), 0.9, 0, 0.35);
        let incoming_b = zone.project((zone.entry_x + 0.16, zone.center.1 + 0.16), 0.9, 0, 0.35);
        let outgoing_a = zone.project((zone.center.0 + 0.05, zone.center.1 + 0.16), 0.9, 0, 0.35);
        let outgoing_b = zone.project((zone.center.0 + 0.15, zone.center.1 + 0.16), 0.9, 0, 0.35);
        let incoming_slope = (incoming_b.1 - incoming_a.1) / (incoming_b.0 - incoming_a.0);
        let outgoing_slope = (outgoing_b.1 - outgoing_a.1) / (outgoing_b.0 - outgoing_a.0);
        assert!(incoming_slope * outgoing_slope < 0.0);
        assert!(field.local_energy((zone.center.0 + 0.15, zone.center.1), 5.0) > 0.0);
    }

    #[test]
    fn distant_atmosphere_is_sparse_upper_and_subordinate() {
        let pulse = pulse();
        let health = pulse.health();
        let presets = [
            PulsePreset::Compact,
            PulsePreset::Standard,
            PulsePreset::Widescreen,
            PulsePreset::LargeWidescreen,
        ];
        let base = FieldFrame::new(160, 80, true);

        for preset in presets {
            let core = core_state(0.0, health, &pulse);
            let flow = FlowField::new(domain_aspect(160, 80, true), 0.0, health, &pulse, core);
            let atmosphere = build_atmospheric_layer(&base, preset, health, 0.0, &pulse, &flow);
            let active = atmosphere
                .samples
                .iter()
                .enumerate()
                .filter(|(_, sample)| sample.energy > FIELD_THRESHOLD)
                .collect::<Vec<_>>();

            let maximum = atmosphere
                .samples
                .iter()
                .map(|sample| sample.energy)
                .fold(0.0, f32::max);
            assert!(
                !active.is_empty(),
                "missing distant layer for {preset:?}, maximum={maximum}"
            );
            assert!(active.len() < atmosphere.samples.len() / 5);
            assert!(
                active
                    .iter()
                    .all(|(index, _)| index / atmosphere.width < atmosphere.height / 2)
            );
            assert!(active.iter().all(|(index, _)| index / atmosphere.width
                < (atmosphere.height as f32 * ATMOSPHERE_FLOOR) as usize + 2));

            let atmospheric_max = active
                .iter()
                .map(|(_, sample)| sample.energy)
                .fold(0.0, f32::max);
            let main = build_field(160, 80, true, preset, health, 0.0, &pulse);
            let core_peak = main
                .samples
                .iter()
                .enumerate()
                .filter(|(index, _)| {
                    let x = index % main.width;
                    let y = index / main.width;
                    let position = (x as f32 / main.width as f32, y as f32 / main.height as f32);
                    (position.0 - core.position.0)
                        .hypot((position.1 - core.position.1) * main.dot_aspect)
                        < core.radius * 2.5
                })
                .map(|(_, sample)| sample.energy)
                .fold(0.0, f32::max);
            assert!(atmospheric_max < core_peak);
            assert!(
                atmosphere
                    .samples
                    .iter()
                    .all(|sample| (0.0..=1.0).contains(&sample.energy))
            );
        }
    }

    #[test]
    fn distant_atmosphere_is_deterministic_and_reduced_motion_is_static() {
        let pulse = pulse();
        let health = pulse.health();
        let base = FieldFrame::new(160, 80, true);
        let core = core_state(0.0, health, &pulse);
        let flow = FlowField::new(domain_aspect(160, 80, true), 0.0, health, &pulse, core);
        let first =
            build_atmospheric_layer(&base, PulsePreset::Widescreen, health, 0.0, &pulse, &flow);
        let same =
            build_atmospheric_layer(&base, PulsePreset::Widescreen, health, 0.0, &pulse, &flow);
        assert_eq!(first, same);

        let mut reduced = pulse;
        reduced.set_reduced_motion(true);
        let reduced_health = reduced.health();
        let reduced_core = core_state(0.0, reduced_health, &reduced);
        let reduced_flow = FlowField::new(
            domain_aspect(160, 80, true),
            0.0,
            reduced_health,
            &reduced,
            reduced_core,
        );
        let reduced_first = build_atmospheric_layer(
            &base,
            PulsePreset::Widescreen,
            reduced_health,
            0.0,
            &reduced,
            &reduced_flow,
        );
        let reduced_later = build_atmospheric_layer(
            &base,
            PulsePreset::Widescreen,
            reduced_health,
            10.0,
            &reduced,
            &reduced_flow,
        );
        assert_eq!(reduced_first, reduced_later);
    }

    #[test]
    fn return_layer_is_deep_long_and_subordinate() {
        let pulse = pulse();
        let health = pulse.health();
        let presets = [
            PulsePreset::Compact,
            PulsePreset::Standard,
            PulsePreset::Widescreen,
            PulsePreset::LargeWidescreen,
        ];
        let base = FieldFrame::new(160, 80, true);

        for preset in presets {
            let core = core_state(0.0, health, &pulse);
            let flow = FlowField::new(domain_aspect(160, 80, true), 0.0, health, &pulse, core);
            let return_layer = build_return_layer(&base, preset, health, 0.0, &pulse, &flow);
            let lower_start = (return_layer.height as f32 * RETURN_FLOOR).floor() as usize;
            let active = return_layer
                .samples
                .iter()
                .enumerate()
                .filter(|(index, sample)| {
                    index / return_layer.width >= lower_start && sample.energy > FIELD_THRESHOLD
                })
                .collect::<Vec<_>>();

            assert!(!active.is_empty(), "missing return plane for {preset:?}");
            assert!(active.len() < return_layer.samples.len() / 5);
            assert!(
                active
                    .iter()
                    .all(|(index, _)| { index / return_layer.width >= return_layer.height / 2 })
            );

            let columns = active
                .iter()
                .map(|(index, _)| index % return_layer.width)
                .collect::<std::collections::HashSet<_>>();
            assert!(columns.len() > return_layer.width / 4);

            let maximum = active
                .iter()
                .map(|(_, sample)| sample.energy)
                .fold(0.0, f32::max);
            assert!(maximum < 0.30);
            assert!(
                return_layer
                    .samples
                    .iter()
                    .all(|sample| (0.0..=1.0).contains(&sample.energy))
            );
        }

        let core = core_state(0.0, health, &pulse);
        let flow = FlowField::new(domain_aspect(160, 80, true), 0.0, health, &pulse, core);
        let return_layer =
            build_return_layer(&base, PulsePreset::Widescreen, health, 0.0, &pulse, &flow);
        let left = return_layer
            .samples
            .chunks(return_layer.width)
            .skip((return_layer.height as f32 * RETURN_FLOOR).floor() as usize)
            .map(|row| row[0].energy)
            .sum::<f32>();
        let right = return_layer
            .samples
            .chunks(return_layer.width)
            .skip((return_layer.height as f32 * RETURN_FLOOR).floor() as usize)
            .map(|row| row[return_layer.width - 1].energy)
            .sum::<f32>();
        assert!(left > 0.0);
        assert!(right > 0.0);
    }

    #[test]
    fn return_layer_is_deterministic_and_reduced_motion_is_static() {
        let pulse = pulse();
        let health = pulse.health();
        let base = FieldFrame::new(160, 80, true);
        let core = core_state(0.0, health, &pulse);
        let flow = FlowField::new(domain_aspect(160, 80, true), 0.0, health, &pulse, core);
        let first = build_return_layer(&base, PulsePreset::Widescreen, health, 0.0, &pulse, &flow);
        let same = build_return_layer(&base, PulsePreset::Widescreen, health, 0.0, &pulse, &flow);
        assert_eq!(first, same);

        let mut reduced = pulse;
        reduced.set_reduced_motion(true);
        let reduced_health = reduced.health();
        let reduced_core = core_state(0.0, reduced_health, &reduced);
        let reduced_flow = FlowField::new(
            domain_aspect(160, 80, true),
            0.0,
            reduced_health,
            &reduced,
            reduced_core,
        );
        let reduced_first = build_return_layer(
            &base,
            PulsePreset::Widescreen,
            reduced_health,
            0.0,
            &reduced,
            &reduced_flow,
        );
        let reduced_later = build_return_layer(
            &base,
            PulsePreset::Widescreen,
            reduced_health,
            10.0,
            &reduced,
            &reduced_flow,
        );
        assert_eq!(reduced_first, reduced_later);
    }

    #[test]
    fn atmospheric_particles_are_sparse_layered_and_bounded() {
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
            let core = core_state(0.0, health, &pulse);
            let flow = FlowField::new(domain_aspect(160, 80, true), 0.0, health, &pulse, core);
            let particles = build_atmospheric_particles(&field, preset, health, 0.0, &pulse, &flow);
            let (dust_limit, tracer_limit, glint_limit) = atmospheric_particle_budget(preset);
            let dust = particles
                .iter()
                .filter(|particle| particle.kind == AtmosphericParticleKind::Dust)
                .collect::<Vec<_>>();
            let tracers = particles
                .iter()
                .filter(|particle| particle.kind == AtmosphericParticleKind::Tracer)
                .collect::<Vec<_>>();
            let glints = particles
                .iter()
                .filter(|particle| particle.kind == AtmosphericParticleKind::Glint)
                .collect::<Vec<_>>();

            assert!(!particles.is_empty());
            assert_eq!(dust.len(), dust_limit);
            assert_eq!(tracers.len(), tracer_limit);
            assert!(glints.len() <= glint_limit);
            assert!(particles.len() <= dust_limit + tracer_limit + glint_limit);
            assert!(particles.iter().all(|particle| {
                particle.position.0.is_finite()
                    && particle.position.1.is_finite()
                    && (0.0..=1.0).contains(&particle.position.0)
                    && (0.0..=1.0).contains(&particle.position.1)
                    && (0.0..=1.0).contains(&particle.energy)
                    && particle.radius.is_finite()
                    && particle.depth.is_finite()
            }));
            assert!(dust.iter().any(|particle| particle.position.1 < 0.34));
            assert!(dust.iter().any(|particle| particle.position.1 > 0.66));
            if matches!(
                preset,
                PulsePreset::Widescreen | PulsePreset::LargeWidescreen
            ) {
                assert!((2..=6).contains(&glints.len()));
            }
            assert!(
                tracers
                    .iter()
                    .all(|particle| field.local_energy(particle.position, 2.4) > FIELD_THRESHOLD)
            );
            assert!(
                glints
                    .iter()
                    .all(|particle| field.local_energy(particle.position, 2.6) > FIELD_THRESHOLD)
            );

            let mut rasterized = FieldFrame::new(160, 80, true);
            rasterize_atmospheric_particles(&mut rasterized, &particles);
            assert_eq!(rasterized.samples.len(), 160 * 80);
            assert!(rasterized.samples.iter().all(|sample| {
                (0.0..=1.0).contains(&sample.energy) && sample.depth.is_finite()
            }));
            if dust.len() >= 3 {
                let first_cluster = &dust[..3];
                assert!(first_cluster.windows(2).all(|pair| {
                    let dx = pair[0].position.0 - pair[1].position.0;
                    let dy = pair[0].position.1 - pair[1].position.1;
                    dx.hypot(dy) < 0.16
                }));
            }
        }
    }

    #[test]
    fn atmospheric_particles_are_deterministic_and_respect_reduced_motion() {
        let pulse = pulse();
        let health = pulse.health();
        let preset = PulsePreset::Widescreen;
        let field = build_field(160, 80, true, preset, health, 0.0, &pulse);
        let core = core_state(0.0, health, &pulse);
        let flow = FlowField::new(domain_aspect(160, 80, true), 0.0, health, &pulse, core);
        let first = build_atmospheric_particles(&field, preset, health, 0.0, &pulse, &flow);
        let same = build_atmospheric_particles(&field, preset, health, 0.0, &pulse, &flow);
        let later = build_atmospheric_particles(&field, preset, health, 1.0, &pulse, &flow);
        assert_eq!(first, same);
        assert!(
            first
                .iter()
                .zip(&later)
                .any(|(first, later)| first.position != later.position)
        );

        let mut reduced = pulse;
        reduced.set_reduced_motion(true);
        let reduced_health = reduced.health();
        let reduced_field = build_field(160, 80, true, preset, reduced_health, 0.0, &reduced);
        let reduced_core = core_state(0.0, reduced_health, &reduced);
        let reduced_flow = FlowField::new(
            domain_aspect(160, 80, true),
            0.0,
            reduced_health,
            &reduced,
            reduced_core,
        );
        let reduced_first = build_atmospheric_particles(
            &reduced_field,
            preset,
            reduced_health,
            0.0,
            &reduced,
            &reduced_flow,
        );
        let reduced_later = build_atmospheric_particles(
            &reduced_field,
            preset,
            reduced_health,
            10.0,
            &reduced,
            &reduced_flow,
        );
        assert_eq!(reduced_first, reduced_later);
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

    fn mean_perceptual_intensity(field: &FieldFrame) -> f32 {
        let lighting = field.lighting_range();
        field
            .samples
            .iter()
            .map(|sample| perceptual_intensity(sample.energy, lighting, 8))
            .sum::<f32>()
            / field.samples.len() as f32
    }

    fn max_perceptual_intensity(field: &FieldFrame) -> f32 {
        let lighting = field.lighting_range();
        field
            .samples
            .iter()
            .map(|sample| perceptual_intensity(sample.energy, lighting, 8))
            .fold(0.0, f32::max)
    }

    fn rare_peak_count(field: &FieldFrame) -> usize {
        let lighting = field.lighting_range();
        field
            .samples
            .iter()
            .filter(|sample| perceptual_intensity(sample.energy, lighting, 8) > 0.84)
            .count()
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
