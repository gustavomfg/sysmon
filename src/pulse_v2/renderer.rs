use super::types::MAX_FIELD_SAMPLES;
use super::{FieldDimensions, FieldError, LightingField, SemanticColorField};
use ratatui::style::Color;

const BRAILLE_DOT_BITS: [u8; 8] = [1, 2, 4, 8, 16, 32, 64, 128];
const BRAILLE_DOT_OFFSETS: [(usize, usize); 8] = [
    (0, 0),
    (0, 1),
    (0, 2),
    (1, 0),
    (1, 1),
    (1, 2),
    (0, 3),
    (1, 3),
];
const UNICODE_SHADES: [char; 5] = [' ', '·', '•', '▪', '■'];

/// The terminal-native representation selected by the caller.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RasterMode {
    Braille,
    Unicode,
}

/// Dimensions of the produced character grid.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RasterDimensions {
    width: usize,
    height: usize,
}

impl RasterDimensions {
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

/// A terminal-independent character frame with no styles or colors.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RasterFrame {
    dimensions: RasterDimensions,
    mode: RasterMode,
    cells: Vec<char>,
}

impl RasterFrame {
    fn try_new(dimensions: RasterDimensions, mode: RasterMode) -> Result<Self, FieldError> {
        let sample_count =
            dimensions
                .width
                .checked_mul(dimensions.height)
                .ok_or(FieldError::Dimensions(
                    super::DimensionsError::SampleCountOverflow,
                ))?;
        if sample_count > MAX_FIELD_SAMPLES {
            return Err(FieldError::Dimensions(
                super::DimensionsError::SampleCountTooLarge,
            ));
        }

        Ok(Self {
            dimensions,
            mode,
            cells: vec![' '; sample_count],
        })
    }

    pub fn dimensions(&self) -> RasterDimensions {
        self.dimensions
    }

    pub fn mode(&self) -> RasterMode {
        self.mode
    }

    pub fn width(&self) -> usize {
        self.dimensions.width
    }

    pub fn height(&self) -> usize {
        self.dimensions.height
    }

    pub fn is_empty(&self) -> bool {
        self.cells.is_empty()
    }

    pub fn as_slice(&self) -> &[char] {
        &self.cells
    }

    pub fn read(&self, x: usize, y: usize) -> Option<char> {
        if x >= self.width() || y >= self.height() {
            return None;
        }
        Some(self.cells[y * self.width() + x])
    }

    /// Returns rows separated by newlines for a future terminal adapter.
    pub fn to_text(&self) -> String {
        let mut text = String::new();
        for y in 0..self.height() {
            if y > 0 {
                text.push('\n');
            }
            for x in 0..self.width() {
                if let Some(character) = self.read(x, y) {
                    text.push(character);
                }
            }
        }
        text
    }
}

/// One terminal cell after semantic foreground color has been applied.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ColorRasterCell {
    character: char,
    foreground: Color,
}

impl ColorRasterCell {
    pub fn character(self) -> char {
        self.character
    }

    pub fn foreground(self) -> Color {
        self.foreground
    }
}

/// Terminal-native content paired with semantic foreground colors.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ColorRasterFrame {
    dimensions: RasterDimensions,
    mode: RasterMode,
    cells: Vec<ColorRasterCell>,
}

impl ColorRasterFrame {
    fn try_new(dimensions: RasterDimensions, mode: RasterMode) -> Result<Self, FieldError> {
        let sample_count =
            dimensions
                .width
                .checked_mul(dimensions.height)
                .ok_or(FieldError::Dimensions(
                    super::DimensionsError::SampleCountOverflow,
                ))?;
        if sample_count > MAX_FIELD_SAMPLES {
            return Err(FieldError::Dimensions(
                super::DimensionsError::SampleCountTooLarge,
            ));
        }

        Ok(Self {
            dimensions,
            mode,
            cells: vec![
                ColorRasterCell {
                    character: ' ',
                    foreground: Color::Reset,
                };
                sample_count
            ],
        })
    }

    pub fn dimensions(&self) -> RasterDimensions {
        self.dimensions
    }

