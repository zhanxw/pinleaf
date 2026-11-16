use std::fmt;

/// 0-based internal PDF page index.
#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PageIndex(pub u32);

impl PageIndex {
    #[inline]
    pub const fn as_usize(self) -> usize {
        self.0 as usize
    }

    #[inline]
    pub const fn one_based(self) -> u32 {
        self.0 + 1
    }
}

impl From<u32> for PageIndex {
    fn from(value: u32) -> Self {
        Self(value)
    }
}

impl fmt::Display for PageIndex {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0 + 1)
    }
}

/// Raw PDF user-space point (origin at bottom-left of MediaBox, Y increases upward, unrotated).
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct PdfPoint {
    pub x: f32,
    pub y: f32,
}

impl PdfPoint {
    #[inline]
    pub const fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }
}

/// Normalized visual page-space point (origin at visual top-left of CropBox, rotation applied, Y increases downward, in PDF points 1/72").
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct NormPoint {
    pub x: f32,
    pub y: f32,
}

impl NormPoint {
    #[inline]
    pub const fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }
}

/// Axis-aligned rectangle in Normalized Page Space.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct NormRect {
    pub min_x: f32,
    pub min_y: f32,
    pub max_x: f32,
    pub max_y: f32,
}

impl NormRect {
    #[inline]
    pub const fn new(min_x: f32, min_y: f32, max_x: f32, max_y: f32) -> Self {
        Self {
            min_x,
            min_y,
            max_x,
            max_y,
        }
    }

    #[inline]
    pub fn from_points(a: NormPoint, b: NormPoint) -> Self {
        Self {
            min_x: a.x.min(b.x),
            min_y: a.y.min(b.y),
            max_x: a.x.max(b.x),
            max_y: a.y.max(b.y),
        }
    }

    #[inline]
    pub fn width(self) -> f32 {
        (self.max_x - self.min_x).max(0.0)
    }

    #[inline]
    pub fn height(self) -> f32 {
        (self.max_y - self.min_y).max(0.0)
    }

    #[inline]
    pub fn center(self) -> NormPoint {
        NormPoint::new((self.min_x + self.max_x) * 0.5, (self.min_y + self.max_y) * 0.5)
    }

    #[inline]
    pub fn contains(self, p: NormPoint) -> bool {
        p.x >= self.min_x && p.x <= self.max_x && p.y >= self.min_y && p.y <= self.max_y
    }

    #[inline]
    pub fn intersects(self, other: NormRect) -> bool {
        self.min_x <= other.max_x
            && self.max_x >= other.min_x
            && self.min_y <= other.max_y
            && self.max_y >= other.min_y
    }

    #[inline]
    pub fn union(self, other: NormRect) -> Self {
        Self {
            min_x: self.min_x.min(other.min_x),
            min_y: self.min_y.min(other.min_y),
            max_x: self.max_x.max(other.max_x),
            max_y: self.max_y.max(other.max_y),
        }
    }

    #[inline]
    pub fn inflate(self, amount: f32) -> Self {
        Self {
            min_x: self.min_x - amount,
            min_y: self.min_y - amount,
            max_x: self.max_x + amount,
            max_y: self.max_y + amount,
        }
    }
}

/// 4-corner quadrilateral in Normalized Page Space, canonically ordered `[TL, TR, BL, BR]`.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct NormQuad {
    pub corners: [NormPoint; 4],
}

impl NormQuad {
    #[inline]
    pub fn from_rect(rect: NormRect) -> Self {
        Self {
            corners: [
                NormPoint::new(rect.min_x, rect.min_y), // TL
                NormPoint::new(rect.max_x, rect.min_y), // TR
                NormPoint::new(rect.min_x, rect.max_y), // BL
                NormPoint::new(rect.max_x, rect.max_y), // BR
            ],
        }
    }

    /// Normalizes any 4-corner ordering (ISO-32000 CCW, Adobe TL-TR-BL-BR, or rotated variants)
    /// into `[TL, TR, BL, BR]` in visual Normalized Page Space.
    pub fn from_unordered_points(mut pts: [NormPoint; 4]) -> Self {
        pts.sort_by(|a, b| a.y.total_cmp(&b.y).then_with(|| a.x.total_cmp(&b.x)));
        let mut top = [pts[0], pts[1]];
        let mut bottom = [pts[2], pts[3]];
        top.sort_by(|a, b| a.x.total_cmp(&b.x));
        bottom.sort_by(|a, b| a.x.total_cmp(&b.x));
        Self {
            corners: [top[0], top[1], bottom[0], bottom[1]],
        }
    }

