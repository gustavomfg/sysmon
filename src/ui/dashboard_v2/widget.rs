use std::time::Duration;

use super::{
    DashboardBounds, DashboardLayout, DashboardMinimum, DashboardPreset, DashboardPriority,
    DashboardRegion,
};

pub const MAX_DASHBOARD_WIDGETS: usize = 10;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum DashboardWidgetId {
    Pulse,
    SystemSummary,
    Cpu,
    Memory,
    Gpu,
    Network,
    Storage,
    History,
    Processes,
    Alerts,
}

impl DashboardWidgetId {
    pub const ALL: [Self; MAX_DASHBOARD_WIDGETS] = [
        Self::Pulse,
        Self::SystemSummary,
        Self::Cpu,
        Self::Memory,
        Self::Gpu,
        Self::Network,
        Self::Storage,
        Self::History,
        Self::Processes,
        Self::Alerts,
    ];

    const fn index(self) -> usize {
        match self {
            Self::Pulse => 0,
            Self::SystemSummary => 1,
            Self::Cpu => 2,
            Self::Memory => 3,
            Self::Gpu => 4,
            Self::Network => 5,
            Self::Storage => 6,
            Self::History => 7,
            Self::Processes => 8,
            Self::Alerts => 9,
        }
    }

    const fn bit(self) -> u16 {
        1u16 << self.index()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WidgetPresetSupport(u8);

impl WidgetPresetSupport {
    pub const fn all() -> Self {
        Self(0b1111)
    }

    pub const fn without(self, preset: DashboardPreset) -> Self {
        Self(self.0 & !preset_bit(preset))
    }

    pub const fn includes(self, preset: DashboardPreset) -> bool {
        self.0 & preset_bit(preset) != 0
    }

    pub const fn is_empty(self) -> bool {
        self.0 == 0
    }
}

const fn preset_bit(preset: DashboardPreset) -> u8 {
    match preset {
        DashboardPreset::Compact => 1 << 0,
        DashboardPreset::Standard => 1 << 1,
        DashboardPreset::Widescreen => 1 << 2,
        DashboardPreset::LargeWidescreen => 1 << 3,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WidgetRequirements {
    minimum: DashboardMinimum,
    presets: WidgetPresetSupport,
}

impl WidgetRequirements {
    pub const fn new(minimum: DashboardMinimum, presets: WidgetPresetSupport) -> Self {
        Self { minimum, presets }
    }

    pub const fn minimum(self) -> DashboardMinimum {
        self.minimum
    }

    pub const fn presets(self) -> WidgetPresetSupport {
        self.presets
    }

    pub const fn is_valid(self) -> bool {
        self.minimum.width() > 0 && self.minimum.height() > 0 && !self.presets.is_empty()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WidgetDescriptor {
    id: DashboardWidgetId,
    section: DashboardRegion,
    priority: DashboardPriority,
    order: u8,
    focusable: bool,
    accepts_commands: bool,
    requirements: WidgetRequirements,
}

impl WidgetDescriptor {
    pub const fn new(
        id: DashboardWidgetId,
        section: DashboardRegion,
        order: u8,
        focusable: bool,
        accepts_commands: bool,
        requirements: WidgetRequirements,
    ) -> Self {
        Self {
            id,
            section,
            priority: section.priority(),
            order,
            focusable,
            accepts_commands,
            requirements,
        }
    }

    pub const fn id(self) -> DashboardWidgetId {
        self.id
    }

    pub const fn section(self) -> DashboardRegion {
        self.section
    }

    pub const fn priority(self) -> DashboardPriority {
        self.priority
    }

    pub const fn order(self) -> u8 {
        self.order
    }

    pub const fn is_focusable(self) -> bool {
        self.focusable
    }

    pub const fn accepts_commands(self) -> bool {
        self.accepts_commands
    }

    pub const fn requirements(self) -> WidgetRequirements {
        self.requirements
    }

    pub fn is_valid(self) -> bool {
        self.priority == self.section.priority() && self.requirements.is_valid()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WidgetAvailability(u16);

impl WidgetAvailability {
    pub const fn all() -> Self {
        Self((1u16 << MAX_DASHBOARD_WIDGETS) - 1)
    }

    pub const fn empty() -> Self {
        Self(0)
    }

    pub const fn includes(self, id: DashboardWidgetId) -> bool {
        self.0 & id.bit() != 0
    }

    pub const fn with(self, id: DashboardWidgetId) -> Self {
        Self(self.0 | id.bit())
    }

    pub const fn without(self, id: DashboardWidgetId) -> Self {
        Self(self.0 & !id.bit())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DashboardState {
    Calm,
    Active,
    Busy,
    Critical,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WidgetState {
    Idle,
    Normal,
    Active,
    Warning,
    Critical,
    Unavailable,
    Loading,
}

impl DashboardState {
    const fn widget_state(self) -> WidgetState {
        match self {
            Self::Calm => WidgetState::Normal,
            Self::Active => WidgetState::Active,
            Self::Busy => WidgetState::Warning,
            Self::Critical => WidgetState::Critical,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WidgetContext {
    preset: DashboardPreset,
    bounds: Option<DashboardBounds>,
    dashboard_state: DashboardState,
    elapsed: Duration,
    reduced_motion: bool,
    focused_widget: Option<DashboardWidgetId>,
    visible: bool,
    availability: WidgetAvailability,
}

impl WidgetContext {
    pub const fn new(
        preset: DashboardPreset,
        dashboard_state: DashboardState,
        elapsed: Duration,
    ) -> Self {
        Self {
            preset,
            bounds: None,
            dashboard_state,
            elapsed,
            reduced_motion: false,
            focused_widget: None,
            visible: true,
            availability: WidgetAvailability::all(),
        }
    }

    pub const fn preset(self) -> DashboardPreset {
        self.preset
    }

    pub const fn bounds(self) -> Option<DashboardBounds> {
        self.bounds
    }

    pub const fn dashboard_state(self) -> DashboardState {
        self.dashboard_state
    }

    pub const fn elapsed(self) -> Duration {
        self.elapsed
    }

    pub const fn reduced_motion(self) -> bool {
        self.reduced_motion
    }

    pub const fn focused_widget(self) -> Option<DashboardWidgetId> {
        self.focused_widget
    }

    pub const fn visible(self) -> bool {
        self.visible
    }

    pub const fn availability(self) -> WidgetAvailability {
        self.availability
    }

    pub const fn with_bounds(self, bounds: DashboardBounds) -> Self {
        Self {
            bounds: Some(bounds),
            ..self
        }
    }

    pub const fn with_reduced_motion(self, reduced_motion: bool) -> Self {
        Self {
            reduced_motion,
            ..self
        }
    }

    pub const fn with_focus(self, focused_widget: Option<DashboardWidgetId>) -> Self {
        Self {
            focused_widget,
            ..self
        }
    }

    pub const fn with_visibility(self, visible: bool) -> Self {
        Self { visible, ..self }
    }

    pub const fn with_availability(self, availability: WidgetAvailability) -> Self {
        Self {
            availability,
            ..self
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WidgetFrame {
    id: DashboardWidgetId,
    state: WidgetState,
}

impl WidgetFrame {
    pub const fn new(id: DashboardWidgetId, state: WidgetState) -> Self {
        Self { id, state }
    }

    pub const fn id(self) -> DashboardWidgetId {
        self.id
    }

    pub const fn state(self) -> WidgetState {
        self.state
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WidgetModel {
    descriptor: WidgetDescriptor,
}

impl WidgetModel {
    pub const fn new(descriptor: WidgetDescriptor) -> Self {
        Self { descriptor }
    }

    pub const fn descriptor(self) -> WidgetDescriptor {
        self.descriptor
    }

    pub fn is_available(self, context: WidgetContext) -> bool {
        let Some(bounds) = context.bounds() else {
            return false;
        };
        let minimum = self.descriptor.requirements().minimum();
        context.visible()
            && context.availability().includes(self.descriptor.id())
            && self
                .descriptor
                .requirements()
                .presets()
                .includes(context.preset())
            && !bounds.is_empty()
            && bounds.width() >= minimum.width()
            && bounds.height() >= minimum.height()
    }

    pub fn evaluate(self, context: WidgetContext) -> Option<WidgetFrame> {
        self.is_available(context).then_some(WidgetFrame::new(
            self.descriptor.id(),
            context.dashboard_state().widget_state(),
        ))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WidgetPlacement {
    id: DashboardWidgetId,
    section: DashboardRegion,
    bounds: DashboardBounds,
    priority: DashboardPriority,
    order: u8,
    focusable: bool,
    accepts_commands: bool,
    focused: bool,
}

impl WidgetPlacement {
    pub const fn id(self) -> DashboardWidgetId {
        self.id
    }

    pub const fn section(self) -> DashboardRegion {
        self.section
    }

    pub const fn bounds(self) -> DashboardBounds {
        self.bounds
    }

    pub const fn priority(self) -> DashboardPriority {
        self.priority
    }

    pub const fn is_focused(self) -> bool {
        self.focused
    }

    pub const fn is_focusable(self) -> bool {
        self.focusable
    }

    pub const fn accepts_commands(self) -> bool {
        self.accepts_commands
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WidgetSelection {
    placements: [Option<WidgetPlacement>; MAX_DASHBOARD_WIDGETS],
    length: u8,
}

impl WidgetSelection {
    const fn empty() -> Self {
        Self {
            placements: [None; MAX_DASHBOARD_WIDGETS],
            length: 0,
        }
    }

    pub fn len(self) -> usize {
        usize::from(self.length)
    }

    pub fn is_empty(self) -> bool {
        self.length == 0
    }

    pub fn contains(self, id: DashboardWidgetId) -> bool {
        self.widgets().any(|widget| widget.id() == id)
    }

    pub fn widgets(self) -> impl Iterator<Item = WidgetPlacement> {
        self.placements
            .into_iter()
            .take(usize::from(self.length))
            .flatten()
    }

    pub fn for_section(self, section: DashboardRegion) -> impl Iterator<Item = WidgetPlacement> {
        self.widgets()
            .filter(move |widget| widget.section() == section)
    }

    pub fn focus_order(self) -> impl Iterator<Item = DashboardWidgetId> {
        self.widgets()
            .filter(|widget| widget.is_focusable())
            .map(|widget| widget.id())
    }

    fn push_sorted(&mut self, placement: WidgetPlacement) {
        if usize::from(self.length) >= MAX_DASHBOARD_WIDGETS {
            return;
        }

        let length = usize::from(self.length);
        let mut insert_at = length;
        for (index, current) in self.placements.iter().enumerate().take(length) {
            let Some(current) = current else { continue };
            if (placement.priority, placement.order) < (current.priority, current.order) {
                insert_at = index;
                break;
            }
        }

        let mut index = length;
        while index > insert_at {
            self.placements[index] = self.placements[index - 1];
            index -= 1;
        }
        self.placements[insert_at] = Some(placement);
        self.length += 1;
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct WidgetRegistry;

impl WidgetRegistry {
    pub const fn new() -> Self {
        Self
    }

    pub const fn descriptors(self) -> &'static [WidgetDescriptor; MAX_DASHBOARD_WIDGETS] {
        &WIDGET_DESCRIPTORS
    }

    pub fn descriptor(self, id: DashboardWidgetId) -> Option<WidgetDescriptor> {
        self.descriptors()
            .iter()
            .copied()
            .find(|descriptor| descriptor.id() == id)
    }

    pub fn model(self, id: DashboardWidgetId) -> Option<WidgetModel> {
        self.descriptor(id).map(WidgetModel::new)
    }

    pub fn descriptors_for_section(
        self,
        section: DashboardRegion,
    ) -> impl Iterator<Item = WidgetDescriptor> {
        self.descriptors()
            .iter()
            .copied()
            .filter(move |descriptor| descriptor.section() == section)
    }

    pub fn select(self, layout: DashboardLayout, context: WidgetContext) -> WidgetSelection {
        let mut selection = WidgetSelection::empty();
        if context.preset() != layout.preset() {
            return selection;
        }

        for descriptor in self.descriptors() {
            let Some(section) = layout.section(descriptor.section()) else {
                continue;
            };
            let widget_context = context.with_bounds(section.bounds());
            let model = WidgetModel::new(*descriptor);
            if !model.is_available(widget_context) {
                continue;
            }
            selection.push_sorted(WidgetPlacement {
                id: descriptor.id(),
                section: descriptor.section(),
                bounds: section.bounds(),
                priority: descriptor.priority(),
                order: descriptor.order(),
                focusable: descriptor.is_focusable(),
                accepts_commands: descriptor.accepts_commands(),
                focused: descriptor.is_focusable()
                    && context.focused_widget() == Some(descriptor.id()),
            });
        }
        selection
    }
}

const fn requirements(width: usize, height: usize) -> WidgetRequirements {
    WidgetRequirements::new(
        DashboardMinimum::new(width, height),
        WidgetPresetSupport::all(),
    )
}

const WIDGET_DESCRIPTORS: [WidgetDescriptor; MAX_DASHBOARD_WIDGETS] = [
    WidgetDescriptor::new(
        DashboardWidgetId::Pulse,
        DashboardRegion::Pulse,
        0,
        true,
        true,
        requirements(16, 4),
    ),
    WidgetDescriptor::new(
        DashboardWidgetId::SystemSummary,
        DashboardRegion::PrimaryMetrics,
        0,
        true,
        true,
        requirements(1, 1),
    ),
    WidgetDescriptor::new(
        DashboardWidgetId::Cpu,
        DashboardRegion::PrimaryMetrics,
        1,
        true,
        true,
        requirements(1, 1),
    ),
    WidgetDescriptor::new(
        DashboardWidgetId::Memory,
        DashboardRegion::PrimaryMetrics,
        2,
        true,
        true,
        requirements(1, 1),
    ),
    WidgetDescriptor::new(
        DashboardWidgetId::Gpu,
        DashboardRegion::PrimaryMetrics,
        3,
        false,
        false,
        requirements(1, 1),
    ),
    WidgetDescriptor::new(
        DashboardWidgetId::Network,
        DashboardRegion::PrimaryMetrics,
        4,
        false,
        false,
        requirements(1, 1),
    ),
    WidgetDescriptor::new(
        DashboardWidgetId::Storage,
        DashboardRegion::SecondaryMetrics,
        0,
        false,
        false,
        requirements(1, 1),
    ),
    WidgetDescriptor::new(
        DashboardWidgetId::History,
        DashboardRegion::Charts,
        0,
        true,
        true,
        requirements(1, 1),
    ),
    WidgetDescriptor::new(
        DashboardWidgetId::Processes,
        DashboardRegion::Processes,
        0,
        true,
        true,
        WidgetRequirements::new(
            DashboardMinimum::new(1, 1),
            WidgetPresetSupport::all().without(DashboardPreset::Compact),
        ),
    ),
    WidgetDescriptor::new(
        DashboardWidgetId::Alerts,
        DashboardRegion::Alerts,
        0,
        true,
        true,
        requirements(1, 1),
    ),
];

#[cfg(test)]
mod tests {
    use super::*;

    fn dimensions(width: usize, height: usize) -> super::super::DashboardDimensions {
        super::super::DashboardDimensions::new(width, height).unwrap()
    }

    fn context(preset: DashboardPreset) -> WidgetContext {
        WidgetContext::new(preset, DashboardState::Calm, Duration::from_secs(1))
    }

    #[test]
    fn ids_are_stable_and_unique() {
        for (index, id) in DashboardWidgetId::ALL.into_iter().enumerate() {
            assert_eq!(id.index(), index);
            assert!(!DashboardWidgetId::ALL[..index].contains(&id));
        }
    }

    #[test]
    fn registry_descriptors_are_valid_and_match_their_sections() {
        let registry = WidgetRegistry::new();
        assert_eq!(registry.descriptors().len(), MAX_DASHBOARD_WIDGETS);
        for descriptor in registry.descriptors() {
            assert!(descriptor.is_valid());
            assert_eq!(descriptor.priority(), descriptor.section().priority());
        }
    }

    #[test]
    fn registry_finds_widgets_and_lists_them_by_section() {
        let registry = WidgetRegistry::new();
        let pulse = registry.descriptor(DashboardWidgetId::Pulse).unwrap();
        assert_eq!(pulse.section(), DashboardRegion::Pulse);
        assert_eq!(
            registry
                .descriptors_for_section(DashboardRegion::PrimaryMetrics)
                .count(),
            5
        );
        assert!(registry.model(DashboardWidgetId::Cpu).is_some());
    }

    #[test]
    fn selection_is_deterministic_and_follows_priority_then_order() {
        let layout = super::super::DashboardLayoutEngine::new()
            .compose(
                dimensions(240, 60),
                DashboardPreset::LargeWidescreen,
                super::super::DashboardContext::default(),
            )
            .unwrap();
        let registry = WidgetRegistry::new();
        let context = context(DashboardPreset::LargeWidescreen);
        let first = registry.select(layout, context);
        let second = registry.select(layout, context);
        assert_eq!(first, second);

        let mut previous: Option<(DashboardPriority, DashboardWidgetId)> = None;
        for widget in first.widgets() {
            if let Some(previous) = previous {
                assert!(previous <= (widget.priority(), widget.id()));
            }
            previous = Some((widget.priority(), widget.id()));
        }
    }

    #[test]
    fn availability_and_visibility_filter_without_reserving_layout_space() {
        let layout = super::super::DashboardLayoutEngine::new()
            .compose(
                dimensions(240, 60),
                DashboardPreset::LargeWidescreen,
                super::super::DashboardContext::default(),
            )
            .unwrap();
        let availability = WidgetAvailability::all()
            .without(DashboardWidgetId::Processes)
            .without(DashboardWidgetId::History);
        let context = context(DashboardPreset::LargeWidescreen).with_availability(availability);
        let selection = WidgetRegistry::new().select(layout, context);

        assert!(!selection.contains(DashboardWidgetId::Processes));
        assert!(!selection.contains(DashboardWidgetId::History));
        assert!(selection.contains(DashboardWidgetId::Pulse));
        assert!(
            selection
                .for_section(DashboardRegion::Processes)
                .next()
                .is_none()
        );

        let hidden = WidgetRegistry::new().select(layout, context.with_visibility(false));
        assert!(hidden.is_empty());
    }

    #[test]
    fn preset_support_and_minimum_requirements_are_checked() {
        let registry = WidgetRegistry::new();
        let process = registry.model(DashboardWidgetId::Processes).unwrap();
        let small = DashboardBounds::new(0, 0, 1, 1).unwrap();
        assert!(!process.is_available(context(DashboardPreset::Compact).with_bounds(small)));
        assert!(process.is_available(context(DashboardPreset::Standard).with_bounds(small)));

        let pulse = registry.model(DashboardWidgetId::Pulse).unwrap();
        assert!(!pulse.is_available(context(DashboardPreset::Standard).with_bounds(small)));
        let pulse_bounds = DashboardBounds::new(0, 0, 16, 4).unwrap();
        assert!(pulse.is_available(context(DashboardPreset::Standard).with_bounds(pulse_bounds)));
    }

    #[test]
    fn focus_order_is_stable_and_focus_is_semantic() {
        let layout = super::super::DashboardLayoutEngine::new()
            .compose(
                dimensions(240, 60),
                DashboardPreset::LargeWidescreen,
                super::super::DashboardContext::default(),
            )
            .unwrap();
        let context =
            context(DashboardPreset::LargeWidescreen).with_focus(Some(DashboardWidgetId::Pulse));
        let selection = WidgetRegistry::new().select(layout, context);
        let focused: Vec<_> = selection
            .widgets()
            .filter(|widget| widget.is_focused())
            .map(|widget| widget.id())
            .collect();
        assert_eq!(focused, [DashboardWidgetId::Pulse]);
        let focus_order: Vec<_> = selection.focus_order().collect();
        assert_eq!(focus_order.first(), Some(&DashboardWidgetId::Pulse));
        assert!(focus_order.contains(&DashboardWidgetId::Cpu));
        assert!(focus_order.contains(&DashboardWidgetId::History));
    }

    #[test]
    fn semantic_frames_do_not_contain_terminal_appearance() {
        let model = WidgetRegistry::new()
            .model(DashboardWidgetId::Pulse)
            .unwrap();
        let bounds = DashboardBounds::new(0, 0, 16, 4).unwrap();
        let calm = model
            .evaluate(context(DashboardPreset::Standard).with_bounds(bounds))
            .unwrap();
        let critical = model
            .evaluate(
                context(DashboardPreset::Standard)
                    .with_bounds(bounds)
                    .with_reduced_motion(true),
            )
            .unwrap();
        assert_eq!(calm.id(), DashboardWidgetId::Pulse);
        assert_eq!(calm.state(), WidgetState::Normal);
        assert_eq!(critical.state(), WidgetState::Normal);
        assert!(
            context(DashboardPreset::Standard)
                .with_reduced_motion(true)
                .reduced_motion()
        );
    }

    #[test]
    fn invalid_region_or_unavailable_data_produces_no_frame() {
        let model = WidgetRegistry::new()
            .model(DashboardWidgetId::Pulse)
            .unwrap();
        let context = context(DashboardPreset::Standard);
        assert!(model.evaluate(context).is_none());
        assert!(
            model
                .evaluate(
                    context
                        .with_bounds(DashboardBounds::new(0, 0, 16, 4).unwrap())
                        .with_availability(WidgetAvailability::empty())
                )
                .is_none()
        );
    }

    #[test]
    fn all_representative_layouts_select_without_panicking() {
        let registry = WidgetRegistry::new();
        for (preset, width, height) in [
            (DashboardPreset::Compact, 80, 24),
            (DashboardPreset::Standard, 140, 40),
            (DashboardPreset::Widescreen, 160, 40),
            (DashboardPreset::LargeWidescreen, 240, 60),
        ] {
            let layout = super::super::DashboardLayoutEngine::new()
                .compose(
                    dimensions(width, height),
                    preset,
                    super::super::DashboardContext::default(),
                )
                .unwrap();
            let selected = registry.select(layout, context(preset));
            assert!(selected.len() <= MAX_DASHBOARD_WIDGETS);
            assert!(selected.widgets().all(|widget| {
                layout
                    .section(widget.section())
                    .is_some_and(|section| section.bounds() == widget.bounds())
            }));
        }
    }

    #[test]
    fn context_preserves_reduced_motion_and_time_without_global_state() {
        let context = WidgetContext::new(
            DashboardPreset::Widescreen,
            DashboardState::Busy,
            Duration::from_millis(250),
        )
        .with_reduced_motion(true);
        assert_eq!(context.elapsed(), Duration::from_millis(250));
        assert!(context.reduced_motion());
        assert_eq!(context.dashboard_state(), DashboardState::Busy);
    }
}
