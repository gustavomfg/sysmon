use super::{
    DashboardContext, DashboardDimensions, DashboardError, DashboardLayout, DashboardLayoutEngine,
    DashboardPreset,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DashboardFrame {
    dimensions: DashboardDimensions,
    preset: DashboardPreset,
    context: DashboardContext,
    layout: DashboardLayout,
}

impl DashboardFrame {
    pub fn dimensions(self) -> DashboardDimensions {
        self.dimensions
    }

    pub fn preset(self) -> DashboardPreset {
        self.preset
    }

    pub fn context(self) -> DashboardContext {
        self.context
    }

    pub fn layout(self) -> DashboardLayout {
        self.layout
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct DashboardEngine {
    layout_engine: DashboardLayoutEngine,
}

impl DashboardEngine {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn compose(
        self,
        dimensions: DashboardDimensions,
        context: DashboardContext,
    ) -> Result<DashboardFrame, DashboardError> {
        self.compose_with_preset(
            dimensions,
            DashboardPreset::from_dimensions(dimensions),
            context,
        )
    }

    pub fn compose_with_preset(
        self,
        dimensions: DashboardDimensions,
        preset: DashboardPreset,
        context: DashboardContext,
    ) -> Result<DashboardFrame, DashboardError> {
        let layout = self.layout_engine.compose(dimensions, preset, context)?;
        Ok(DashboardFrame {
            dimensions,
            preset,
            context,
            layout,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::dashboard_v2::{DashboardBounds, DashboardPriority, DashboardRegion};

    fn dimensions(width: usize, height: usize) -> DashboardDimensions {
        DashboardDimensions::new(width, height).unwrap()
    }

    #[test]
    fn creates_all_presets_with_valid_non_overlapping_regions() {
        let engine = DashboardEngine::new();
        for preset in DashboardPreset::ALL {
            let frame = engine
                .compose_with_preset(dimensions(240, 60), preset, DashboardContext::default())
                .unwrap();
            assert_eq!(frame.preset(), preset);
            assert_eq!(frame.dimensions(), dimensions(240, 60));
            assert_eq!(frame.layout().preset(), preset);
            assert!(frame.layout().validate().is_ok());

            for section in frame.layout().sections() {
                assert!(section.bounds().is_within(frame.dimensions()));
            }
        }
    }

    #[test]
    fn automatic_preset_selection_is_responsive() {
        let engine = DashboardEngine::new();
        let cases = [
            ((80, 24), DashboardPreset::Compact),
            ((120, 32), DashboardPreset::Standard),
            ((160, 40), DashboardPreset::Widescreen),
            ((200, 50), DashboardPreset::LargeWidescreen),
        ];

        for ((width, height), expected) in cases {
            let frame = engine
                .compose(dimensions(width, height), DashboardContext::default())
                .unwrap();
            assert_eq!(frame.preset(), expected);
        }
    }

    #[test]
    fn compositions_are_distinct_across_presets() {
        let engine = DashboardEngine::new();
        let dimensions = dimensions(240, 60);
        let layouts = DashboardPreset::ALL.map(|preset| {
            engine
                .compose_with_preset(dimensions, preset, DashboardContext::default())
                .unwrap()
                .layout()
        });

        assert_ne!(layouts[0], layouts[1]);
        assert_ne!(layouts[1], layouts[2]);
        assert_ne!(layouts[2], layouts[3]);
        assert!(layouts[0].composition().len() < layouts[3].composition().len());
    }

    #[test]
    fn same_inputs_produce_the_same_frame() {
        let engine = DashboardEngine::new();
        let dimensions = dimensions(160, 40);
        let first = engine
            .compose(dimensions, DashboardContext::default())
            .unwrap();
        let second = engine
            .compose(dimensions, DashboardContext::default())
            .unwrap();

        assert_eq!(first, second);
    }

    #[test]
    fn context_can_remove_regions_without_affecting_other_bounds() {
        let engine = DashboardEngine::new();
        let context = DashboardContext::default().without(DashboardRegion::Alerts);
        let frame = engine
            .compose_with_preset(dimensions(160, 40), DashboardPreset::Widescreen, context)
            .unwrap();

        assert!(frame.layout().section(DashboardRegion::Alerts).is_none());
        assert!(frame.layout().section(DashboardRegion::Pulse).is_some());
        assert!(frame.layout().validate().is_ok());
    }

    #[test]
    fn minimum_dimensions_are_safe_and_explicitly_empty_where_needed() {
        let engine = DashboardEngine::new();
        let frame = engine
            .compose(dimensions(1, 1), DashboardContext::default())
            .unwrap();

        assert!(frame.layout().validate().is_ok());
        assert!(
            frame
                .layout()
                .sections()
                .all(|section| !section.bounds().is_empty())
        );
    }

    #[test]
    fn dimensions_and_bounds_reject_excessive_or_overflowing_values() {
        assert!(DashboardDimensions::new(usize::MAX, 2).is_err());
        assert!(DashboardDimensions::new(2, usize::MAX).is_err());
        assert!(DashboardBounds::new(usize::MAX, 0, 1, 1).is_err());
        assert!(DashboardBounds::new(0, usize::MAX, 1, 1).is_err());
    }

    #[test]
    fn composition_keeps_region_contracts_and_order() {
        let context = DashboardContext::default();
        let compact = DashboardEngine::new()
            .compose_with_preset(dimensions(80, 24), DashboardPreset::Compact, context)
            .unwrap()
            .layout()
            .composition();
        let large = DashboardEngine::new()
            .compose_with_preset(
                dimensions(240, 60),
                DashboardPreset::LargeWidescreen,
                context,
            )
            .unwrap()
            .layout()
            .composition();

        assert_eq!(compact.regions().next(), Some(DashboardRegion::Pulse));
        assert_eq!(compact.regions().last(), Some(DashboardRegion::Footer));
        assert!(compact.contains(DashboardRegion::Pulse));
        assert!(!compact.contains(DashboardRegion::SecondaryMetrics));
        assert!(!compact.contains(DashboardRegion::Processes));
        assert!(large.contains(DashboardRegion::Processes));
        assert_eq!(large.len(), DashboardRegion::ALL.len());
    }

    #[test]
    fn priorities_are_explicit_and_composition_is_priority_ordered() {
        assert_eq!(DashboardRegion::Pulse.priority(), DashboardPriority::Pulse);
        assert_eq!(
            DashboardRegion::PrimaryMetrics.priority(),
            DashboardPriority::PrimaryMetrics
        );
        assert_eq!(
            DashboardRegion::Header.priority(),
            DashboardPriority::Header
        );
        assert_eq!(
            DashboardRegion::Charts.priority(),
            DashboardPriority::Charts
        );
        assert_eq!(
            DashboardRegion::Processes.priority(),
            DashboardPriority::Processes
        );
        assert_eq!(
            DashboardRegion::Alerts.priority(),
            DashboardPriority::Alerts
        );
        assert_eq!(
            DashboardRegion::SecondaryMetrics.priority(),
            DashboardPriority::SecondaryMetrics
        );
        assert_eq!(
            DashboardRegion::Footer.priority(),
            DashboardPriority::Footer
        );

        let layout = DashboardEngine::new()
            .compose_with_preset(
                dimensions(240, 60),
                DashboardPreset::LargeWidescreen,
                DashboardContext::default(),
            )
            .unwrap()
            .layout();
        let mut previous = None;
        for region in layout.composition().regions() {
            if let Some(previous) = previous {
                assert!(previous <= region.priority());
            }
            previous = Some(region.priority());
        }
    }

    #[test]
    fn pulse_is_dominant_in_wide_presets() {
        let engine = DashboardEngine::new();
        for preset in [
            DashboardPreset::Widescreen,
            DashboardPreset::LargeWidescreen,
        ] {
            let layout = engine
                .compose_with_preset(dimensions(240, 60), preset, DashboardContext::default())
                .unwrap()
                .layout();
            let pulse = layout.section(DashboardRegion::Pulse).unwrap().bounds();
            assert!(
                layout
                    .sections()
                    .filter(|section| section.region() != DashboardRegion::Pulse)
                    .all(|section| pulse.area() >= section.bounds().area())
            );
        }
    }

    #[test]
    fn hidden_sections_leave_no_reserved_space_and_expand_neighbors() {
        let engine = DashboardEngine::new();
        let wide_dimensions = dimensions(160, 40);
        let full = engine
            .compose_with_preset(
                wide_dimensions,
                DashboardPreset::Widescreen,
                DashboardContext::default(),
            )
            .unwrap()
            .layout();
        let reduced_context = DashboardContext::default()
            .without(DashboardRegion::SecondaryMetrics)
            .without(DashboardRegion::Processes);
        let reduced = engine
            .compose_with_preset(
                wide_dimensions,
                DashboardPreset::Widescreen,
                reduced_context,
            )
            .unwrap()
            .layout();

        assert!(reduced.section(DashboardRegion::SecondaryMetrics).is_none());
        assert!(reduced.section(DashboardRegion::Processes).is_none());
        assert!(reduced.validate().is_ok());
        assert!(
            reduced
                .section(DashboardRegion::PrimaryMetrics)
                .unwrap()
                .bounds()
                .height()
                > full
                    .section(DashboardRegion::PrimaryMetrics)
                    .unwrap()
                    .bounds()
                    .height()
        );

        let standard_context = DashboardContext::default()
            .without(DashboardRegion::SecondaryMetrics)
            .without(DashboardRegion::Processes)
            .without(DashboardRegion::Charts);
        let standard = engine
            .compose_with_preset(
                dimensions(140, 40),
                DashboardPreset::Standard,
                standard_context,
            )
            .unwrap()
            .layout();
        let alerts = standard.section(DashboardRegion::Alerts).unwrap().bounds();
        assert_eq!(alerts.x(), 0);
        assert_eq!(alerts.width(), 140);
    }

    #[test]
    fn every_active_region_respects_its_architectural_minimum() {
        let engine = DashboardEngine::new();
        for preset in DashboardPreset::ALL {
            let layout = engine
                .compose_with_preset(dimensions(240, 60), preset, DashboardContext::default())
                .unwrap()
                .layout();
            for region in layout.composition().regions() {
                let bounds = layout.section(region).unwrap().bounds();
                let minimum = region.minimum(preset);
                assert!(bounds.width() >= minimum.width());
                assert!(bounds.height() >= minimum.height());
            }
        }
    }

    #[test]
    fn standard_keeps_metrics_and_charts_but_processes_are_optional() {
        let engine = DashboardEngine::new();
        let context = DashboardContext::default().without(DashboardRegion::Processes);
        let layout = engine
            .compose_with_preset(dimensions(140, 40), DashboardPreset::Standard, context)
            .unwrap()
            .layout();

        assert!(layout.composition().contains(DashboardRegion::Pulse));
        assert!(
            layout
                .composition()
                .contains(DashboardRegion::PrimaryMetrics)
        );
        assert!(
            layout
                .composition()
                .contains(DashboardRegion::SecondaryMetrics)
        );
        assert!(layout.composition().contains(DashboardRegion::Charts));
        assert!(layout.composition().contains(DashboardRegion::Alerts));
        assert!(!layout.composition().contains(DashboardRegion::Processes));
    }

    #[test]
    fn wide_compositions_are_asymmetric_and_change_gradually() {
        let engine = DashboardEngine::new();
        let first = engine
            .compose_with_preset(
                dimensions(160, 40),
                DashboardPreset::Widescreen,
                DashboardContext::default(),
            )
            .unwrap()
            .layout();
        let next = engine
            .compose_with_preset(
                dimensions(161, 40),
                DashboardPreset::Widescreen,
                DashboardContext::default(),
            )
            .unwrap()
            .layout();

        let pulse = first.section(DashboardRegion::Pulse).unwrap().bounds();
        let primary = first
            .section(DashboardRegion::PrimaryMetrics)
            .unwrap()
            .bounds();
        let charts = first.section(DashboardRegion::Charts).unwrap().bounds();
        assert!(pulse.x() > primary.x());
        assert!(charts.x() > pulse.x());
        assert_eq!(first.composition(), next.composition());
        let next_pulse = next.section(DashboardRegion::Pulse).unwrap().bounds();
        assert!(pulse.width().abs_diff(next_pulse.width()) <= 1);
        assert!(first.validate().is_ok());
        assert!(next.validate().is_ok());
    }
}
