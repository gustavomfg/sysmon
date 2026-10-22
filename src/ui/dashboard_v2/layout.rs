use super::{
    DashboardBounds, DashboardComposition, DashboardContext, DashboardDimensions, DashboardError,
    DashboardMinimum, DashboardPreset, DashboardPriority, DashboardRegion, MAX_DASHBOARD_REGIONS,
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

    pub fn priority(self) -> DashboardPriority {
        self.region.priority()
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
        for section in self.sections() {
            let bounds = section.bounds();
            let minimum = section.region().minimum(self.preset);
            if bounds.is_empty()
                || bounds.width() < minimum.width()
                || bounds.height() < minimum.height()
                || !bounds.is_within(self.dimensions)
                || !self.composition.contains(section.region())
            {
                return Err(DashboardError::InvalidLayout);
            }
        }

        for region in DashboardRegion::ALL {
            if self.section(region).is_some() != self.composition.contains(region) {
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
        let mut sections = [None; MAX_DASHBOARD_REGIONS];
        let (header, body, footer) = chrome(dimensions, context)?;
        place(&mut sections, DashboardRegion::Header, header, context);
        place(&mut sections, DashboardRegion::Footer, footer, context);

        let body_regions = DashboardRegion::ALL
            .into_iter()
            .filter(|region| !matches!(region, DashboardRegion::Header | DashboardRegion::Footer))
            .filter(|region| context.includes(*region))
            .collect::<Vec<_>>();
        if let [region] = body_regions.as_slice() {
            // An expanded panel owns the body even when that region is not
            // part of the compact preset's normal multi-panel composition.
            place(&mut sections, *region, body, context);
        } else {
            match preset {
                DashboardPreset::Compact => compact_layout(&mut sections, body, preset, context)?,
                DashboardPreset::Standard => standard_layout(&mut sections, body, preset, context)?,
                DashboardPreset::Widescreen => {
                    widescreen_layout(&mut sections, body, preset, context)?
                }
                DashboardPreset::LargeWidescreen => {
                    large_widescreen_layout(&mut sections, body, preset, context)?
                }
            }
        }

        let composition = DashboardComposition::from_active(active_regions(&sections));
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
    preset: DashboardPreset,
    context: DashboardContext,
) -> Result<(), DashboardError> {
    let support = group_slot(
        1,
        36,
        [
            Some(DashboardRegion::PrimaryMetrics),
            Some(DashboardRegion::Alerts),
            None,
        ],
        Axis::Horizontal,
        preset,
        context,
    );
    let slots = [
        region_slot(0, DashboardRegion::Pulse, 64, preset, context),
        support,
        None,
        None,
        None,
        None,
        None,
        None,
    ];
    let areas = allocate_axis(body, Axis::Vertical, slots)?;
    if let Some(hero) = areas[0] {
        place(sections, DashboardRegion::Pulse, hero, context);
    }
    if let Some(support_bounds) = areas[1] {
        let support_slots = region_slots(
            [
                Some((DashboardRegion::PrimaryMetrics, 60)),
                Some((DashboardRegion::Alerts, 40)),
                None,
                None,
                None,
                None,
                None,
                None,
            ],
            preset,
            context,
        );
        let support_areas = allocate_axis(support_bounds, Axis::Horizontal, support_slots)?;
        place_regions(sections, support_areas, context);
    }
    Ok(())
}

fn standard_layout(
    sections: &mut [Option<DashboardSection>; MAX_DASHBOARD_REGIONS],
    body: DashboardBounds,
    preset: DashboardPreset,
    context: DashboardContext,
) -> Result<(), DashboardError> {
    three_column_layout(
        sections,
        body,
        preset,
        context,
        [30, 42, 28],
        [70, 30],
        [34, 38, 28],
    )
}

fn widescreen_layout(
    sections: &mut [Option<DashboardSection>; MAX_DASHBOARD_REGIONS],
    body: DashboardBounds,
    preset: DashboardPreset,
    context: DashboardContext,
) -> Result<(), DashboardError> {
    three_column_layout(
        sections,
        body,
        preset,
        context,
        [28, 44, 28],
        [72, 28],
        [34, 38, 28],
    )
}

fn large_widescreen_layout(
    sections: &mut [Option<DashboardSection>; MAX_DASHBOARD_REGIONS],
    body: DashboardBounds,
    preset: DashboardPreset,
    context: DashboardContext,
) -> Result<(), DashboardError> {
    three_column_layout(
        sections,
        body,
        preset,
        context,
        [32, 40, 28],
        [74, 26],
        [34, 38, 28],
    )
}

fn three_column_layout(
    sections: &mut [Option<DashboardSection>; MAX_DASHBOARD_REGIONS],
    body: DashboardBounds,
    preset: DashboardPreset,
    context: DashboardContext,
    column_weights: [usize; 3],
    center_weights: [usize; 2],
    right_weights: [usize; 3],
) -> Result<(), DashboardError> {
    let left = group_slot(
        0,
        column_weights[0],
        [Some(DashboardRegion::PrimaryMetrics)],
        Axis::Horizontal,
        preset,
        context,
    );
    let center = group_slot(
        1,
        column_weights[1],
        [
            Some(DashboardRegion::Pulse),
            Some(DashboardRegion::SecondaryMetrics),
        ],
        Axis::Vertical,
        preset,
        context,
    );
    let right = group_slot(
        2,
        column_weights[2],
        [
            Some(DashboardRegion::Charts),
            Some(DashboardRegion::Processes),
            Some(DashboardRegion::Alerts),
        ],
        Axis::Vertical,
        preset,
        context,
    );
    let columns = allocate_axis(
        body,
        Axis::Horizontal,
        [left, center, right, None, None, None, None, None],
    )?;

    if let Some(left_bounds) = columns[0] {
        let left_slots = region_slots(
            [
                Some((DashboardRegion::PrimaryMetrics, 100)),
                None,
                None,
                None,
                None,
                None,
                None,
                None,
            ],
            preset,
            context,
        );
        place_regions(
            sections,
            allocate_axis(left_bounds, Axis::Vertical, left_slots)?,
            context,
        );
    }

    if let Some(center_bounds) = columns[1] {
        let center_slots = region_slots(
            [
                Some((DashboardRegion::Pulse, center_weights[0])),
                Some((DashboardRegion::SecondaryMetrics, center_weights[1])),
                None,
                None,
                None,
                None,
                None,
                None,
            ],
            preset,
            context,
        );
        place_regions(
            sections,
            allocate_axis(center_bounds, Axis::Vertical, center_slots)?,
            context,
        );
    }

    if let Some(right_bounds) = columns[2] {
        let right_slots = region_slots(
            [
                Some((DashboardRegion::Charts, right_weights[0])),
                Some((DashboardRegion::Processes, right_weights[1])),
                Some((DashboardRegion::Alerts, right_weights[2])),
                None,
                None,
                None,
                None,
                None,
            ],
            preset,
            context,
        );
        place_regions(
            sections,
            allocate_axis(right_bounds, Axis::Vertical, right_slots)?,
            context,
        );
    }
    Ok(())
}

fn chrome(
    dimensions: DashboardDimensions,
    context: DashboardContext,
) -> Result<(DashboardBounds, DashboardBounds, DashboardBounds), DashboardError> {
    let header_height = if context.includes(DashboardRegion::Header) {
        match dimensions.height() {
            0 => 0,
            1..=3 => 1,
            _ => 2,
        }
    } else {
        0
    };
    let footer_height = usize::from(
        context.includes(DashboardRegion::Footer) && dimensions.height() > header_height,
    );
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Axis {
    Horizontal,
    Vertical,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct LayoutSlot {
    key: usize,
    weight: usize,
    minimum: DashboardMinimum,
    priority: DashboardPriority,
}

fn region_slot(
    key: usize,
    region: DashboardRegion,
    weight: usize,
    preset: DashboardPreset,
    context: DashboardContext,
) -> Option<LayoutSlot> {
    context.includes(region).then_some(LayoutSlot {
        key,
        weight,
        minimum: region.minimum(preset),
        priority: region.priority(),
    })
}

fn region_slots(
    regions: [Option<(DashboardRegion, usize)>; MAX_DASHBOARD_REGIONS],
    preset: DashboardPreset,
    context: DashboardContext,
) -> [Option<LayoutSlot>; MAX_DASHBOARD_REGIONS] {
    let mut slots = [None; MAX_DASHBOARD_REGIONS];
    for (index, region) in regions.into_iter().enumerate() {
        if let Some((region, weight)) = region
            && context.includes(region)
        {
            slots[index] = Some(LayoutSlot {
                key: region.index(),
                weight,
                minimum: region.minimum(preset),
                priority: region.priority(),
            });
        }
    }
    slots
}

fn group_slot<const N: usize>(
    key: usize,
    weight: usize,
    regions: [Option<DashboardRegion>; N],
    axis: Axis,
    preset: DashboardPreset,
    context: DashboardContext,
) -> Option<LayoutSlot> {
    let active = regions
        .into_iter()
        .flatten()
        .filter(|region| context.includes(*region))
        .collect::<NoAllocRegions>();
    if active.is_empty() {
        return None;
    }

    let mut minimum = DashboardMinimum::new(0, 0);
    let mut priority = DashboardPriority::Footer;
    for region in active.iter() {
        let region_minimum = region.minimum(preset);
        minimum = DashboardMinimum::new(
            match axis {
                Axis::Horizontal => minimum.width() + region_minimum.width(),
                Axis::Vertical => minimum.width().max(region_minimum.width()),
            },
            match axis {
                Axis::Horizontal => minimum.height().max(region_minimum.height()),
                Axis::Vertical => minimum.height() + region_minimum.height(),
            },
        );
        priority = priority.min(region.priority());
    }
    Some(LayoutSlot {
        key,
        weight,
        minimum,
        priority,
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct NoAllocRegions {
    regions: [Option<DashboardRegion>; MAX_DASHBOARD_REGIONS],
    length: u8,
}

impl NoAllocRegions {
    fn is_empty(self) -> bool {
        self.length == 0
    }

    fn iter(self) -> impl Iterator<Item = DashboardRegion> {
        self.regions
            .into_iter()
            .take(usize::from(self.length))
            .flatten()
    }
}

impl FromIterator<DashboardRegion> for NoAllocRegions {
    fn from_iter<T: IntoIterator<Item = DashboardRegion>>(iter: T) -> Self {
        let mut result = Self {
            regions: [None; MAX_DASHBOARD_REGIONS],
            length: 0,
        };
        for region in iter {
            if usize::from(result.length) == MAX_DASHBOARD_REGIONS {
                break;
            }
            result.regions[usize::from(result.length)] = Some(region);
            result.length += 1;
        }
        result
    }
}

fn allocate_axis(
    bounds: DashboardBounds,
    axis: Axis,
    slots: [Option<LayoutSlot>; MAX_DASHBOARD_REGIONS],
) -> Result<[Option<DashboardBounds>; MAX_DASHBOARD_REGIONS], DashboardError> {
    let mut active = slots;
    let available = match axis {
        Axis::Horizontal => bounds.width(),
        Axis::Vertical => bounds.height(),
    };

    loop {
        let mut total_minimum: usize = 0;
        let mut incompatible = false;
        for slot in active.into_iter().flatten() {
            let cross_fits = match axis {
                Axis::Horizontal => slot.minimum.height() <= bounds.height(),
                Axis::Vertical => slot.minimum.width() <= bounds.width(),
            };
            if !cross_fits {
                incompatible = true;
            } else {
                total_minimum = total_minimum
                    .checked_add(axis_minimum(slot.minimum, axis))
                    .ok_or(DashboardError::AreaTooLarge)?;
            }
        }

        if !incompatible && total_minimum <= available {
            break;
        }
        let Some(drop_index) = lowest_priority_index(active) else {
            break;
        };
        active[drop_index] = None;
    }

    let mut result = [None; MAX_DASHBOARD_REGIONS];
    let mut total_minimum: usize = 0;
    let mut weight_sum: usize = 0;
    let mut count = 0;
    for slot in active.into_iter().flatten() {
        total_minimum = total_minimum
            .checked_add(axis_minimum(slot.minimum, axis))
            .ok_or(DashboardError::AreaTooLarge)?;
        weight_sum = weight_sum
            .checked_add(slot.weight.max(1))
            .ok_or(DashboardError::AreaTooLarge)?;
        count += 1;
    }
    if count == 0 || total_minimum > available {
        return Ok(result);
    }

    let remaining = available - total_minimum;
    let mut distributed_extra = 0;
    let mut seen = 0;
    let mut offset = 0;
    for slot in active.into_iter().flatten() {
        seen += 1;
        let extra = if seen == count {
            remaining - distributed_extra
        } else {
            remaining
                .checked_mul(slot.weight.max(1))
                .ok_or(DashboardError::AreaTooLarge)?
                / weight_sum
        };
        distributed_extra += extra;
        let span = axis_minimum(slot.minimum, axis) + extra;
        let section_bounds = match axis {
            Axis::Horizontal => {
                DashboardBounds::new(bounds.x() + offset, bounds.y(), span, bounds.height())?
            }
            Axis::Vertical => {
                DashboardBounds::new(bounds.x(), bounds.y() + offset, bounds.width(), span)?
            }
        };
        result[slot.key] = Some(section_bounds);
        offset += span;
    }
    Ok(result)
}

fn axis_minimum(minimum: DashboardMinimum, axis: Axis) -> usize {
    match axis {
        Axis::Horizontal => minimum.width(),
        Axis::Vertical => minimum.height(),
    }
}

fn lowest_priority_index(slots: [Option<LayoutSlot>; MAX_DASHBOARD_REGIONS]) -> Option<usize> {
    slots
        .into_iter()
        .enumerate()
        .filter_map(|(index, slot)| slot.map(|slot| (index, slot.priority)))
        .max_by_key(|(_, priority)| *priority)
        .map(|(index, _)| index)
}

fn place_regions(
    sections: &mut [Option<DashboardSection>; MAX_DASHBOARD_REGIONS],
    bounds: [Option<DashboardBounds>; MAX_DASHBOARD_REGIONS],
    context: DashboardContext,
) {
    for region in DashboardRegion::ALL {
        if let Some(region_bounds) = bounds[region.index()] {
            place(sections, region, region_bounds, context);
        }
    }
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

fn active_regions(
    sections: &[Option<DashboardSection>; MAX_DASHBOARD_REGIONS],
) -> [bool; MAX_DASHBOARD_REGIONS] {
    let mut active = [false; MAX_DASHBOARD_REGIONS];
    for section in sections.iter().flatten() {
        active[section.region().index()] = true;
    }
    active
}
