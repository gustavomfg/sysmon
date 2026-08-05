use super::{
    DashboardBounds, DashboardComposition, DashboardContext, DashboardDimensions, DashboardError,
    DashboardPreset, DashboardRegion, MAX_DASHBOARD_REGIONS,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DashboardSection {
    region: DashboardRegion,
    bounds: DashboardBounds,
}

impl DashboardSection {
    pub fn region(self) -> DashboardRegion {
        self.region
    }

    pub fn bounds(self) -> DashboardBounds {
        self.bounds
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DashboardLayout {
    dimensions: DashboardDimensions,
    preset: DashboardPreset,
    context: DashboardContext,
    composition: DashboardComposition,
    sections: [Option<DashboardSection>; MAX_DASHBOARD_REGIONS],
}

impl DashboardLayout {
    pub fn dimensions(self) -> DashboardDimensions {
        self.dimensions
    }

    pub fn preset(self) -> DashboardPreset {
        self.preset
    }

    pub fn context(self) -> DashboardContext {
        self.context
    }

    pub fn composition(self) -> DashboardComposition {
        self.composition
    }

    pub fn section(self, region: DashboardRegion) -> Option<DashboardSection> {
        self.sections[region.index()]
    }

    pub fn sections(self) -> impl Iterator<Item = DashboardSection> {
        self.sections.into_iter().flatten()
    }

    pub fn validate(self) -> Result<(), DashboardError> {
        for section in self
            .sections()
            .filter(|section| !section.bounds().is_empty())
        {
            if !section.bounds().is_within(self.dimensions) {
                return Err(DashboardError::InvalidLayout);
            }
        }

        let sections = self.sections;
        for (index, first) in sections.iter().enumerate() {
            let Some(first) = first else { continue };
            for second in sections.iter().skip(index + 1).flatten() {
                if first.bounds().overlaps(second.bounds()) {
                    return Err(DashboardError::InvalidLayout);
                }
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct DashboardLayoutEngine;

impl DashboardLayoutEngine {
    pub fn new() -> Self {
        Self
    }

    pub fn compose(
        self,
        dimensions: DashboardDimensions,
        preset: DashboardPreset,
        context: DashboardContext,
    ) -> Result<DashboardLayout, DashboardError> {
        let composition = DashboardComposition::for_preset(preset, context);
        let mut sections = [None; MAX_DASHBOARD_REGIONS];
        let (header, body, footer) = chrome(dimensions)?;
        place(&mut sections, DashboardRegion::Header, header, context);
        place(&mut sections, DashboardRegion::Footer, footer, context);

        match preset {
            DashboardPreset::Compact => compact_layout(&mut sections, body, context)?,
            DashboardPreset::Standard => standard_layout(&mut sections, body, context)?,
            DashboardPreset::Widescreen => widescreen_layout(&mut sections, body, context)?,
            DashboardPreset::LargeWidescreen => {
                large_widescreen_layout(&mut sections, body, context)?
            }
        }

        let layout = DashboardLayout {
            dimensions,
            preset,
            context,
            composition,
            sections,
        };
        layout.validate()?;
        Ok(layout)
    }
}

fn compact_layout(
    sections: &mut [Option<DashboardSection>; MAX_DASHBOARD_REGIONS],
    body: DashboardBounds,
    context: DashboardContext,
) -> Result<(), DashboardError> {
    let (hero, support) = split_vertical(body, 62, 100)?;
    place(sections, DashboardRegion::Pulse, hero, context);
    let (primary, secondary) = split_horizontal(support, 60, 100)?;
    place(sections, DashboardRegion::PrimaryMetrics, primary, context);
    place(
        sections,
        DashboardRegion::SecondaryMetrics,
        secondary,
        context,
    );
    Ok(())
}

fn standard_layout(
    sections: &mut [Option<DashboardSection>; MAX_DASHBOARD_REGIONS],
    body: DashboardBounds,
    context: DashboardContext,
) -> Result<(), DashboardError> {
    let (top, bottom) = split_vertical(body, 58, 100)?;
    let (pulse, primary) = split_horizontal(top, 52, 100)?;
    place(sections, DashboardRegion::Pulse, pulse, context);
    place(sections, DashboardRegion::PrimaryMetrics, primary, context);

    let (secondary, side) = split_horizontal(bottom, 54, 100)?;
    place(
        sections,
        DashboardRegion::SecondaryMetrics,
        secondary,
        context,
    );
    let (charts, alerts) = split_vertical(side, 68, 100)?;
    place(sections, DashboardRegion::Charts, charts, context);
    place(sections, DashboardRegion::Alerts, alerts, context);
    Ok(())
}

fn widescreen_layout(
    sections: &mut [Option<DashboardSection>; MAX_DASHBOARD_REGIONS],
    body: DashboardBounds,
    context: DashboardContext,
) -> Result<(), DashboardError> {
    let (left, center_right) = split_horizontal(body, 26, 100)?;
    let (center, right) = split_horizontal(center_right, 66, 100)?;
    let (primary, secondary) = split_vertical(left, 52, 100)?;
    place(sections, DashboardRegion::PrimaryMetrics, primary, context);
    place(
        sections,
        DashboardRegion::SecondaryMetrics,
        secondary,
        context,
    );
    place(sections, DashboardRegion::Pulse, center, context);

    let (charts, lower_right) = split_vertical(right, 38, 100)?;
    let (processes, alerts) = split_vertical(lower_right, 58, 100)?;
    place(sections, DashboardRegion::Charts, charts, context);
    place(sections, DashboardRegion::Processes, processes, context);
    place(sections, DashboardRegion::Alerts, alerts, context);
    Ok(())
}

fn large_widescreen_layout(
    sections: &mut [Option<DashboardSection>; MAX_DASHBOARD_REGIONS],
    body: DashboardBounds,
    context: DashboardContext,
) -> Result<(), DashboardError> {
    let (hero, alerts) = split_vertical(body, 82, 100)?;
    let (left, center_right) = split_horizontal(hero, 22, 100)?;
    let (center, right) = split_horizontal(center_right, 72, 100)?;
    let (primary, secondary) = split_vertical(left, 48, 100)?;
    place(sections, DashboardRegion::PrimaryMetrics, primary, context);
    place(
        sections,
        DashboardRegion::SecondaryMetrics,
        secondary,
        context,
    );
    place(sections, DashboardRegion::Pulse, center, context);

    let (charts, processes) = split_vertical(right, 54, 100)?;
    place(sections, DashboardRegion::Charts, charts, context);
    place(sections, DashboardRegion::Processes, processes, context);
    place(sections, DashboardRegion::Alerts, alerts, context);
    Ok(())
}

fn chrome(
    dimensions: DashboardDimensions,
) -> Result<(DashboardBounds, DashboardBounds, DashboardBounds), DashboardError> {
    let header_height = match dimensions.height() {
        0 => 0,
        1..=3 => 1,
        _ => 2,
    };
    let footer_height = usize::from(dimensions.height() > header_height);
    let body_height = dimensions.height() - header_height - footer_height;
    Ok((
        DashboardBounds::new(0, 0, dimensions.width(), header_height)?,
        DashboardBounds::new(0, header_height, dimensions.width(), body_height)?,
        DashboardBounds::new(
            0,
            dimensions.height() - footer_height,
            dimensions.width(),
            footer_height,
        )?,
    ))
}

fn split_horizontal(
    bounds: DashboardBounds,
    numerator: usize,
    denominator: usize,
) -> Result<(DashboardBounds, DashboardBounds), DashboardError> {
    let first_width = weighted(bounds.width(), numerator, denominator)?;
    let first = DashboardBounds::new(bounds.x(), bounds.y(), first_width, bounds.height())?;
    let second = DashboardBounds::new(
        bounds.x() + first_width,
        bounds.y(),
        bounds.width() - first_width,
        bounds.height(),
    )?;
    Ok((first, second))
}

fn split_vertical(
    bounds: DashboardBounds,
    numerator: usize,
    denominator: usize,
) -> Result<(DashboardBounds, DashboardBounds), DashboardError> {
    let first_height = weighted(bounds.height(), numerator, denominator)?;
    let first = DashboardBounds::new(bounds.x(), bounds.y(), bounds.width(), first_height)?;
    let second = DashboardBounds::new(
        bounds.x(),
        bounds.y() + first_height,
        bounds.width(),
        bounds.height() - first_height,
    )?;
    Ok((first, second))
}

fn weighted(value: usize, numerator: usize, denominator: usize) -> Result<usize, DashboardError> {
    if denominator == 0 || numerator > denominator {
        return Err(DashboardError::InvalidLayout);
    }
    value
        .checked_mul(numerator)
        .map(|weighted| weighted / denominator)
        .ok_or(DashboardError::AreaTooLarge)
}

fn place(
    sections: &mut [Option<DashboardSection>; MAX_DASHBOARD_REGIONS],
    region: DashboardRegion,
    bounds: DashboardBounds,
    context: DashboardContext,
) {
    if context.includes(region) && !bounds.is_empty() {
        sections[region.index()] = Some(DashboardSection { region, bounds });
    }
}
