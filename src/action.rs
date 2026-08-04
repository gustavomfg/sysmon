#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    Quit,
    OpenPalette,
    ClosePalette,
    PaletteUp,
    PaletteDown,
    PaletteBackspace,
    PaletteSubmit,
    PaletteCharacter(char),
    FocusNext,
    FocusPrevious,
    ExitFocus,
}