    pub fn bounding_rect(&self) -> NormRect {
        let mut min_x = f32::INFINITY;
        let mut min_y = f32::INFINITY;
        let mut max_x = f32::NEG_INFINITY;
        let mut max_y = f32::NEG_INFINITY;
        for c in &self.corners {
            min_x = min_x.min(c.x);
            min_y = min_y.min(c.y);
            max_x = max_x.max(c.x);
            max_y = max_y.max(c.y);
        }
        NormRect::new(min_x, min_y, max_x, max_y)
    }

    pub fn contains(&self, p: NormPoint) -> bool {
        // Check bounding rect first; for general quads use barycentric/cross-product sign check
        // on the two triangles (TL, TR, BR) and (TL, BR, BL).
        if !self.bounding_rect().contains(p) {
            return false;
        }
        let [tl, tr, bl, br] = self.corners;
        point_in_triangle(p, tl, tr, br) || point_in_triangle(p, tl, br, bl)
    }
}

fn sign(p1: NormPoint, p2: NormPoint, p3: NormPoint) -> f32 {
    (p1.x - p3.x) * (p2.y - p3.y) - (p2.x - p3.x) * (p1.y - p3.y)
}

fn point_in_triangle(pt: NormPoint, v1: NormPoint, v2: NormPoint, v3: NormPoint) -> bool {
    let d1 = sign(pt, v1, v2);
    let d2 = sign(pt, v2, v3);
    let d3 = sign(pt, v3, v1);
    let has_neg = (d1 < -1e-4) || (d2 < -1e-4) || (d3 < -1e-4);
    let has_pos = (d1 > 1e-4) || (d2 > 1e-4) || (d3 > 1e-4);
    !(has_neg && has_pos)
}

/// Page `/Rotate` clockwise rotation in degrees.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Default)]
pub enum PageRotation {
    #[default]
    Deg0,
    Deg90Cw,
    Deg180,
    Deg270Cw,
}

impl PageRotation {
    pub fn from_degrees(deg: i32) -> Self {
        match deg.rem_euclid(360) {
            90 => Self::Deg90Cw,
            180 => Self::Deg180,
            270 => Self::Deg270Cw,
            _ => Self::Deg0,
        }
    }

    pub const fn as_degrees(self) -> i32 {
        match self {
            Self::Deg0 => 0,
            Self::Deg90Cw => 90,
            Self::Deg180 => 180,
            Self::Deg270Cw => 270,
        }
    }
}

/// Bijective coordinate mapper between raw PDF User Space and Normalized visual Page Space.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct PageTransform {
    /// `[x0, y0, x1, y1]` intersected CropBox in raw PDF user-space points.
    pub crop_box: [f32; 4],
    pub rotation: PageRotation,
}

impl PageTransform {
    pub fn new(crop_box: [f32; 4], rotation: PageRotation) -> Self {
        let [x0, y0, x1, y1] = crop_box;
        Self {
            crop_box: [x0.min(x1), y0.min(y1), x0.max(x1), y0.max(y1)],
            rotation,
        }
    }

    /// Visual page dimensions `(width_pts, height_pts)` after rotation.
    pub fn visual_size(&self) -> (f32, f32) {
        let w = (self.crop_box[2] - self.crop_box[0]).max(1.0);
        let h = (self.crop_box[3] - self.crop_box[1]).max(1.0);
        match self.rotation {
            PageRotation::Deg0 | PageRotation::Deg180 => (w, h),
            PageRotation::Deg90Cw | PageRotation::Deg270Cw => (h, w),
        }
    }

    pub fn pdf_to_norm(&self, p: PdfPoint) -> NormPoint {
        let [x0, y0, x1, y1] = self.crop_box;
        match self.rotation {
            PageRotation::Deg0 => NormPoint::new(p.x - x0, y1 - p.y),
            PageRotation::Deg90Cw => NormPoint::new(p.y - y0, p.x - x0),
            PageRotation::Deg180 => NormPoint::new(x1 - p.x, p.y - y0),
            PageRotation::Deg270Cw => NormPoint::new(y1 - p.y, x1 - p.x),
        }
    }

    pub fn norm_to_pdf(&self, n: NormPoint) -> PdfPoint {
        let [x0, y0, x1, y1] = self.crop_box;
        match self.rotation {
            PageRotation::Deg0 => PdfPoint::new(x0 + n.x, y1 - n.y),
            PageRotation::Deg90Cw => PdfPoint::new(x0 + n.y, y0 + n.x),
            PageRotation::Deg180 => PdfPoint::new(x1 - n.x, y0 + n.y),
            PageRotation::Deg270Cw => PdfPoint::new(x1 - n.y, y1 - n.x),
        }
    }

