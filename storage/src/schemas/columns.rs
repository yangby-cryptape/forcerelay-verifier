//! Constants which define low-level database column families.

/// Column families alias type
pub type Column = &'static str;

/// Total column number
pub const COUNT: usize = 2;

/// Column to store MMR for beacon headers
pub const COLUMN_BEACON_HEADER_MMR: Column = "beacon-header-mmr";

/// Column to store finalized updates
pub const COLUMN_FINALIZED_UPDATES: Column = "finalized-updates";
