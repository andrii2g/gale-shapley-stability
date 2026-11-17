mod profile;
mod solver;
mod svg;
mod verify;

pub use profile::{Profile, Side};
pub use solver::{Event, Outcome, Run, solve};
pub use svg::{write_animated_svg, write_svg_frames};
pub use verify::{MAX_EXHAUSTIVE_SIZE, Verification, blocking_pairs, verify_exhaustively};
