//! Shared, allocation-free line rasterizer for the existing terminal regions.
//! Each terminal cell provides 2 × 4 Braille subpixels. No smoothing changes
//! telemetry, and missing samples break the line instead of implying activity.
use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Modifier, Style},
};

use super::super::{chart::SparklineModel, metrics::CHART_POINTS};

type Series = (Option<SparklineModel<CHART_POINTS>>, Style);

#[derive(Clone, Copy)]
pub(super) enum Scale {
    Percent,
    Memory,
    Network,
}

impl Scale {
    fn bounds(self, series: &[Series], limit: usize) -> (f64, f64) {
        match self {
            Self::Percent | Self::Memory => {
                let mut minimum = 1.0_f64;
                let mut maximum = 0.0_f64;
                let mut found = false;
                for value in series
                    .iter()
                    .filter_map(|(model, _)| model.as_ref())
                    .flat_map(|model| model.visible_samples(limit).flatten())
                    .filter(|value| value.is_finite())
                {
                    let value = value.clamp(0.0, 1.0);
                    minimum = minimum.min(value);
                    maximum = maximum.max(value);
                    found = true;
                }
                if !found {
                    return (0.0, 1.0);
                }

                // Percent values are normalized 0..1. Reserve at least two
                // percentage points of padding on each side, and a ten-point
                // viewport (thirty for RAM) so insignificant noise cannot fill the chart.
                let padding = ((maximum - minimum) * 0.20).max(0.02);
                let minimum_span = if matches!(self, Self::Memory) {
                    0.30
                } else {
                    0.10
                };
                let span = (maximum - minimum + 2.0 * padding).clamp(minimum_span, 1.0);
                let center = (minimum + maximum) / 2.0;
                // Shift the viewport at domain edges, preserving the minimum
                // span rather than clipping it into a tiny range near 0 or 100.
                let lower = (center - span / 2.0).clamp(0.0, 1.0 - span);
                let upper = lower + span;
                // Outward two-percentage-point bands keep nearby extrema on
                // identical bounds. Stateless and deterministic across frames.
                (
                    (lower * 50.0).floor() / 50.0,
                    ((upper * 50.0).ceil() / 50.0).min(1.0),
                )
            }
            Self::Network => {
                // A zero baseline and 1 KiB/s floor keep idle byte-sized noise
                // from filling the plot. Power-of-two bands remain stable while
                // the recent peak stays in the same band; both directions share it.
                let peak = series
                    .iter()
                    .filter_map(|(model, _)| model.as_ref())
                    .flat_map(|model| model.visible_samples(limit).flatten())
                    .filter(|value| value.is_finite())
                    .fold(1024.0_f64, f64::max);
                (
                    0.0,
                    2.0_f64.powf(peak.log2().ceil()).max(peak).min(f64::MAX),
                )
            }
        }
    }
}

fn visible_sample_limit(width: u16) -> usize {
    (usize::from(width) / 2 + 1).clamp(2, super::super::chart::VISIBLE_HISTORY_SAMPLES)
}

/// Use at most three terminal rows, degrading only when text leaves less room.
pub(super) fn region(body: Rect, offset: u16) -> Rect {
    Rect::new(
        body.x,
        body.y + offset.min(body.height),
        body.width,
        body.height.saturating_sub(offset).min(3),
    )
}

