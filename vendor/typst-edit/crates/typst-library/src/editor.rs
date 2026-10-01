//! Experimental, opaque structural identity and nonprinting math holes.

use crate::foundations::elem;
use crate::layout::{Abs, Point, Size};
use crate::math::Mathy;

/// A caller-owned identity. Neither a source span nor an introspection location.
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub struct EditOrigin {
    /// Stable owning node identity.
    pub node: u128,
    /// Stable slot identity, if this is a slot rather than its parent node.
    pub slot: Option<u128>,
    /// Whether this slot is incomplete instead of containing regular content.
    pub hole: bool,
}

/// Geometry in the containing frame's local point coordinates.
#[derive(Debug, Copy, Clone, Hash)]
pub struct EditBounds {
    /// Structural owner and slot.
    pub origin: EditOrigin,
    /// Whether this extent belongs to a printed decoration instead of content.
    pub decoration: bool,
    /// Top-left point in the frame.
    pub position: Point,
    /// Layout extent, including an empty slot's reserved area.
    pub size: Size,
    /// Baseline measured from the frame top.
    pub baseline: Abs,
}

/// A required, empty mathematical input slot. Not registered in the language.
#[elem(Mathy)]
pub struct EditHoleElem {}

/// A grapheme insertion stop inside a shaped cluster.
#[derive(Debug, Clone, Hash)]
pub struct EditCaret {
    /// UTF-8 byte offset in the caller's text leaf.
    pub byte: usize,
    /// Horizontal distance from the cluster's left edge, in points.
    pub offset: Abs,
    /// True for shaped cluster edges; false for proportional ligature fallback.
    pub exact: bool,
}

/// A shaped cluster's advance geometry, independent of printable glyph ranges.
#[derive(Debug, Clone, Hash)]
pub struct EditCluster {
    /// Stable text leaf identity.
    pub origin: EditOrigin,
    /// UTF-8 byte range in the caller's text leaf (not u16 glyph/source offsets).
    pub range: std::ops::Range<usize>,
    /// Top-left advance position in the containing frame's local points.
    pub position: Point,
    /// Advance width and line metric height, in points.
    pub size: Size,
    /// Baseline from the containing frame's top, in points.
    pub baseline: Abs,
    /// Whether logical text advances from right to left.
    pub rtl: bool,
    /// Insertion stops on grapheme boundaries.
    pub carets: Vec<EditCaret>,
}
