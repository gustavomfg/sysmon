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

        match preset {
            DashboardPreset::Compact => compact_layout(&mut sections, body, preset, context)?,
            DashboardPreset::Standard => standard_layout(&mut sections, body, preset, context)?,
            DashboardPreset::Widescreen => widescreen_layout(&mut sections, body, preset, context)?,
            DashboardPreset::LargeWidescreen => {
                large_widescreen_layout(&mut sections, body, preset, context)?
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
    let top_slots = region_slots(
        [
            Some((DashboardRegion::Pulse, 58)),
            Some((DashboardRegion::PrimaryMetrics, 42)),
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
    let side = group_slot(
        1,
        54,
        [
            Some(DashboardRegion::Charts),
            Some(DashboardRegion::Processes),
            Some(DashboardRegion::Alerts),
        ],
        Axis::Vertical,
        preset,
        context,
    );
    let bottom_slots = [
        region_slot(0, DashboardRegion::SecondaryMetrics, 46, preset, context),
        side,
        None,
        None,
        None,
        None,
        None,
        None,
    ];
    let rows = row_slots(body, 58, 42, top_slots, bottom_slots)?;
    place_regions(sections, rows[0], context);
    if let Some(secondary_bounds) = rows[1][0] {
        place(
            sections,
            DashboardRegion::SecondaryMetrics,
            secondary_bounds,
            context,
        );
    }
    if let Some(side_bounds) = rows[1][1] {
        let side_slots = region_slots(
            [
                Some((DashboardRegion::Charts, 50)),
                Some((DashboardRegion::Processes, 30)),
                Some((DashboardRegion::Alerts, 20)),
                None,
                None,
                None,
                None,
                None,
            ],
            preset,
            context,
        );
        let side_areas = allocate_axis(side_bounds, Axis::Vertical, side_slots)?;
        place_regions(sections, side_areas, context);
    }
    Ok(())
}

fn widescreen_layout(
    sections: &mut [Option<DashboardSection>; MAX_DASHBOARD_REGIONS],
    body: DashboardBounds,
    preset: DashboardPreset,
    context: DashboardContext,
) -> Result<(), DashboardError> {
    let left = group_slot(
        0,
        26,
        [
            Some(DashboardRegion::PrimaryMetrics),
            Some(DashboardRegion::SecondaryMetrics),
            None,
        ],
        Axis::Vertical,
        preset,
        context,
    );
    let right = group_slot(
        2,
        24,
        [
            Some(DashboardRegion::Charts),
            Some(DashboardRegion::Processes),
            Some(DashboardRegion::Alerts),
        ],
        Axis::Vertical,
        preset,
        context,
    );
    let slots = [
        left,
        region_slot(1, DashboardRegion::Pulse, 50, preset, context),
        right,
        None,
        None,
        None,
        None,
        None,
    ];
    let columns = allocate_axis(body, Axis::Horizontal, slots)?;

    if let Some(left_bounds) = columns[0] {
        let left_slots = region_slots(
            [
                Some((DashboardRegion::PrimaryMetrics, 58)),
                Some((DashboardRegion::SecondaryMetrics, 42)),
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
    if let Some(pulse_bounds) = columns[1] {
        place(sections, DashboardRegion::Pulse, pulse_bounds, context);
    }
    if let Some(right_bounds) = columns[2] {
        let right_slots = region_slots(
            [
                Some((DashboardRegion::Charts, 45)),
                Some((DashboardRegion::Processes, 35)),
                Some((DashboardRegion::Alerts, 20)),
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

fn large_widescreen_layout(
    sections: &mut [Option<DashboardSection>; MAX_DASHBOARD_REGIONS],
    body: DashboardBounds,
    preset: DashboardPreset,
    context: DashboardContext,
) -> Result<(), DashboardError> {
    let hero = group_slot(
        0,
        82,
        [
            Some(DashboardRegion::Pulse),
            Some(DashboardRegion::PrimaryMetrics),
            Some(DashboardRegion::SecondaryMetrics),
            Some(DashboardRegion::Charts),
            Some(DashboardRegion::Processes),
            None,
            None,
            None,
        ],
        Axis::Horizontal,
        preset,
        context,
    );
    let slots = [
        hero,
        region_slot(1, DashboardRegion::Alerts, 18, preset, context),
        None,
        None,
        None,
        None,
        None,
        None,
    ];
    let rows = allocate_axis(body, Axis::Vertical, slots)?;
    if let Some(hero_bounds) = rows[0] {
        let left = group_slot(
            0,
            24,
            [
                Some(DashboardRegion::PrimaryMetrics),
                Some(DashboardRegion::SecondaryMetrics),
                None,
            ],
            Axis::Vertical,
            preset,
            context,
        );
        let right = group_slot(
            2,
            20,
            [
                Some(DashboardRegion::Charts),
                Some(DashboardRegion::Processes),
                None,
            ],
            Axis::Vertical,
            preset,
            context,
        );
        let columns = allocate_axis(
            hero_bounds,
            Axis::Horizontal,
            [
                left,
                region_slot(1, DashboardRegion::Pulse, 56, preset, context),
                right,
                None,
                None,
                None,
                None,
                None,
            ],
        )?;
        if let Some(left_bounds) = columns[0] {
            let left_slots = region_slots(
                [
                    Some((DashboardRegion::PrimaryMetrics, 52)),
                    Some((DashboardRegion::SecondaryMetrics, 48)),
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
        if let Some(pulse_bounds) = columns[1] {
            place(sections, DashboardRegion::Pulse, pulse_bounds, context);
        }
        if let Some(right_bounds) = columns[2] {
            let right_slots = region_slots(
                [
                    Some((DashboardRegion::Charts, 58)),
                    Some((DashboardRegion::Processes, 42)),
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
                allocate_axis(right_bounds, Axis::Vertical, right_slots)?,
                context,
            );
        }
    }
    if let Some(alert_bounds) = rows[1] {
        place(sections, DashboardRegion::Alerts, alert_bounds, context);
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

fn row_slots(
    body: DashboardBounds,
    top_weight: usize,
    bottom_weight: usize,
    top_slots: [Option<LayoutSlot>; MAX_DASHBOARD_REGIONS],
    bottom_slots: [Option<LayoutSlot>; MAX_DASHBOARD_REGIONS],
) -> Result<[[Option<DashboardBounds>; MAX_DASHBOARD_REGIONS]; 2], DashboardError> {
    let top_group = slots_group_slot(0, top_weight, top_slots, Axis::Horizontal);
    let bottom_group = slots_group_slot(1, bottom_weight, bottom_slots, Axis::Horizontal);
    let rows = allocate_axis(
        body,
        Axis::Vertical,
        [top_group, bottom_group, None, None, None, None, None, None],
    )?;

    let top = rows[0]
        .map(|bounds| allocate_axis(bounds, Axis::Horizontal, top_slots))
        .transpose()?
        .unwrap_or([None; MAX_DASHBOARD_REGIONS]);
    let bottom = rows[1]
        .map(|bounds| allocate_axis(bounds, Axis::Horizontal, bottom_slots))
        .transpose()?
        .unwrap_or([None; MAX_DASHBOARD_REGIONS]);
    Ok([top, bottom])
}

fn slots_group_slot(
    key: usize,
    weight: usize,
    slots: [Option<LayoutSlot>; MAX_DASHBOARD_REGIONS],
    axis: Axis,
) -> Option<LayoutSlot> {
    let mut minimum = DashboardMinimum::new(0, 0);
    let mut priority = DashboardPriority::Footer;
    let mut has_slots = false;
    for slot in slots.into_iter().flatten() {
        has_slots = true;
        minimum = DashboardMinimum::new(
            match axis {
                Axis::Horizontal => minimum.width() + slot.minimum.width(),
                Axis::Vertical => minimum.width().max(slot.minimum.width()),
            },
            match axis {
                Axis::Horizontal => minimum.height().max(slot.minimum.height()),
                Axis::Vertical => minimum.height() + slot.minimum.height(),
            },
        );
        priority = priority.min(slot.priority);
    }
    has_slots.then_some(LayoutSlot {
        key,
        weight,
        minimum,
        priority,
    })
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