pub(super) fn render(series: &[Series], scale: Scale, area: Rect, buffer: &mut Buffer) {
    // Keep geometry in the requested viewport when its edges are clipped.
    let clip = area.intersection(*buffer.area());
    if area.is_empty() || clip.is_empty() {
        return;
    }
    let limit = visible_sample_limit(area.width);
    let (lower, upper) = scale.bounds(series, limit);
    let width = usize::from(area.width) * 2;
    let height = usize::from(area.height) * 4;

    // Rasterize bounded strips of the logical canvas, then encode whole 2x4
    // groups. Scratch memory stays fixed even on exceptionally large terminals.
    for row in clip.y..clip.bottom() {
        for left in (clip.x..clip.right()).step_by(CANVAS_CELLS) {
            let columns = usize::from(clip.right() - left).min(CANVAS_CELLS);
            let mut canvas = DotCanvas::new(
                usize::from(left - area.x) * 2,
                usize::from(row - area.y) * 4,
                columns,
            );
            for (series_index, (model, _)) in series.iter().enumerate() {
                let Some(model) = model else { continue };
                let count = model.sample_count().min(limit);
                let mut previous = None;
                for (index, sample) in model.visible_samples(limit).enumerate() {
                    let Some(value) = sample.filter(|value| value.is_finite()) else {
                        previous = None;
                        continue;
                    };
                    // Do not quantize vertices: round only the interpolated
                    // geometric dots in DDA, never the telemetry samples.
                    let x = if count <= 1 {
                        (width - 1) as f64
                    } else {
                        index as f64 * (width - 1) as f64 / (count - 1) as f64
                    };
                    let y = (1.0 - ((value - lower) / (upper - lower)).clamp(0.0, 1.0))
                        * (height - 1) as f64;
                    let current = (x, y);
                    let start = previous.unwrap_or(if count == 1 { (0.0, y) } else { current });
                    canvas.segment(start, current, series_index);
                    previous = Some(current);
                }
            }
            canvas.write(buffer, left, row, series);
        }
    }
}

const CANVAS_CELLS: usize = 256;
const BRAILLE_BITS: [[u8; 4]; 2] = [[1, 2, 4, 64], [8, 16, 32, 128]];

/// A strip of the full logical dot canvas, with global pixel coordinates.
/// Masks contain dots only; no terminal glyph is read or written while drawing.
struct DotCanvas {
    x: usize,
    y: usize,
    columns: usize,
    masks: [u8; CANVAS_CELLS],
    owners: [usize; CANVAS_CELLS],
}

impl DotCanvas {
    fn new(x: usize, y: usize, columns: usize) -> Self {
        Self {
            x,
            y,
            columns: columns.min(CANVAS_CELLS),
            masks: [0; CANVAS_CELLS],
            owners: [0; CANVAS_CELLS],
        }
    }

    fn dot(&mut self, x: f64, y: f64, owner: usize) {
        let x = x.round();
        let y = y.round();
        if x < self.x as f64
            || x >= (self.x + self.columns * 2) as f64
            || y < self.y as f64
            || y >= (self.y + 4) as f64
        {
            return;
        }
        let x = x as usize - self.x;
        let y = y as usize - self.y;
        self.masks[x / 2] |= BRAILLE_BITS[x % 2][y];
        // Terminal cells have one foreground color. At overlaps preserve both
        // masks and use the last series' color, matching the previous renderer.
        self.owners[x / 2] = owner;
    }

    fn segment(&mut self, start: (f64, f64), end: (f64, f64), owner: usize) {
        if self.columns == 0
            || ![start.0, start.1, end.0, end.1]
                .iter()
                .all(|v| v.is_finite())
        {
            return;
        }
        let dx = end.0 - start.0;
        let dy = end.1 - start.1;
        let steps = dx.abs().max(dy.abs()).ceil();
        if steps == 0.0 {
            self.dot(start.0, start.1, owner);
            return;
        }

        // Parametric clipping includes the half-dot rounding footprint. Keep
        // the original segment's DDA phase so strip boundaries cannot add seams.
        let (mut first, mut last) = (0.0_f64, 1.0_f64);
        for (origin, delta, low, high) in [
            (
                start.0,
                dx,
                self.x as f64 - 0.5,
                (self.x + self.columns * 2) as f64 - 0.5,
            ),
            (start.1, dy, self.y as f64 - 0.5, (self.y + 4) as f64 - 0.5),
        ] {
            if delta == 0.0 {
                if origin < low || origin > high {
                    return;
                }
            } else {
                let a = (low - origin) / delta;
                let b = (high - origin) / delta;
                first = first.max(a.min(b));
                last = last.min(a.max(b));
            }
        }
        if first > last {
            return;
        }
        let first_step = (first * steps).floor().max(0.0) as usize;
        let last_step = (last * steps).ceil().min(steps) as usize;
        for step in first_step..=last_step {
            let t = step as f64 / steps;
            self.dot(start.0 + t * dx, start.1 + t * dy, owner);
        }
    }

