use super::regions::DashboardRegion;

pub const MAX_DASHBOARD_REGIONS: usize = 8;
pub const MAX_DASHBOARD_DIMENSION: usize = 16_384;
pub const MAX_DASHBOARD_CELLS: usize = 4_194_304;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DashboardError {
    DimensionsTooLarge,
    AreaTooLarge,
    BoundsOverflow,
    InvalidLayout,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DashboardMinimum {
    width: usize,
    height: usize,
}

impl DashboardMinimum {
    pub const fn new(width: usize, height: usize) -> Self {
        Self { width, height }
    }

    pub const fn width(self) -> usize {
        self.width
    }

    pub const fn height(self) -> usize {
        self.height
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DashboardDimensions {
    width: usize,
    height: usize,
}

impl DashboardDimensions {
    pub fn new(width: usize, height: usize) -> Result<Self, DashboardError> {
        if width > MAX_DASHBOARD_DIMENSION || height > MAX_DASHBOARD_DIMENSION {
            return Err(DashboardError::DimensionsTooLarge);
        }
        let area = width
            .checked_mul(height)
            .ok_or(DashboardError::AreaTooLarge)?;
        if area > MAX_DASHBOARD_CELLS {
            return Err(DashboardError::AreaTooLarge);
        }
        Ok(Self { width, height })
    }

    pub fn width(self) -> usize {
        self.width
    }

    pub fn height(self) -> usize {
        self.height
    }

    pub fn area(self) -> usize {
        self.width * self.height
    }

    pub fn is_empty(self) -> bool {
        self.width == 0 || self.height == 0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DashboardBounds {
    x: usize,
    y: usize,
    width: usize,
    height: usize,
}

impl DashboardBounds {
    pub fn new(x: usize, y: usize, width: usize, height: usize) -> Result<Self, DashboardError> {
        if width > MAX_DASHBOARD_DIMENSION || height > MAX_DASHBOARD_DIMENSION {
            return Err(DashboardError::DimensionsTooLarge);
        }
        let right = x.checked_add(width).ok_or(DashboardError::BoundsOverflow)?;
        let bottom = y
            .checked_add(height)
            .ok_or(DashboardError::BoundsOverflow)?;
        if right > MAX_DASHBOARD_DIMENSION || bottom > MAX_DASHBOARD_DIMENSION {
            return Err(DashboardError::DimensionsTooLarge);
        }
        Ok(Self {
            x,
            y,
            width,
            height,
        })
    }

    pub fn x(self) -> usize {
        self.x
    }

    pub fn y(self) -> usize {
        self.y
    }

    pub fn width(self) -> usize {
        self.width
    }

    pub fn height(self) -> usize {
        self.height
    }

    pub fn right(self) -> usize {
        self.x + self.width
    }

    pub fn bottom(self) -> usize {
        self.y + self.height
    }

    pub fn area(self) -> usize {
        self.width * self.height
    }

    pub fn is_empty(self) -> bool {
        self.width == 0 || self.height == 0
    }

    pub fn contains(self, x: usize, y: usize) -> bool {
        x >= self.x && x < self.right() && y >= self.y && y < self.bottom()
    }

    pub fn is_within(self, dimensions: DashboardDimensions) -> bool {
        self.right() <= dimensions.width() && self.bottom() <= dimensions.height()
    }

    pub fn overlaps(self, other: Self) -> bool {
        !self.is_empty()
            && !other.is_empty()
            && self.x < other.right()
            && other.x < self.right()
            && self.y < other.bottom()
            && other.y < self.bottom()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DashboardPreset {
    Compact,
    Standard,
    Widescreen,
    LargeWidescreen,
}

impl DashboardPreset {
    pub const ALL: [Self; 4] = [
        Self::Compact,
        Self::Standard,
        Self::Widescreen,
        Self::LargeWidescreen,
    ];

    pub fn from_dimensions(dimensions: DashboardDimensions) -> Self {
        match (dimensions.width(), dimensions.height()) {
            (180.., 44..) => Self::LargeWidescreen,
            (140.., 32..) => Self::Widescreen,
            (100.., 28..) => Self::Standard,
            _ => Self::Compact,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DashboardContext {
    enabled_regions: u16,
}

impl DashboardContext {
    pub fn all() -> Self {
        let mut enabled_regions = 0;
        for region in DashboardRegion::ALL {
            enabled_regions |= region.mask();
        }
        Self { enabled_regions }
    }

    pub fn includes(self, region: DashboardRegion) -> bool {
        self.enabled_regions & region.mask() != 0
    }

    pub fn without(self, region: DashboardRegion) -> Self {
        Self {
            enabled_regions: self.enabled_regions & !region.mask(),
        }
    }

    pub fn with(self, region: DashboardRegion) -> Self {
        Self {
            enabled_regions: self.enabled_regions | region.mask(),
        }
    }
}

impl Default for DashboardContext {
    fn default() -> Self {
        Self::all()
    }
}
