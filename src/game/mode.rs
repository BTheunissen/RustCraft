//! Session rules, independent of input and presentation.

/// Creative exploration remains the default; survival enables resource interactions.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum GameMode {
    /// Unrestricted flight and exploration speed, without survival interactions.
    #[default]
    Creative,
    /// Walking, swimming, mining, pickups, and inventory-backed placement.
    Survival,
}
