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

const LAYERS: [EnergyLayer; 4] = [
    EnergyLayer {
        offset: -0.28,
        frequency: 0.82,
        speed: 0.72,
        phase: 0.20,
        width: 0.90,
    },
    EnergyLayer {
        offset: -0.08,
        frequency: 1.13,
        speed: 1.03,
        phase: 1.80,
        width: 0.72,
    },
    EnergyLayer {
        offset: 0.14,
        frequency: 0.68,
        speed: 1.27,
        phase: 3.40,
        width: 0.82,
    },
    EnergyLayer {
        offset: 0.31,
        frequency: 1.36,
        speed: 0.57,
        phase: 5.10,
        width: 0.62,
    },
];

const PARTICLE_SEEDS: [f32; 12] = [
    0.03, 0.13, 0.22, 0.31, 0.43, 0.52, 0.61, 0.69, 0.77, 0.84, 0.91, 0.97,
];

#[derive(Debug, Clone, Copy)]
struct EnergyLayer {
    offset: f32,
    frequency: f32,
    speed: f32,
    phase: f32,
    width: f32,
}

#[derive(Debug, Clone, Copy, Default, PartialEq)]
struct EnergySample {
    energy: f32,
    layer: usize,
    particle: bool,
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

    (0..height)
        .map(|cell_y| {
            let mut spans = Vec::with_capacity(width);
            for cell_x in 0..width {
                let mut mask = 0_u8;
                let mut strongest = EnergySample::default();

                for (dot_x, dot_y, bit) in BRAILLE_DOTS {
                    let x = (cell_x * 2 + usize::from(dot_x)) as f32;
                    let y = (cell_y * 4 + usize::from(dot_y)) as f32;
                    let sample = sample_at(
                        x,
                        y,
                        logical_width as f32,
                        logical_height as f32,
                        health,
                        time,
                        pulse,
                    );
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

    (0..height)
        .map(|cell_y| {
            let spans = (0..width)
                .map(|cell_x| {
                    let sample = sample_at(
                        cell_x as f32,
                        (cell_y * 2) as f32 + 0.5,
                        width as f32,
                        logical_height as f32,
                        health,
                        time,
                        pulse,
                    );
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

fn sample_at(
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    health: PulseHealth,
    time: f32,
    pulse: &PulseModel,
) -> EnergySample {
    let mut strongest = EnergySample::default();
    let density = health.density();
    let breathing = pulse.breath();
    let phase = pulse.phase();
    let amplitude = health.amplitude()
        * (0.86 + breathing * 0.18)
        * if pulse.reduced_motion() { 0.55 } else { 1.0 };
    let line_scale = 0.75 + density * 0.55 + health.pressure() * 0.10 + health.volatility() * 0.15;
    let phase_time = time + phase * 0.05 + pulse.input().volatility * 0.02;

    for (index, layer) in LAYERS.iter().enumerate() {
        let line_y = layer_y(x, width, height, *layer, amplitude, phase_time);
        let distance = (y - line_y).abs();
        let line_width = (0.65 + layer.width * line_scale).max(0.8);
        let energy = smoothstep(1.0 - distance / (line_width + 2.2));
        if energy > strongest.energy {
            strongest = EnergySample {
                energy,
                layer: index,
                particle: false,
            };
        }
    }

    if particle_near(x, y, width, height, health, time, pulse) {
        strongest.particle = true;
        strongest.energy = strongest.energy.max(0.85);
    }

    strongest
}

fn layer_y(x: f32, width: f32, height: f32, layer: EnergyLayer, amplitude: f32, time: f32) -> f32 {
    let normalized_x = (x / width.max(1.0)).clamp(0.0, 1.0);
    let centered_x = normalized_x * 2.0 - 1.0;
    let envelope = (1.0 - centered_x.abs() * 0.62).clamp(0.35, 1.0);
    let core_pull = 1.0 - centered_x.abs() * 0.18;
    let span = height * (0.16 + amplitude * 0.035) * envelope;
    let primary =
        (normalized_x * std::f32::consts::TAU * layer.frequency + time * layer.speed + layer.phase)
            .sin();
    let secondary = (normalized_x * std::f32::consts::TAU * (layer.frequency * 0.43 + 0.22)
        - time * layer.speed * 0.61
        + layer.phase * 1.7)
        .cos();
    let fold =
        (centered_x * centered_x * 2.0 - 0.55) * (time * 0.17 + layer.phase).sin() * height * 0.035;

    height * 0.5
        + layer.offset * height * 0.72 * core_pull
        + primary * span * layer.width * 0.55
        + secondary * span * 0.30
        + fold
}

fn particle_near(
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    health: PulseHealth,
    time: f32,
    pulse: &PulseModel,
) -> bool {
    let count = if pulse.reduced_motion() {
        2
    } else {
        ((PARTICLE_SEEDS.len() as f32 * health.density()).round() as usize)
            .clamp(3, PARTICLE_SEEDS.len())
    };

    (0..count).any(|index| {
        let layer = LAYERS[index % LAYERS.len()];
        let seed = PARTICLE_SEEDS[index];
        let speed = 0.045 + index as f32 * 0.006 + health.speed() * 0.008;
        let progress = (seed + time * speed).rem_euclid(1.0);
        let particle_x = progress * width.max(1.0);
        let particle_y = layer_y(particle_x, width, height, layer, health.amplitude(), time);
        let radius = if pulse.reduced_motion() { 1.0 } else { 1.25 };
        (x - particle_x).hypot(y - particle_y) <= radius
    })
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
    let layer_tint = if sample.layer.is_multiple_of(2) {
        theme.secondary
    } else {
        theme.primary
    };
    let color = blend_color(base, layer_tint, 0.12 + sample.layer as f32 * 0.04);
    let brightness = 0.52 + sample.energy * 0.48;
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

    use super::{braille_lines, sample_at, unicode_lines, uses_braille};
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
        let first = sample_at(16.0, 10.0, 32.0, 20.0, health, pulse.time_seconds(), &pulse);
        let second = sample_at(16.0, 10.0, 32.0, 20.0, health, pulse.time_seconds(), &pulse);

        assert_eq!(first, second);
        let particle_is_visible = (0..32).any(|x| {
            (0..20).any(|y| {
                sample_at(
                    x as f32,
                    y as f32,
                    32.0,
                    20.0,
                    health,
                    pulse.time_seconds(),
                    &pulse,
                )
                .particle
            })
        });
        assert!(particle_is_visible);
    }
}
