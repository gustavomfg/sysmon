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
    use crate::ui::dashboard_v2::{DashboardBounds, DashboardComposition, DashboardRegion};

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
        let compact = DashboardComposition::for_preset(DashboardPreset::Compact, context);
        let large = DashboardComposition::for_preset(DashboardPreset::LargeWidescreen, context);

        assert_eq!(compact.regions().next(), Some(DashboardRegion::Header));
        assert_eq!(compact.regions().last(), Some(DashboardRegion::Footer));
        assert!(compact.contains(DashboardRegion::Pulse));
        assert!(!compact.contains(DashboardRegion::Processes));
        assert!(large.contains(DashboardRegion::Processes));
        assert_eq!(large.len(), DashboardRegion::ALL.len());
    }
}