    fn write(&self, buffer: &mut Buffer, x: u16, y: u16, series: &[Series]) {
        for column in 0..self.columns {
            let cell = &mut buffer[(x + column as u16, y)];
            let mask = self.masks[column];
            if mask == 0 {
                cell.set_char(' ');
            } else {
                let style = series[self.owners[column]]
                    .1
                    .remove_modifier(Modifier::BOLD | Modifier::DIM);
                cell.set_char(char::from_u32(0x2800 + u32::from(mask)).unwrap())
                    .set_style(style);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::super::{MetricAvailability, MetricUnit, TimeSample, TimeSeries, TimeWindow};
    use super::*;
    use ratatui::style::Color;
    use std::time::Duration;

    fn model(values: &[Option<f64>]) -> SparklineModel<CHART_POINTS> {
        let mut history = TimeSeries::<120>::new(MetricUnit::BytesPerSecond);
        for (index, value) in values.iter().enumerate() {
            history
                .push(
                    TimeSample::new(
                        Duration::from_secs(index as u64),
                        *value,
                        if value.is_some() {
                            MetricAvailability::Available
                        } else {
                            MetricAvailability::Unavailable
                        },
                        None,
                    )
                    .unwrap(),
                )
                .unwrap();
        }
        history.sparkline(TimeWindow::Medium)
    }

    fn mask(buffer: &Buffer, x: u16, y: u16) -> u32 {
        (buffer[(x, y)].symbol().chars().next().unwrap() as u32).saturating_sub(0x2800)
    }

    #[test]
    fn constant_zero_low_and_full_scale_span_the_width_without_fill() {
        for (value, expected) in [(0.0, 192), (1.0, 9)] {
            let mut buffer = Buffer::empty(Rect::new(0, 0, 30, 4));
            render(
                &[(Some(model(&[Some(value); 4])), Style::default())],
                Scale::Percent,
                *buffer.area(),
                &mut buffer,
            );
            let row = if value == 1.0 { 0 } else { 3 };
            for x in 0..30 {
                assert_eq!(mask(&buffer, x, row), expected);
            }
            assert_eq!(
                buffer
                    .content
                    .iter()
                    .filter(|cell| cell.symbol() != " ")
                    .count(),
                30
            );
        }
    }

    #[test]
    fn peak_and_drop_reach_exact_scale_extremes() {
        let mut buffer = Buffer::empty(Rect::new(0, 0, 21, 4));
        render(
            &[(
                Some(model(&[Some(0.0), Some(1.0), Some(0.0)])),
                Style::default(),
            )],
            Scale::Percent,
            *buffer.area(),
            &mut buffer,
        );
        assert_ne!(mask(&buffer, 0, 3) & 64, 0);
        assert_ne!(mask(&buffer, 10, 0) & 9, 0);
        assert_ne!(mask(&buffer, 20, 3) & 128, 0);
    }

    #[test]
    fn missing_readings_break_the_line_and_empty_history_stays_empty() {
        for values in [&[][..], &[None, None][..]] {
            let mut buffer = Buffer::empty(Rect::new(0, 0, 20, 3));
            render(
                &[(Some(model(values)), Style::default())],
                Scale::Percent,
                *buffer.area(),
                &mut buffer,
            );
            assert!(buffer.content.iter().all(|cell| cell.symbol() == " "));
        }
        let mut buffer = Buffer::empty(Rect::new(0, 0, 21, 3));
        render(
            &[(Some(model(&[Some(0.5), None, Some(0.5)])), Style::default())],
            Scale::Percent,
            *buffer.area(),
            &mut buffer,
        );
        assert_eq!(buffer[(10, 1)].symbol(), " ");
    }

    #[test]
    fn network_shares_stable_bands_and_keeps_both_series() {
        let series = [
            (
                Some(model(&[Some(100.0); 3])),
                Style::default().fg(Color::Blue),
            ),
            (
                Some(model(&[Some(1800.0); 3])),
                Style::default().fg(Color::Green),
            ),
        ];
        assert_eq!(Scale::Network.bounds(&series, 60), (0.0, 2048.0));
        assert_eq!(
            Scale::Network.bounds(&[(Some(model(&[Some(2.0)])), Style::default())], 60),
            (0.0, 1024.0)
        );
        let mut buffer = Buffer::empty(Rect::new(0, 0, 20, 4));
        render(&series, Scale::Network, *buffer.area(), &mut buffer);
        for color in [Color::Blue, Color::Green] {
            assert!(
                buffer
                    .content
                    .iter()
                    .any(|cell| cell.fg == color && cell.symbol() != " ")
            );
        }
    }

    #[test]
    fn resizing_clips_to_region_and_never_changes_samples() {
        let chart = model(&[Some(0.0), Some(1.0), Some(0.0)]);
        for (width, height) in [(0, 0), (1, 1), (2, 1), (10, 2), (80, 8), (240, 40)] {
            let mut buffer = Buffer::filled(
                Rect::new(0, 0, width + 2, height + 2),
                ratatui::buffer::Cell::new("x"),
            );
            render(
                &[(Some(chart), Style::default())],
                Scale::Percent,
                Rect::new(1, 1, width, height),
                &mut buffer,
            );
            assert_eq!(buffer[(0, 0)].symbol(), "x");
            assert_eq!(chart.points().len(), 3);
        }
    }
    #[test]
    fn chart_height_uses_spare_rows_without_crossing_context_or_panel() {
        for (height, expected) in [(2, 0), (3, 1), (4, 2), (5, 3), (10, 3)] {
            let body = Rect::new(4, 6, 30, height);
            let chart = region(body, 2);
            assert_eq!(chart.height, expected);
            assert_eq!(chart.y, 8);
            assert!(chart.bottom() <= body.bottom());
            assert_eq!(chart.width, body.width);
        }
    }

    #[test]
    fn width_selects_a_recent_raw_window_and_resize_preserves_the_latest_value() {
        let values: Vec<_> = (0..120).map(|i| Some(i as f64)).collect();
        let chart = model(&values);
        assert_eq!(chart.sample_count(), 60);
        for (width, count) in [(10, 6), (30, 16), (60, 31), (120, 60), (240, 60)] {
            let samples: Vec<_> = chart.visible_samples(visible_sample_limit(width)).collect();
            assert_eq!(samples.len(), count);
            assert_eq!(samples.first(), Some(&Some((120 - count) as f64)));
            assert_eq!(samples.last(), Some(&Some(119.0)));
        }
        assert_eq!(chart.sample_count(), 60);
    }

    fn percent_bounds(values: &[Option<f64>], width: u16) -> (f64, f64) {
        Scale::Percent.bounds(
            &[(Some(model(values)), Style::default())],
            visible_sample_limit(width),
        )
    }

    #[test]
    fn constant_and_single_samples_keep_a_padded_minimum_span() {
        for value in [0.0, 0.01, 0.46, 0.5, 0.99, 1.0] {
            let bounds = percent_bounds(&[Some(value); 6], 30);
            assert_eq!(bounds, percent_bounds(&[Some(value)], 30));
            assert!(bounds.0 >= 0.0 && bounds.1 <= 1.0);
            assert!(bounds.0 <= value && bounds.1 >= value);
            assert!(bounds.1 - bounds.0 >= 0.10 - 1e-12);
            assert!(bounds.1 - bounds.0 <= 0.14 + 1e-12);
        }
        assert_eq!(percent_bounds(&[], 30), (0.0, 1.0));
        assert_eq!(percent_bounds(&[None], 30), (0.0, 1.0));
    }

    #[test]
    fn small_extrema_changes_share_quantized_bounds_and_noise_stays_small() {
        let baseline = percent_bounds(&[Some(0.46); 3], 25);
        for values in [
            [0.459, 0.460, 0.461],
            [0.458, 0.461, 0.462],
            [0.460, 0.462, 0.461],
        ] {
            let bounds = percent_bounds(&values.map(Some), 25);
            assert_eq!(bounds, baseline);
            assert!((0.462 - 0.458) / (bounds.1 - bounds.0) < 0.05);
        }
    }

    #[test]
    fn small_oscillations_gain_visible_height_without_modifying_samples() {
        let chart = model(&[Some(0.48), Some(0.52), Some(0.48)]);
        let before = chart;
        let mut buffer = Buffer::empty(Rect::new(0, 0, 25, 3));
        render(
            &[(Some(chart), Style::default())],
            Scale::Percent,
            *buffer.area(),
            &mut buffer,
        );
        let (low, high) = percent_bounds(&[Some(0.48), Some(0.52), Some(0.48)], 25);
        assert!((0.52 - 0.48) / (high - low) > 0.25);
        assert_ne!(mask(&buffer, 0, 1), mask(&buffer, 12, 1));
        assert_eq!(chart, before);
    }

    #[test]
    fn edge_windows_never_leave_the_percent_domain() {
        for values in [[0.0, 0.005, 0.01], [0.99, 0.995, 1.0]] {
            let (low, high) = percent_bounds(&values.map(Some), 25);
            assert!(low >= 0.0 && high <= 1.0);
            assert!(low <= values[0] && high >= values[2]);
            assert!(high - low >= 0.10 - 1e-12);
        }
        assert_eq!(
            percent_bounds(&[Some(0.0), Some(1.0), Some(0.0)], 25),
            (0.0, 1.0)
        );
    }

    #[test]
    fn isolated_peak_keeps_padding_and_is_not_clipped() {
        let values = [0.08, 0.10, 0.11, 0.09, 0.15, 0.12].map(Some);
        let (low, high) = percent_bounds(&values, 25);
        assert!(low <= 0.06 + 1e-12 && high >= 0.17);
        assert!(high - low < 0.20);
        let spike = [0.10, 0.11, 0.90, 0.12].map(Some);
        let (low, high) = percent_bounds(&spike, 25);
        assert!(low <= 0.10 && high >= 0.90);
        assert!((0.90 - 0.10) / (high - low) >= 0.80);
    }

    #[test]
    fn resize_uses_only_visible_samples_and_returns_the_same_scale_when_restored() {
        let chart = model(&[Some(0.95), None, Some(0.10), Some(0.11), Some(0.12)]);
        let series = [(Some(chart), Style::default())];
        let wide = Scale::Percent.bounds(&series, visible_sample_limit(30));
        let narrow = Scale::Percent.bounds(&series, visible_sample_limit(4));
        assert!(wide.1 >= 0.95);
        assert!(narrow.0 <= 0.10 && narrow.1 >= 0.12 && narrow.1 < 0.25);
        assert_eq!(
            wide,
            Scale::Percent.bounds(&series, visible_sample_limit(30))
        );
        assert_eq!(chart.sample_count(), 5);
    }

    #[test]
    fn two_and_three_row_charts_preserve_constant_extremes_and_full_peaks() {
        for height in [2, 3] {
            for value in [0.0, 0.5, 1.0] {
                let mut buffer = Buffer::empty(Rect::new(0, 0, 25, height));
                render(
                    &[(Some(model(&[Some(value); 6])), Style::default())],
                    Scale::Percent,
                    *buffer.area(),
                    &mut buffer,
                );
                assert_eq!(
                    buffer
                        .content
                        .iter()
                        .filter(|cell| cell.symbol() != " ")
                        .count(),
                    25
                );
            }
            let mut buffer = Buffer::empty(Rect::new(0, 0, 25, height));
            render(
                &[(
                    Some(model(&[Some(0.0), Some(1.0), Some(0.0)])),
                    Style::default(),
                )],
                Scale::Percent,
                *buffer.area(),
                &mut buffer,
            );
            assert_ne!(mask(&buffer, 0, height - 1) & 64, 0);
            assert_ne!(mask(&buffer, 12, 0) & 1, 0);
            assert_ne!(mask(&buffer, 24, height - 1) & 128, 0);
        }
    }
    type Point = (f64, f64);

    fn raster(segments: &[(Point, Point)], columns: usize, rows: usize) -> Vec<(usize, usize)> {
        let mut dots = Vec::new();
        for row in 0..rows {
            for left in (0..columns).step_by(CANVAS_CELLS) {
                let width = (columns - left).min(CANVAS_CELLS);
                let mut canvas = DotCanvas::new(left * 2, row * 4, width);
                for &(start, end) in segments {
                    canvas.segment(start, end, 0);
                }
                for x in 0..width * 2 {
                    for (y, bit) in BRAILLE_BITS[x % 2].iter().enumerate() {
                        if canvas.masks[x / 2] & bit != 0 {
                            dots.push((left * 2 + x, row * 4 + y));
                        }
                    }
                }
            }
        }
        dots
    }

    #[test]
    fn raster_horizontal_fills_both_dot_columns_of_each_braille_cell() {
        let dots = raster(&[((0.0, 2.0), (11.0, 2.0))], 6, 1);
        assert_eq!(dots, (0..12).map(|x| (x, 2)).collect::<Vec<_>>());
        let mut canvas = DotCanvas::new(0, 0, 1);
        canvas.segment((0.0, 0.0), (1.0, 3.0), 0);
        assert_eq!(canvas.masks[0], 1 | 2 | 32 | 128);
    }

    #[test]
    fn raster_ascending_diagonal_connects_all_logical_rows() {
        let dots = raster(&[((0.0, 7.0), (7.0, 0.0))], 4, 2);
        assert_eq!(dots.len(), 8);
        for i in 0..8 {
            assert!(dots.contains(&(i, 7 - i)));
        }
    }

    #[test]
    fn raster_descending_diagonal_connects_all_logical_rows() {
        let dots = raster(&[((0.0, 0.0), (7.0, 7.0))], 4, 2);
        assert_eq!(dots.len(), 8);
        for i in 0..8 {
            assert!(dots.contains(&(i, i)));
        }
    }

    #[test]
    fn raster_vertical_and_steep_lines_have_no_missing_rows() {
        for end_x in [2.0, 3.0] {
            let dots = raster(&[((2.0, 11.0), (end_x, 0.0))], 3, 3);
            assert_eq!(dots.len(), 12);
            for y in 0..12 {
                assert!(
                    dots.iter()
                        .any(|&(x, row)| row == y && (2..=3).contains(&x))
                );
            }
        }
    }

    #[test]
    fn equal_samples_form_a_line_and_a_zero_length_segment_is_one_dot() {
        assert_eq!(raster(&[((2.0, 2.0), (2.0, 2.0))], 3, 1), vec![(2, 2)]);
        let mut buffer = Buffer::empty(Rect::new(0, 0, 30, 3));
        render(
            &[(Some(model(&[Some(0.4), Some(0.4)])), Style::default())],
            Scale::Percent,
            *buffer.area(),
            &mut buffer,
        );
        assert_eq!(
            buffer
                .content
                .iter()
                .filter(|cell| cell.symbol() != " ")
                .count(),
            30
        );
    }

    #[test]
    fn zero_to_hundred_and_hundred_to_zero_reach_both_edges() {
        for values in [[0.0, 1.0], [1.0, 0.0]] {
            let mut buffer = Buffer::empty(Rect::new(0, 0, 30, 3));
            render(
                &[(Some(model(&values.map(Some))), Style::default())],
                Scale::Percent,
                *buffer.area(),
                &mut buffer,
            );
            let (first_row, last_row, first_bit, last_bit) = if values[0] == 0.0 {
                (2, 0, 64, 8)
            } else {
                (0, 2, 1, 128)
            };
            assert_ne!(mask(&buffer, 0, first_row) & first_bit, 0);
            assert_ne!(mask(&buffer, 29, last_row) & last_bit, 0);
            for x in 0..30 {
                assert!((0..3).any(|y| buffer[(x, y)].symbol() != " "));
            }
        }
    }

    #[test]
    fn consecutive_segments_and_canvas_strips_meet_without_seams() {
        let dots = raster(
            &[((0.0, 7.0), (511.0, 0.0)), ((511.0, 0.0), (519.0, 7.0))],
            260,
            2,
        );
        for x in 0..520 {
            assert!(dots.iter().any(|&(column, _)| column == x));
        }
        assert!(dots.contains(&(511, 0)));
        assert!(dots.contains(&(519, 7)));
        for x in 1..520 {
            let previous: Vec<_> = dots.iter().filter(|(column, _)| *column == x - 1).collect();
            assert!(
                dots.iter()
                    .filter(|(column, _)| *column == x)
                    .any(|(_, y)| previous.iter().any(|(_, py)| y.abs_diff(*py) <= 1))
            );
        }
    }

    #[test]
    fn fractional_vertices_are_not_rounded_before_rasterization() {
        let dots = raster(&[((0.0, 0.49), (10.0, 1.49))], 6, 1);
        assert!(dots.contains(&(0, 0)));
        assert!(dots.contains(&(1, 1))); // Early-rounded endpoints would still draw y=0 here.
        assert!(dots.contains(&(10, 1)));
    }

    #[test]
    fn one_column_chart_still_uses_both_horizontal_subpixels() {
        let dots = raster(&[((0.0, 3.0), (1.0, 0.0))], 1, 1);
        assert_eq!(dots.len(), 4);
        assert!(dots.contains(&(0, 3)) && dots.contains(&(1, 0)));
    }

    #[test]
    fn clipping_rejects_external_dots_and_preserves_original_geometry() {
        let dots = raster(
            &[((-10.0, 1.0), (30.0, 1.0)), ((-1.0, -1.0), (-1.0, 5.0))],
            5,
            1,
        );
        assert_eq!(dots, (0..10).map(|x| (x, 1)).collect::<Vec<_>>());
        assert!(raster(&[((0.0, -3.0), (10.0, -3.0))], 5, 1).is_empty());
        let chart = model(&[Some(0.0), Some(1.0), Some(0.0)]);
        let series = [(Some(chart), Style::default())];
        let viewport = Rect::new(0, 0, 40, 3);
        let mut full = Buffer::empty(viewport);
        let mut clipped = Buffer::empty(Rect::new(6, 1, 10, 2));
        render(&series, Scale::Percent, viewport, &mut full);
        render(&series, Scale::Percent, viewport, &mut clipped);
        for y in 1..3 {
            for x in 6..16 {
                assert_eq!(full[(x, y)].symbol(), clipped[(x, y)].symbol());
            }
        }
    }

    #[test]
    fn geometric_interpolation_does_not_add_or_modify_history_samples() {
        let mut history = TimeSeries::<120>::new(MetricUnit::Percent);
        for (i, value) in [0.10, 0.60, 0.25].into_iter().enumerate() {
            history
                .push(
                    TimeSample::new(
                        Duration::from_secs(i as u64),
                        Some(value),
                        MetricAvailability::Available,
                        None,
                    )
                    .unwrap(),
                )
                .unwrap();
        }
        let before = history;
        let chart = history.sparkline::<CHART_POINTS>(TimeWindow::Long);
        for width in [1, 2, 30, 300] {
            let mut buffer = Buffer::empty(Rect::new(0, 0, width, 3));
            render(
                &[(Some(chart), Style::default())],
                Scale::Percent,
                *buffer.area(),
                &mut buffer,
            );
            assert_eq!(history, before);
            assert_eq!(history.len(), 3);
            assert_eq!(chart.sample_count(), 3);
            assert_eq!(
                chart.visible_samples(60).collect::<Vec<_>>(),
                vec![Some(0.10), Some(0.60), Some(0.25)]
            );
        }
    }

    #[test]
    fn memory_minimum_range_keeps_one_percent_changes_proportional() {
        for values in [[0.40, 0.41], [0.0, 0.01], [0.99, 1.0]] {
            let series = [(Some(model(&values.map(Some))), Style::default())];
            let (low, high) = Scale::Memory.bounds(&series, 60);
            assert!(high - low >= 0.30 - 1e-12);
            assert!(low >= 0.0 && high <= 1.0);
            assert!(low <= values[0] && high >= values[1]);
            assert!((values[1] - values[0]) / (high - low) < 0.034);
        }
    }
}
