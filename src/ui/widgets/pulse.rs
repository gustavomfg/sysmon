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
const FIELD_THRESHOLD: f32 = 0.05;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PulsePreset {
    Compact,
    Standard,
    Widescreen,
    LargeWidescreen,
}

#[derive(Debug, Clone, Copy)]
struct PresetParameters {
    streams: usize,
    trail_steps: usize,
    step_size: f32,
    density: f32,
    particle_stride: usize,
    margin: f32,
    virtual_margin: f32,
    diffusion_passes: usize,
}

impl PulsePreset {
    fn parameters(self) -> PresetParameters {
        match self {
            Self::Compact => PresetParameters {
                streams: 12,
                trail_steps: 34,
                step_size: 0.025,
                density: 0.68,
                particle_stride: 15,
                margin: 0.08,
                virtual_margin: 0.16,
                diffusion_passes: 1,
            },
            Self::Standard => PresetParameters {
                streams: 20,
                trail_steps: 48,
                step_size: 0.022,
                density: 0.78,
                particle_stride: 13,
                margin: 0.06,
                virtual_margin: 0.20,
                diffusion_passes: 2,
            },
            Self::Widescreen => PresetParameters {
                streams: 28,
                trail_steps: 64,
                step_size: 0.019,
                density: 0.86,
                particle_stride: 11,
                margin: 0.045,
                virtual_margin: 0.24,
                diffusion_passes: 2,
            },
            Self::LargeWidescreen => PresetParameters {
                streams: 36,
                trail_steps: 76,
                step_size: 0.017,
                density: 0.92,
                particle_stride: 10,
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

    fn local_energy(&self, position: (f32, f32), radius: f32) -> f32 {
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
                energy += self.sample(x, y).energy * local_weight;
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
                let falloff = (1.0 - distance / (radius * self.dot_aspect.max(1.0))).max(0.0);
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
            let local_energy = if step == 0 { energy } else { energy * 0.68 };
            self.deposit(position, local_energy, radius, layer, depth);
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
                    let mut neighborhood_energy = current.energy;
                    let mut neighborhood_count = 1.0;
                    let mut strongest = current;

                    for neighbor_y in y.saturating_sub(1)..=(y + 1).min(self.height - 1) {
                        for neighbor_x in x.saturating_sub(1)..=(x + 1).min(self.width - 1) {
                            if neighbor_x == x && neighbor_y == y {
                                continue;
                            }
                            let neighbor = self.samples[neighbor_y * self.width + neighbor_x];
                            neighborhood_energy += neighbor.energy;
                            neighborhood_count += 1.0;
                            if neighbor.energy > strongest.energy {
                                strongest = neighbor;
                            }
                        }
                    }

                    next[index] = EnergySample {
                        energy: (current.energy * (1.0 - amount)
                            + neighborhood_energy / neighborhood_count * amount)
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
struct StreamSpec {
    seed: (f32, f32),
    phase: f32,
    speed: f32,
    direction: f32,
    layer: usize,
    depth: f32,
    edge_span: bool,
}

#[derive(Debug, Clone, Copy)]
struct FlowGeometry {
    aspect: f32,
    step_size: f32,
    virtual_margin: f32,
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
                    sample_style(theme, health, strongest),
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

    (0..height)
        .map(|cell_y| {
            let spans = (0..width)
                .map(|cell_x| {
                    let sample = field.sample(cell_x * 2 + 1, cell_y * 2 + 1);
                    let symbol = unicode_symbol(sample);
                    Span::styled(symbol.to_string(), sample_style(theme, health, sample))
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
    let active_streams = active_stream_count(parameters.streams, health, pulse);
    let base_trail_steps = active_trail_steps(parameters.trail_steps, pulse);
    let geometry = FlowGeometry {
        aspect: domain_aspect(width, height, braille),
        step_size: parameters.step_size,
        virtual_margin: parameters.virtual_margin,
    };
    let motion = if pulse.reduced_motion() { 0.0 } else { 1.0 };
    let breathing = if pulse.reduced_motion() {
        1.0
    } else {
        0.975 + (pulse.breath() - 0.5) * 0.10
    };
    let input = pulse.input();
    let field_time = time + pulse.phase() * 0.025 + input.network * 0.018 + input.load * 0.012;

    for stream_index in 0..active_streams {
        let stream = stream_spec(
            stream_index,
            active_streams,
            parameters.margin,
            parameters.virtual_margin,
        );
        let trail_steps = if stream.edge_span {
            edge_span_steps(
                base_trail_steps,
                parameters.step_size,
                parameters.virtual_margin,
            )
        } else {
            base_trail_steps
        };
        let lead_steps = if pulse.reduced_motion() {
            0
        } else {
            ((field_time * stream.speed * 23.0 + stream.phase * 7.0).rem_euclid(160.0)) as usize
        };
        let mut position = advect_steps(
            stream.seed,
            lead_steps,
            stream,
            field_time,
            health,
            geometry,
        );

        for trail_index in 0..trail_steps {
            let progress = trail_index as f32 / trail_steps.max(1) as f32;
            let rhythm = ((trail_index as f32 * 0.83 + stream.phase * 1.7).sin() * 0.5) + 0.5;
            let taper = 0.68 + (1.0 - progress) * 0.32;
            let layer_weight = 0.72 + stream.depth * 0.28;
            let energy = (0.12 + health.density() * parameters.density * 0.31 + rhythm * 0.12)
                * layer_weight
                * (0.90 + health.amplitude() * 0.10)
                * (0.86 + health.pressure().clamp(0.0, 1.0) * 0.14)
                * taper;
            let highlight =
                trail_index % parameters.particle_stride == stream_index % 3 && rhythm > 0.68;
            let energy = if highlight { energy + 0.28 } else { energy };
            let radius = if highlight {
                1.25
            } else {
                1.0 + stream.depth * 0.25
            };
            let next_position = advect_steps(
                position,
                1,
                stream,
                field_time + progress * 0.12,
                health,
                geometry,
            );
            let start = breathe_position(position, breathing, motion);
            let end = breathe_position(next_position, breathing, motion);
            field.deposit_segment(start, end, energy, radius, stream.layer, stream.depth);
            position = next_position;
        }
    }

    let diffusion_amount = 0.18 + health.density() * 0.08;
    apply_discharges(&mut field, health, field_time, pulse);
    field.diffuse(parameters.diffusion_passes, diffusion_amount);

    field
}

fn active_stream_count(base: usize, health: PulseHealth, pulse: &PulseModel) -> usize {
    let density_factor = 0.72 + health.density() * 0.28;
    let reduced_factor = if pulse.reduced_motion() { 0.68 } else { 1.0 };
    ((base as f32 * density_factor * reduced_factor).round() as usize).max(FLOW_LAYERS)
}

fn active_trail_steps(base: usize, pulse: &PulseModel) -> usize {
    if pulse.reduced_motion() {
        (base * 3 / 4).max(12)
    } else {
        base
    }
}

fn apply_discharges(field: &mut FieldFrame, health: PulseHealth, time: f32, pulse: &PulseModel) {
    if pulse.reduced_motion() || health.volatility() < 0.06 {
        return;
    }

    let volatility = smoothstep((health.volatility() - 0.06) / 0.34);
    let nodes = [(0.47, 0.46, 0.4), (0.56, 0.55, 2.2)];
    for (index, (node_x, node_y, phase)) in nodes.into_iter().enumerate() {
        let convergence = field.local_energy((node_x, node_y), 3.5);
        let node_energy = smoothstep((convergence - 0.10) / 0.26) * volatility;
        let cadence = ((time * (1.05 + index as f32 * 0.18) + phase).sin() * 0.5) + 0.5;
        let burst = smoothstep((cadence - 0.72) / 0.28);
        let discharge = node_energy * burst;
        if discharge < 0.08 {
            continue;
        }

        field.deposit(
            (node_x, node_y),
            discharge * 0.62,
            1.35 + discharge * 1.2,
            FLOW_LAYERS,
            1.0,
        );
        let flow = flow_vector((node_x, node_y), time, health, field.dot_aspect);
        let flow_angle = flow.1.atan2(flow.0);
        for branch in 0..3 {
            let branch_angle =
                flow_angle + (branch as f32 - 1.0) * 0.75 + (time * 0.13 + phase).sin() * 0.12;
            let length = 0.035 + discharge * (0.035 + branch as f32 * 0.008);
            let endpoint = (
                node_x + branch_angle.cos() * length,
                node_y + branch_angle.sin() * length,
            );
            field.deposit_segment(
                (node_x, node_y),
                endpoint,
                discharge * (0.20 - branch as f32 * 0.025),
                0.62,
                FLOW_LAYERS,
                1.0,
            );
        }
    }
}

fn edge_span_steps(base: usize, step_size: f32, virtual_margin: f32) -> usize {
    let required_length = 1.65 + virtual_margin * 2.0;
    base.max((required_length / step_size).ceil() as usize)
}

fn domain_aspect(width: usize, height: usize, braille: bool) -> f32 {
    let dot_aspect = if braille { 1.0 } else { 1.8 };
    (height as f32 / width.max(1) as f32 * dot_aspect).clamp(0.22, 3.5)
}

fn stream_spec(index: usize, count: usize, margin: f32, virtual_margin: f32) -> StreamSpec {
    let layer = index % FLOW_LAYERS;
    let lane = index / FLOW_LAYERS;
    let lane_count = count.div_ceil(FLOW_LAYERS).max(1);
    let lane_position = (lane as f32 + 0.5) / lane_count as f32;
    let jitter = hash01(index * 29 + 7) - 0.5;
    let upper = layer < 2;
    let center = if upper { 0.31 } else { 0.69 };
    let spread = if upper { 0.28 } else { 0.26 };
    let seed_y =
        (center + (lane_position - 0.5) * spread + jitter * 0.035).clamp(margin, 1.0 - margin);
    let from_left = layer.is_multiple_of(2);
    let phase = hash01(index * 41 + 13) * TAU;
    let edge_offset = hash01(index * 71 + 31) * 0.035;

    StreamSpec {
        seed: (
            if from_left {
                -virtual_margin - edge_offset
            } else {
                1.0 + virtual_margin + edge_offset
            },
            seed_y,
        ),
        phase,
        speed: 0.78 + hash01(index * 53 + 19) * 0.34,
        direction: if from_left { 1.0 } else { -1.0 },
        layer,
        depth: 0.27 + layer as f32 * 0.18 + hash01(index * 67 + 23) * 0.10,
        edge_span: index < FLOW_LAYERS,
    }
}

fn advect_steps(
    mut position: (f32, f32),
    steps: usize,
    stream: StreamSpec,
    time: f32,
    health: PulseHealth,
    geometry: FlowGeometry,
) -> (f32, f32) {
    for step in 0..steps {
        let vector = flow_vector(
            position,
            time + step as f32 * 0.011,
            health,
            geometry.aspect,
        );
        let direction = stream.direction;
        position.0 = wrap_virtual(
            position.0 + vector.0 * geometry.step_size * direction,
            geometry.virtual_margin,
        );
        position.1 += vector.1 * geometry.step_size * direction;
        position.1 = reflect(position.1, 0.025, 0.975);
    }
    position
}

fn wrap_virtual(value: f32, margin: f32) -> f32 {
    let span = 1.0 + margin * 2.0;
    (value + margin).rem_euclid(span) - margin
}

fn flow_vector(position: (f32, f32), time: f32, health: PulseHealth, aspect: f32) -> (f32, f32) {
    let phase = time * (0.06 + health.volatility() * 0.08);
    let braid_phase = TAU * (position.0 * 0.83 + position.1 * 0.61) + phase;
    let bend_phase = TAU * (position.0 * 0.47 - position.1 * 0.92) - phase * 0.63;
    let mut vx = 0.54 + braid_phase.sin() * 0.10;
    let mut vy =
        braid_phase.cos() * (0.12 + health.amplitude() * 0.035) + (0.5 - position.1) * 0.10;

    vx += bend_phase.cos() * 0.055;
    vy += bend_phase.sin() * (0.09 + health.volatility() * 0.10);

    let vortices = [
        (0.30, 0.34, 1.0, 0.035),
        (0.68, 0.66, -1.0, 0.041),
        (0.52, 0.50, 0.75, 0.026),
    ];
    for (center_x, center_y, rotation, strength) in vortices {
        let dx = position.0 - center_x;
        let dy = (position.1 - center_y) * aspect;
        let radius_squared = (dx * dx + dy * dy).max(0.018);
        let influence = strength * rotation / (radius_squared + 0.065);
        vx += -dy * influence;
        vy += dx * influence / aspect.max(0.2);
    }

    let length = vx.hypot(vy).max(0.001);
    (vx / length, vy / length)
}

fn breathe_position(position: (f32, f32), scale: f32, motion: f32) -> (f32, f32) {
    let scale = 1.0 + (scale - 1.0) * motion;
    (
        0.5 + (position.0 - 0.5) * scale,
        (0.5 + (position.1 - 0.5) * scale).clamp(0.015, 0.985),
    )
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

fn unicode_symbol(sample: EnergySample) -> char {
    if sample.energy > 0.55 {
        '•'
    } else if sample.energy > 0.28 {
        '∙'
    } else if sample.energy > 0.12 {
        '·'
    } else {
        ' '
    }
}

fn sample_style(theme: &Theme, health: PulseHealth, sample: EnergySample) -> Style {
    let base = health_color(theme, health.intensity());
    let layer_tint = match sample.layer % FLOW_LAYERS {
        0 => theme.secondary,
        1 => theme.primary,
        2 => theme.pulse_medium,
        _ => theme.pulse_high,
    };
    let color = blend_color(base, layer_tint, 0.05 + sample.depth * 0.10);
    let luminous = smoothstep((sample.energy - 0.72) / 0.28);
    let color = blend_color(color, theme.text, luminous * (0.32 + sample.depth * 0.28));
    let brightness = (0.26 + sample.depth * 0.34 + sample.energy * 0.44).clamp(0.0, 1.0);
    Style::default().fg(scale_color(color, brightness))
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
    use ratatui::layout::Rect;

    use super::{
        EnergySample, FIELD_THRESHOLD, FieldFrame, PulsePreset, braille_lines, build_field,
        field_for_area, preset_for, unicode_lines, uses_braille,
    };
    use crate::{
        pulse::{HealthState, PulseInput, PulseModel},
        theme::Theme,
    };

    fn pulse() -> PulseModel {
        let mut pulse = PulseModel::new(false);
        pulse.set_input(crate::pulse::PulseInput {
            cpu: 0.6,
            memory: 0.7,
            gpu: Some(0.4),
            network: 0.3,
            load: 0.5,
            volatility: 0.2,
        });
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
