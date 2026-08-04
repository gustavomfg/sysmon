use crate::{
    app::AppState,
    pulse::{PulseHealth, PulseModel},
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

const CURVE_SAMPLES: usize = 28;

const TRAJECTORIES: [Trajectory; 4] = [
    Trajectory {
        control_points: [(0.05, 0.82), (0.12, 0.36), (0.60, 0.12), (0.96, 0.26)],
        phase: 0.20,
        speed: 0.72,
        depth: 0.34,
        thickness: 0.72,
    },
    Trajectory {
        control_points: [(0.16, 0.02), (0.58, 0.14), (0.32, 0.82), (0.90, 0.98)],
        phase: 1.80,
        speed: 1.03,
        depth: 0.58,
        thickness: 0.88,
    },
    Trajectory {
        control_points: [(0.00, 0.45), (0.24, 0.92), (0.78, 0.10), (1.00, 0.68)],
        phase: 3.40,
        speed: 1.27,
        depth: 0.86,
        thickness: 1.00,
    },
    Trajectory {
        control_points: [(0.48, 1.00), (0.10, 0.65), (0.82, 0.32), (0.56, 0.00)],
        phase: 5.10,
        speed: 0.57,
        depth: 0.70,
        thickness: 0.78,
    },
];

const PARTICLE_SEEDS: [f32; 12] = [
    0.03, 0.13, 0.22, 0.31, 0.43, 0.52, 0.61, 0.69, 0.77, 0.84, 0.91, 0.97,
];

#[derive(Debug, Clone, Copy)]
struct Trajectory {
    control_points: [(f32, f32); 4],
    phase: f32,
    speed: f32,
    depth: f32,
    thickness: f32,
}

#[derive(Debug, Clone, Copy, Default, PartialEq)]
struct EnergySample {
    energy: f32,
    layer: usize,
    depth: f32,
    particle: bool,
}

#[derive(Debug, Clone, Copy, Default)]
struct ParticlePoint {
    layer: usize,
    depth: f32,
    x: f32,
    y: f32,
}

#[derive(Debug, Clone, Copy)]
struct FieldFrame {
    trajectory_points: [[(f32, f32); CURVE_SAMPLES + 1]; TRAJECTORIES.len()],
    trajectory_count: usize,
    particles: [ParticlePoint; PARTICLE_SEEDS.len()],
    particle_count: usize,
    particle_radius_scale: f32,
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
    let width = usize::from(area.width.saturating_sub(4)).max(1);
    let height = usize::from(area.height.saturating_sub(2)).max(1);
    let logical_width = width * 2;
    let logical_height = height * 4;
    let health = pulse.health();
    let time = animation_time(pulse);
    let field = build_field(
        logical_width as f32,
        logical_height as f32,
        health,
        time,
        pulse,
    );

    (0..height)
        .map(|cell_y| {
            let mut spans = Vec::with_capacity(width);
            for cell_x in 0..width {
                let mut mask = 0_u8;
                let mut strongest = EnergySample::default();

                for (dot_x, dot_y, bit) in BRAILLE_DOTS {
                    let x = (cell_x * 2 + usize::from(dot_x)) as f32;
                    let y = (cell_y * 4 + usize::from(dot_y)) as f32;
                    let sample = sample_at(x, y, health, &field);
                    if sample.energy > 0.25 {
                        mask |= 1 << bit;
                    }
                    if sample.energy > strongest.energy || sample.particle {
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
    let width = usize::from(area.width.saturating_sub(4)).max(1);
    let height = usize::from(area.height.saturating_sub(2)).max(1);
    let logical_height = height * 2;
    let health = pulse.health();
    let time = animation_time(pulse);
    let field = build_field(width as f32, logical_height as f32, health, time, pulse);

    (0..height)
        .map(|cell_y| {
            let spans = (0..width)
                .map(|cell_x| {
                    let sample =
                        sample_at(cell_x as f32, (cell_y * 2) as f32 + 0.5, health, &field);
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

fn active_trajectory_count(health: PulseHealth, pulse: &PulseModel) -> usize {
    let count = (2.0 + health.density() * 2.0).round() as usize;
    if pulse.reduced_motion() {
        count.clamp(2, 3)
    } else {
        count.clamp(2, TRAJECTORIES.len())
    }
}

fn particle_count(health: PulseHealth, pulse: &PulseModel) -> usize {
    if pulse.reduced_motion() {
        2
    } else {
        (4.0 + health.density() * 8.0).round() as usize
    }
    .clamp(2, PARTICLE_SEEDS.len())
}

fn build_field(
    width: f32,
    height: f32,
    health: PulseHealth,
    time: f32,
    pulse: &PulseModel,
) -> FieldFrame {
    let trajectory_count = active_trajectory_count(health, pulse);
    let breathing = pulse.breath();
    let phase = pulse.phase();
    let field_time = time + phase * 0.05 + pulse.input().volatility * 0.02;
    let mut field = FieldFrame {
        trajectory_points: [[(0.0, 0.0); CURVE_SAMPLES + 1]; TRAJECTORIES.len()],
        trajectory_count,
        particles: [ParticlePoint::default(); PARTICLE_SEEDS.len()],
        particle_count: particle_count(health, pulse),
        particle_radius_scale: if pulse.reduced_motion() { 0.75 } else { 1.0 },
    };

    for (index, trajectory) in TRAJECTORIES.iter().take(trajectory_count).enumerate() {
        for sample_index in 0..=CURVE_SAMPLES {
            let progress = sample_index as f32 / CURVE_SAMPLES as f32;
            field.trajectory_points[index][sample_index] = trajectory_point(
                progress,
                width,
                height,
                *trajectory,
                health,
                field_time + breathing * 0.02,
                pulse,
            );
        }
    }

    for (index, seed) in PARTICLE_SEEDS.iter().take(field.particle_count).enumerate() {
        let layer = index % trajectory_count;
        let trajectory = TRAJECTORIES[layer];
        let direction = if index % 4 == 0 { -1.0 } else { 1.0 };
        let speed = (0.035 + index as f32 * 0.004) * (0.76 + health.speed() * 0.20) * direction;
        let progress = (*seed + time * speed).rem_euclid(1.0);
        let (x, y) = trajectory_point(
            progress, width, height, trajectory, health, field_time, pulse,
        );
        field.particles[index] = ParticlePoint {
            layer,
            depth: trajectory.depth,
            x,
            y,
        };
    }

    field
}

fn sample_at(x: f32, y: f32, health: PulseHealth, field: &FieldFrame) -> EnergySample {
    let mut strongest = EnergySample::default();
    let curve_thickness = 0.65 + health.density() * 0.42 + health.pressure() * 0.10;

    for (index, trajectory) in TRAJECTORIES.iter().take(field.trajectory_count).enumerate() {
        for &(curve_x, curve_y) in &field.trajectory_points[index] {
            let distance = (x - curve_x).hypot(y - curve_y);
            let thickness =
                (trajectory.thickness * curve_thickness + trajectory.depth * 0.75).max(0.8);
            let energy =
                smoothstep(1.0 - distance / (thickness + 2.1)) * (0.50 + trajectory.depth * 0.50);
            if energy > strongest.energy {
                strongest = EnergySample {
                    energy,
                    layer: index,
                    depth: trajectory.depth,
                    particle: false,
                };
            }
        }
    }

    for particle in &field.particles[..field.particle_count] {
        let radius = (0.75 + particle.depth * 0.95) * field.particle_radius_scale;
        if (x - particle.x).hypot(y - particle.y) <= radius {
            strongest.layer = particle.layer;
            strongest.depth = particle.depth;
            strongest.particle = true;
            strongest.energy = strongest.energy.max(0.86);
        }
    }

    strongest
}

fn trajectory_point(
    progress: f32,
    width: f32,
    height: f32,
    trajectory: Trajectory,
    health: PulseHealth,
    time: f32,
    pulse: &PulseModel,
) -> (f32, f32) {
    let progress = progress.clamp(0.0, 1.0);
    let base = cubic_bezier(trajectory.control_points, progress);
    let movement = if pulse.reduced_motion() { 0.0 } else { 1.0 };
    let dynamic_scale = health.amplitude() * (0.80 + health.volatility() * 0.40);
    let edge_falloff = (progress * (1.0 - progress) * 4.0).sqrt();
    let curl =
        (progress * std::f32::consts::TAU * 1.7 + trajectory.phase + time * trajectory.speed).sin();
    let counter_curl = (progress * std::f32::consts::TAU * 0.73 - trajectory.phase * 1.3
        + time * trajectory.speed * 0.61)
        .cos();
    let drift = (time * 0.19 + trajectory.phase).sin();
    let turbulence = 0.012 + health.volatility() * 0.040;

    let x = base.0 * width
        + movement
            * (curl * width * turbulence * (0.6 + trajectory.depth * 0.5)
                + drift * width * 0.012 * edge_falloff);
    let y = base.1 * height
        + movement
            * dynamic_scale
            * (counter_curl * height * turbulence * 1.8
                + curl * height * 0.018 * trajectory.depth
                + drift * height * 0.015 * edge_falloff);

    (x.clamp(0.0, width), y.clamp(0.0, height))
}

fn cubic_bezier(points: [(f32, f32); 4], progress: f32) -> (f32, f32) {
    let inverse = 1.0 - progress;
    let first = inverse * inverse * inverse;
    let second = 3.0 * inverse * inverse * progress;
    let third = 3.0 * inverse * progress * progress;
    let fourth = progress * progress * progress;

    (
        points[0].0 * first + points[1].0 * second + points[2].0 * third + points[3].0 * fourth,
        points[0].1 * first + points[1].1 * second + points[2].1 * third + points[3].1 * fourth,
    )
}

fn unicode_symbol(sample: EnergySample) -> char {
    if sample.particle {
        '•'
    } else if sample.energy > 0.72 {
        '●'
    } else if sample.energy > 0.42 {
        '∙'
    } else if sample.energy > 0.20 {
        '·'
    } else {
        ' '
    }
}

fn sample_style(theme: &Theme, health: PulseHealth, sample: EnergySample) -> Style {
    let base = health_color(theme, health.intensity());
    let layer_tint = match sample.layer % 3 {
        0 => theme.secondary,
        1 => theme.primary,
        _ => theme.pulse_medium,
    };
    let color = blend_color(base, layer_tint, 0.10 + sample.depth * 0.20);
    let brightness = (0.28
        + sample.depth * 0.32
        + sample.energy * 0.40
        + if sample.particle { 0.14 } else { 0.0 })
    .clamp(0.0, 1.0);
    let style = Style::default().fg(scale_color(color, brightness));

    if sample.particle {
        style.add_modifier(Modifier::BOLD)
    } else {
        style
    }
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
    use std::time::Duration;

    use ratatui::layout::Rect;

    use super::{
        TRAJECTORIES, braille_lines, build_field, sample_at, trajectory_point, unicode_lines,
        uses_braille,
    };
    use crate::{pulse::PulseModel, theme::Theme};

    #[test]
    fn selects_braille_for_a_usable_pulse_area() {
        assert!(uses_braille(Rect::new(0, 0, 32, 10)));
        assert!(!uses_braille(Rect::new(0, 0, 20, 6)));
    }

    #[test]
    fn renders_both_terminal_native_field_variants() {
        let mut pulse = PulseModel::new(false);
        pulse.set_input(crate::pulse::PulseInput {
            cpu: 0.6,
            memory: 0.7,
            gpu: None,
            network: 0.3,
            load: 0.4,
            volatility: 0.2,
        });
        let theme = Theme::default();

        assert!(!braille_lines(Rect::new(0, 0, 32, 10), &pulse, &theme).is_empty());
        assert!(!unicode_lines(Rect::new(0, 0, 20, 6), &pulse, &theme).is_empty());
    }

    #[test]
    fn trajectories_change_direction_and_cross_the_field() {
        let mut pulse = PulseModel::new(false);
        pulse.set_input(crate::pulse::PulseInput {
            cpu: 0.6,
            memory: 0.7,
            gpu: Some(0.4),
            network: 0.3,
            load: 0.5,
            volatility: 0.2,
        });
        let health = pulse.health();
        let vertical_motion = TRAJECTORIES
            .iter()
            .filter(|trajectory| {
                let start = trajectory_point(0.12, 64.0, 32.0, **trajectory, health, 0.0, &pulse);
                let end = trajectory_point(0.88, 64.0, 32.0, **trajectory, health, 0.0, &pulse);
                (start.1 - end.1).abs() > 4.0
            })
            .count();
        assert!(vertical_motion >= 3);

        let mut close_pairs = 0;
        for (first, first_trajectory) in TRAJECTORIES.iter().enumerate() {
            for second_trajectory in TRAJECTORIES.iter().skip(first + 1) {
                let intersects = (0..=20).any(|step| {
                    let progress = step as f32 / 20.0;
                    let one = trajectory_point(
                        progress,
                        64.0,
                        32.0,
                        *first_trajectory,
                        health,
                        0.0,
                        &pulse,
                    );
                    let two = trajectory_point(
                        progress,
                        64.0,
                        32.0,
                        *second_trajectory,
                        health,
                        0.0,
                        &pulse,
                    );
                    (one.0 - two.0).hypot(one.1 - two.1) < 5.0
                });
                close_pairs += usize::from(intersects);
            }
        }
        assert!(close_pairs >= 2);
    }

    #[test]
    fn particle_positions_are_deterministic_for_the_same_time() {
        let mut pulse = PulseModel::new(false);
        pulse.set_input(crate::pulse::PulseInput {
            cpu: 0.8,
            memory: 0.8,
            gpu: Some(0.5),
            network: 0.4,
            load: 0.6,
            volatility: 0.3,
        });
        pulse.update(Duration::from_millis(120));
        let health = pulse.health();
        let field = build_field(32.0, 20.0, health, pulse.time_seconds(), &pulse);
        let first = sample_at(16.0, 10.0, health, &field);
        let second = sample_at(16.0, 10.0, health, &field);

        assert_eq!(first, second);
        let particle_is_visible = (0..32)
            .any(|x| (0..20).any(|y| sample_at(x as f32, y as f32, health, &field).particle));
        assert!(particle_is_visible);
    }
}