    pub fn mode(&self) -> RasterMode {
        self.mode
    }

    pub fn width(&self) -> usize {
        self.dimensions.width
    }

    pub fn height(&self) -> usize {
        self.dimensions.height
    }

    pub fn is_empty(&self) -> bool {
        self.cells.is_empty()
    }

    pub fn as_slice(&self) -> &[ColorRasterCell] {
        &self.cells
    }

    pub fn read(&self, x: usize, y: usize) -> Option<ColorRasterCell> {
        if x >= self.width() || y >= self.height() {
            return None;
        }
        Some(self.cells[y * self.width() + x])
    }
}

/// Converts perceptual lighting into terminal-native characters.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct PulseV2Rasterizer;

impl PulseV2Rasterizer {
    pub fn render(field: &LightingField, mode: RasterMode) -> Result<RasterFrame, FieldError> {
        match mode {
            RasterMode::Braille => Self::braille(field),
            RasterMode::Unicode => Self::unicode(field),
        }
    }

    pub fn braille(field: &LightingField) -> Result<RasterFrame, FieldError> {
        let dimensions = braille_dimensions(field.dimensions());
        let mut frame = RasterFrame::try_new(dimensions, RasterMode::Braille)?;

        for cell_y in 0..dimensions.height {
            for cell_x in 0..dimensions.width {
                frame.cells[cell_y * dimensions.width + cell_x] =
                    braille_cell(field, cell_x, cell_y);
            }
        }

        Ok(frame)
    }

    pub fn unicode(field: &LightingField) -> Result<RasterFrame, FieldError> {
        let dimensions = RasterDimensions {
            width: field.width(),
            height: field.height(),
        };
        let mut frame = RasterFrame::try_new(dimensions, RasterMode::Unicode)?;

        for y in 0..dimensions.height {
            for x in 0..dimensions.width {
                let intensity = field
                    .read(x, y)
                    .map(|sample| sample.intensity())
                    .unwrap_or(0.0);
                frame.cells[y * dimensions.width + x] = unicode_shade(intensity);
            }
        }

        Ok(frame)
    }

    pub fn braille_colored(
        field: &LightingField,
        colors: &SemanticColorField,
    ) -> Result<ColorRasterFrame, FieldError> {
        if field.dimensions() != colors.dimensions() {
            return Err(FieldError::DimensionsMismatch);
        }

        let dimensions = braille_dimensions(field.dimensions());
        let mut frame = ColorRasterFrame::try_new(dimensions, RasterMode::Braille)?;

        for cell_y in 0..dimensions.height {
            for cell_x in 0..dimensions.width {
                let (character, foreground) = braille_cell_colored(field, colors, cell_x, cell_y);
                frame.cells[cell_y * dimensions.width + cell_x] = ColorRasterCell {
                    character,
                    foreground,
                };
            }
        }

        Ok(frame)
    }

    pub fn unicode_colored(
        field: &LightingField,
        colors: &SemanticColorField,
    ) -> Result<ColorRasterFrame, FieldError> {
        if field.dimensions() != colors.dimensions() {
            return Err(FieldError::DimensionsMismatch);
        }

        let dimensions = RasterDimensions {
            width: field.width(),
            height: field.height(),
        };
        let mut frame = ColorRasterFrame::try_new(dimensions, RasterMode::Unicode)?;

        for y in 0..dimensions.height {
            for x in 0..dimensions.width {
                let intensity = field
                    .read(x, y)
                    .map(|sample| sample.intensity())
                    .unwrap_or(0.0);
                let foreground = colors
                    .read(x, y)
                    .map(|sample| sample.color())
                    .unwrap_or(Color::Reset);
                frame.cells[y * dimensions.width + x] = ColorRasterCell {
                    character: unicode_shade(intensity),
                    foreground,
                };
            }
        }

        Ok(frame)
    }
}

fn braille_dimensions(field_dimensions: FieldDimensions) -> RasterDimensions {
    RasterDimensions {
        width: field_dimensions.width().saturating_add(1) / 2,
        height: field_dimensions.height().saturating_add(3) / 4,
    }
}

