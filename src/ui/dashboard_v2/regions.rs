use super::types::{DashboardMinimum, DashboardPreset, MAX_DASHBOARD_REGIONS};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum DashboardPriority {
    Pulse,
    PrimaryMetrics,
    Header,
    Charts,
    Processes,
    Alerts,
    SecondaryMetrics,
    Footer,
}

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

    pub const fn priority(self) -> DashboardPriority {
        match self {
            Self::Pulse => DashboardPriority::Pulse,
            Self::PrimaryMetrics => DashboardPriority::PrimaryMetrics,
            Self::Header => DashboardPriority::Header,
            Self::Charts => DashboardPriority::Charts,
            Self::Processes => DashboardPriority::Processes,
            Self::Alerts => DashboardPriority::Alerts,
            Self::SecondaryMetrics => DashboardPriority::SecondaryMetrics,
            Self::Footer => DashboardPriority::Footer,
        }
    }

    pub(crate) fn minimum(self, preset: DashboardPreset) -> DashboardMinimum {
        match self {
            Self::Header | Self::Footer => DashboardMinimum::new(1, 1),
            Self::Pulse => match preset {
                DashboardPreset::Compact => DashboardMinimum::new(16, 4),
                DashboardPreset::Standard => DashboardMinimum::new(30, 7),
                DashboardPreset::Widescreen => DashboardMinimum::new(44, 10),
                DashboardPreset::LargeWidescreen => DashboardMinimum::new(56, 12),
            },
            Self::PrimaryMetrics => DashboardMinimum::new(18, 3),
            Self::SecondaryMetrics => DashboardMinimum::new(14, 2),
            Self::Charts => DashboardMinimum::new(20, 4),
            Self::Processes => DashboardMinimum::new(22, 5),
            Self::Alerts => DashboardMinimum::new(14, 2),
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
    pub(crate) fn from_active(active: [bool; MAX_DASHBOARD_REGIONS]) -> Self {
        let mut composition = Self {
            regions: [None; MAX_DASHBOARD_REGIONS],
            length: 0,
        };

        for region in [
            DashboardRegion::Pulse,
            DashboardRegion::PrimaryMetrics,
            DashboardRegion::Header,
            DashboardRegion::Charts,
            DashboardRegion::Processes,
            DashboardRegion::Alerts,
            DashboardRegion::SecondaryMetrics,
            DashboardRegion::Footer,
        ] {
            if active[region.index()] {
                composition.push(region);
            }
        }
        composition
    }

    fn push(&mut self, region: DashboardRegion) {
        if usize::from(self.length) < MAX_DASHBOARD_REGIONS {
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
