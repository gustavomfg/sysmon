use super::types::{DashboardContext, DashboardPreset, MAX_DASHBOARD_REGIONS};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DashboardRegion {
    Header,
    Pulse,
    PrimaryMetrics,
    SecondaryMetrics,
    Charts,
    Processes,
    Alerts,
    Footer,
}

impl DashboardRegion {
    pub const ALL: [Self; MAX_DASHBOARD_REGIONS] = [
        Self::Header,
        Self::Pulse,
        Self::PrimaryMetrics,
        Self::SecondaryMetrics,
        Self::Charts,
        Self::Processes,
        Self::Alerts,
        Self::Footer,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::Header => "Header",
            Self::Pulse => "Pulse",
            Self::PrimaryMetrics => "Primary Metrics",
            Self::SecondaryMetrics => "Secondary Metrics",
            Self::Charts => "Charts",
            Self::Processes => "Processes",
            Self::Alerts => "Alerts",
            Self::Footer => "Footer",
        }
    }

    pub(crate) fn index(self) -> usize {
        match self {
            Self::Header => 0,
            Self::Pulse => 1,
            Self::PrimaryMetrics => 2,
            Self::SecondaryMetrics => 3,
            Self::Charts => 4,
            Self::Processes => 5,
            Self::Alerts => 6,
            Self::Footer => 7,
        }
    }

    pub(crate) fn mask(self) -> u16 {
        1 << self.index()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DashboardComposition {
    regions: [Option<DashboardRegion>; MAX_DASHBOARD_REGIONS],
    length: u8,
}

impl DashboardComposition {
    pub(crate) fn for_preset(preset: DashboardPreset, context: DashboardContext) -> Self {
        let mut composition = Self {
            regions: [None; MAX_DASHBOARD_REGIONS],
            length: 0,
        };
        composition.push_if(DashboardRegion::Header, context);
        composition.push_if(DashboardRegion::Pulse, context);
        composition.push_if(DashboardRegion::PrimaryMetrics, context);
        composition.push_if(DashboardRegion::SecondaryMetrics, context);

        match preset {
            DashboardPreset::Compact => {}
            DashboardPreset::Standard => {
                composition.push_if(DashboardRegion::Charts, context);
                composition.push_if(DashboardRegion::Alerts, context);
            }
            DashboardPreset::Widescreen | DashboardPreset::LargeWidescreen => {
                composition.push_if(DashboardRegion::Charts, context);
                composition.push_if(DashboardRegion::Processes, context);
                composition.push_if(DashboardRegion::Alerts, context);
            }
        }

        composition.push_if(DashboardRegion::Footer, context);
        composition
    }

    fn push_if(&mut self, region: DashboardRegion, context: DashboardContext) {
        if context.includes(region) && usize::from(self.length) < MAX_DASHBOARD_REGIONS {
            self.regions[usize::from(self.length)] = Some(region);
            self.length += 1;
        }
    }

    pub fn len(self) -> usize {
        usize::from(self.length)
    }

    pub fn is_empty(self) -> bool {
        self.length == 0
    }

    pub fn contains(self, region: DashboardRegion) -> bool {
        self.regions().any(|candidate| candidate == region)
    }

    pub fn regions(self) -> impl Iterator<Item = DashboardRegion> {
        self.regions
            .into_iter()
            .take(usize::from(self.length))
            .flatten()
    }
}