fn braille_cell(field: &LightingField, cell_x: usize, cell_y: usize) -> char {
    let mut scores = [0.0; 8];
    let mut total = 0.0;
    let mut active = false;

    for (index, (offset_x, offset_y)) in BRAILLE_DOT_OFFSETS.iter().copied().enumerate() {
        let intensity = field
            .read(cell_x * 2 + offset_x, cell_y * 4 + offset_y)
            .map(|sample| sample.intensity())
            .unwrap_or(0.0)
            .clamp(0.0, 1.0);
        let score = perceptual_dot_weight(intensity);
        scores[index] = score;
        total += score;
        active |= intensity > f32::EPSILON;
    }

    if !active {
        return ' ';
    }

    let requested_dots = total.round() as usize;
    let dot_count = requested_dots.max(1).min(BRAILLE_DOT_BITS.len());
    let mut order = [0usize; 8];
    for (index, value) in order.iter_mut().enumerate() {
        *value = index;
    }
    order.sort_unstable_by(|left, right| {
        scores[*right]
            .total_cmp(&scores[*left])
            .then_with(|| left.cmp(right))
    });

    let mut bits = 0u8;
    for index in order.into_iter().take(dot_count) {
        if scores[index] > f32::EPSILON {
            bits |= BRAILLE_DOT_BITS[index];
        }
    }

    char::from_u32(0x2800 + u32::from(bits)).unwrap_or(' ')
}

fn braille_cell_colored(
    field: &LightingField,
    colors: &SemanticColorField,
    cell_x: usize,
    cell_y: usize,
) -> (char, Color) {
    let mut scores = [0.0; 8];
    let mut total = 0.0;
    let mut active = false;
    let mut strongest_intensity = 0.0;
    let mut strongest_index = 0;

    for (index, (offset_x, offset_y)) in BRAILLE_DOT_OFFSETS.iter().copied().enumerate() {
        let x = cell_x * 2 + offset_x;
        let y = cell_y * 4 + offset_y;
        let intensity = field
            .read(x, y)
            .map(|sample| sample.intensity())
            .unwrap_or(0.0);
        let score = perceptual_dot_weight(intensity);
        scores[index] = score;
        total += score;
        active |= intensity > f32::EPSILON;
        if intensity > strongest_intensity {
            strongest_intensity = intensity;
            strongest_index = index;
        }
    }

    let fallback_color = colors
        .read(cell_x * 2, cell_y * 4)
        .map(|sample| sample.color())
        .unwrap_or(Color::Reset);
    if !active {
        return (' ', fallback_color);
    }

    let requested_dots = total.round() as usize;
    let dot_count = requested_dots.max(1).min(BRAILLE_DOT_BITS.len());
    let mut order = [0usize; 8];
    for (index, value) in order.iter_mut().enumerate() {
        *value = index;
    }
    order.sort_unstable_by(|left, right| {
        scores[*right]
            .total_cmp(&scores[*left])
            .then_with(|| left.cmp(right))
    });

    let mut bits = 0u8;
    for index in order.into_iter().take(dot_count) {
        if scores[index] > f32::EPSILON {
            bits |= BRAILLE_DOT_BITS[index];
        }
    }
    let character = char::from_u32(0x2800 + u32::from(bits)).unwrap_or(' ');
    let (offset_x, offset_y) = BRAILLE_DOT_OFFSETS[strongest_index];
    let foreground = colors
        .read(cell_x * 2 + offset_x, cell_y * 4 + offset_y)
        .map(|sample| sample.color())
        .unwrap_or(fallback_color);

    (character, foreground)
}

fn perceptual_dot_weight(intensity: f32) -> f32 {
    if intensity <= f32::EPSILON {
        return 0.0;
    }

    intensity.clamp(0.0, 1.0).powf(0.78)
}

fn unicode_shade(intensity: f32) -> char {
    if intensity <= f32::EPSILON {
        return UNICODE_SHADES[0];
    }

    let perceptual = intensity.clamp(0.0, 1.0).powf(0.78);
    let level = (1.0 + perceptual * 3.0).floor() as usize;
    UNICODE_SHADES[level.min(UNICODE_SHADES.len() - 1)]
}