    pub fn pdf_rect_to_norm(&self, rect: [f32; 4]) -> NormRect {
        let p1 = self.pdf_to_norm(PdfPoint::new(rect[0], rect[1]));
        let p2 = self.pdf_to_norm(PdfPoint::new(rect[2], rect[3]));
        NormRect::from_points(p1, p2)
    }

    pub fn norm_rect_to_pdf(&self, rect: NormRect) -> [f32; 4] {
        let p1 = self.norm_to_pdf(NormPoint::new(rect.min_x, rect.min_y));
        let p2 = self.norm_to_pdf(NormPoint::new(rect.max_x, rect.max_y));
        [
            p1.x.min(p2.x),
            p1.y.min(p2.y),
            p1.x.max(p2.x),
            p1.y.max(p2.y),
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_close(a: f32, b: f32) {
        assert!((a - b).abs() < 1e-4, "expected {a} ≈ {b}");
    }

    #[test]
    fn round_trip_all_rotations_with_nonzero_cropbox() {
        let crop_box = [36.0, 50.0, 576.0, 742.0]; // w = 540, h = 692
        let sample_pdf = [
            PdfPoint::new(36.0, 50.0),
            PdfPoint::new(576.0, 742.0),
            PdfPoint::new(120.5, 412.25),
            PdfPoint::new(300.0, 200.0),
        ];

        for deg in [0, 90, 180, 270, -90, 450] {
            let rot = PageRotation::from_degrees(deg);
            let tr = PageTransform::new(crop_box, rot);
            let (vw, vh) = tr.visual_size();
            match rot {
                PageRotation::Deg0 | PageRotation::Deg180 => {
                    assert_close(vw, 540.0);
                    assert_close(vh, 692.0);
                }
                PageRotation::Deg90Cw | PageRotation::Deg270Cw => {
                    assert_close(vw, 692.0);
                    assert_close(vh, 540.0);
                }
            }

            for &pt in &sample_pdf {
                let norm = tr.pdf_to_norm(pt);
                assert!(norm.x >= -1e-4 && norm.x <= vw + 1e-4);
                assert!(norm.y >= -1e-4 && norm.y <= vh + 1e-4);
                let back = tr.norm_to_pdf(norm);
                assert_close(back.x, pt.x);
                assert_close(back.y, pt.y);
            }
        }
    }

    #[test]
    fn visual_corners_map_correctly_for_90_cw() {
        // On a 100x200 page rotated 90 CW, visual size is 200x100.
        // Raw PDF top-left (0, 200) moves to visual top-right (200, 0).
        // Raw PDF bottom-left (0, 0) moves to visual top-left (0, 0).
        let tr = PageTransform::new([0.0, 0.0, 100.0, 200.0], PageRotation::Deg90Cw);
        let tl = tr.pdf_to_norm(PdfPoint::new(0.0, 0.0));
        let tr_pt = tr.pdf_to_norm(PdfPoint::new(0.0, 200.0));
        let br_pt = tr.pdf_to_norm(PdfPoint::new(100.0, 200.0));
        assert_close(tl.x, 0.0);
        assert_close(tl.y, 0.0);
        assert_close(tr_pt.x, 200.0);
        assert_close(tr_pt.y, 0.0);
        assert_close(br_pt.x, 200.0);
        assert_close(br_pt.y, 100.0);
    }

    #[test]
    fn quad_unordered_normalization() {
        // ISO-32000 CCW order: BL, BR, TL, TR
        let pts = [
            NormPoint::new(10.0, 40.0), // BL
            NormPoint::new(90.0, 40.0), // BR
            NormPoint::new(10.0, 20.0), // TL
            NormPoint::new(90.0, 20.0), // TR
        ];
        let q = NormQuad::from_unordered_points(pts);
        assert_eq!(q.corners[0], NormPoint::new(10.0, 20.0));
        assert_eq!(q.corners[1], NormPoint::new(90.0, 20.0));
        assert_eq!(q.corners[2], NormPoint::new(10.0, 40.0));
        assert_eq!(q.corners[3], NormPoint::new(90.0, 40.0));
        assert!(q.contains(NormPoint::new(50.0, 30.0)));
        assert!(!q.contains(NormPoint::new(5.0, 30.0)));
    }
}
