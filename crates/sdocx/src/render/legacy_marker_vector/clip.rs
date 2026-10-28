use std::convert::TryFrom;
#[derive(Clone, Copy, Debug)]
struct Q {
    n: i128,
    d: i128,
}
fn gcd(mut a: u128, mut b: u128) -> u128 {
    while b != 0 {
        (a, b) = (b, a % b)
    }
    a
}
impl Q {
    fn new(n: i128, d: i128) -> Option<Self> {
        if d <= 0 {
            return None;
        }
        let g = i128::try_from(gcd(n.unsigned_abs(), d as u128)).ok()?;
        Some(Self {
            n: n.checked_div(g)?,
            d: d.checked_div(g)?,
        })
    }
    fn i(n: i128) -> Self {
        Self { n, d: 1 }
    }
    fn add(self, b: Self) -> Option<Self> {
        let g = i128::try_from(gcd(self.d as u128, b.d as u128)).ok()?;
        let x = self.d / g;
        let y = b.d / g;
        Self::new(
            self.n.checked_mul(y)?.checked_add(b.n.checked_mul(x)?)?,
            x.checked_mul(b.d)?,
        )
    }
    fn neg(self) -> Option<Self> {
        Some(Self {
            n: self.n.checked_neg()?,
            d: self.d,
        })
    }
    fn sub(self, b: Self) -> Option<Self> {
        self.add(b.neg()?)
    }
    fn mul(self, b: Self) -> Option<Self> {
        let g = i128::try_from(gcd(self.n.unsigned_abs(), b.d as u128)).ok()?;
        let h = i128::try_from(gcd(b.n.unsigned_abs(), self.d as u128)).ok()?;
        Self::new(
            (self.n / g).checked_mul(b.n / h)?,
            (self.d / h).checked_mul(b.d / g)?,
        )
    }
    fn div(self, b: Self) -> Option<Self> {
        if b.n == 0 {
            return None;
        }
        let (n, d) = if b.n < 0 {
            (b.d.checked_neg()?, b.n.checked_neg()?)
        } else {
            (b.d, b.n)
        };
        self.mul(Self { n, d })
    }
    fn f(self) -> f64 {
        self.n as f64 / self.d as f64
    }
}
type P = [Q; 2];
type Poly = Vec<P>;
fn cross(a: P, b: P, p: P) -> Option<Q> {
    b[0].sub(a[0])?
        .mul(p[1].sub(a[1])?)?
        .sub(b[1].sub(a[1])?.mul(p[0].sub(a[0])?)?)
}
fn half(p: &[P], a: P, b: P, inside: bool, ops: &mut usize, limit: usize) -> Option<Poly> {
    let mut out = Vec::new();
    let mut prev = *p.last()?;
    let mut dp = cross(a, b, prev)?;
    for &cur in p {
        if *ops >= limit {
            return None;
        }
        *ops = ops.checked_add(1)?;
        let dc = cross(a, b, cur)?;
        let ip = if inside { dp.n >= 0 } else { dp.n <= 0 };
        let ic = if inside { dc.n >= 0 } else { dc.n <= 0 };
        if ip != ic {
            let t = dp.div(dp.sub(dc)?)?;
            out.push([
                prev[0].add(t.mul(cur[0].sub(prev[0])?)?)?,
                prev[1].add(t.mul(cur[1].sub(prev[1])?)?)?,
            ]);
        }
        if ic {
            out.push(cur)
        }
        if out.len() > 128 {
            return None;
        }
        prev = cur;
        dp = dc;
    }
    Some(if out.len() < 3 { vec![] } else { out })
}
fn minus(p: &[P], c: &[P], ops: &mut usize, limit: usize) -> Option<Vec<Poly>> {
    let mut left = p.to_vec();
    let mut out = vec![];
    for i in 0..c.len() {
        let x = half(&left, c[i], c[(i + 1) % c.len()], false, ops, limit)?;
        if !x.is_empty() {
            out.push(x)
        }
        left = half(&left, c[i], c[(i + 1) % c.len()], true, ops, limit)?;
        if left.is_empty() {
            break;
        }
    }
    Some(out)
}
fn exact_orientation(t: &[[f32; 2]]) -> Option<i8> {
    let mut values = [(0i128, 0i32); 6];
    let mut min_exp = i32::MAX;
    for (i, v) in t.iter().take(3).flat_map(|p| [&p[0], &p[1]]).enumerate() {
        let n = *v;
        if !n.is_finite() || n.abs() > 1_000_000. {
            return None;
        }
        let bits = n.to_bits();
        let exp = (bits >> 23) & 255;
        let frac = bits & 0x7fffff;
        let (mantissa, e) = if exp == 0 {
            (frac, -149)
        } else {
            (frac | 0x800000, exp as i32 - 150)
        };
        let signed = if bits >> 31 == 1 {
            -(mantissa as i128)
        } else {
            mantissa as i128
        };
        values[i] = (signed, e);
        if signed != 0 {
            min_exp = min_exp.min(e)
        }
    }
    if min_exp == i32::MAX {
        return Some(0);
    }
    let mut z = [0i128; 6];
    for (i, (m, e)) in values.into_iter().enumerate() {
        if m != 0 {
            let shift = u32::try_from(e - min_exp).ok()?;
            if shift > 55 {
                return None;
            }
            z[i] = m.checked_shl(shift)?
        }
    }
    let ax = z[2].checked_sub(z[0])?;
    let ay = z[3].checked_sub(z[1])?;
    let bx = z[4].checked_sub(z[0])?;
    let by = z[5].checked_sub(z[1])?;
    Some(
        ax.checked_mul(by)?
            .checked_sub(ay.checked_mul(bx)?)?
            .signum() as i8,
    )
}
fn bbox(p: &[P]) -> [i128; 4] {
    [
        p.iter().map(|v| v[0].n).min().unwrap(),
        p.iter().map(|v| v[1].n).min().unwrap(),
        p.iter().map(|v| v[0].n).max().unwrap(),
        p.iter().map(|v| v[1].n).max().unwrap(),
    ]
}
fn intersects(a: [i128; 4], b: [i128; 4]) -> bool {
    a[0] < b[2] && b[0] < a[2] && a[1] < b[3] && b[1] < a[3]
}
fn piece_intersects(piece: &[P], bounds: [i128; 4], ops: &mut usize, limit: usize) -> Option<bool> {
    let mut left = false;
    let mut right = false;
    let mut above = false;
    let mut below = false;
    for point in piece {
        if *ops >= limit {
            return None;
        }
        *ops = ops.checked_add(1)?;
        let x = point[0];
        let y = point[1];
        left |= x.n < bounds[2].checked_mul(x.d)?;
        right |= x.n > bounds[0].checked_mul(x.d)?;
        above |= y.n < bounds[3].checked_mul(y.d)?;
        below |= y.n > bounds[1].checked_mul(y.d)?;
        if left && right && above && below {
            return Some(true);
        }
    }
    Some(false)
}
pub(super) type VisiblePolygon = Vec<Vec<Vec<[f64; 2]>>>;
pub(super) type VisiblePolygons = Vec<VisiblePolygon>;

pub(super) fn run(
    mesh: &crate::ink::legacy_marker_mesh::Mesh,
    ops: &mut usize,
    limit: usize,
    collapsed: &mut bool,
    grid: f64,
) -> Option<(VisiblePolygons, usize)> {
    let body = &mesh.body;
    let cap = &mesh.caps[0];
    let fringe: Vec<Vec<[f32; 2]>> = mesh
        .fringe
        .iter()
        .map(|v| v.iter().map(|p| [p[0], p[1]]).collect())
        .collect();
    let n = body
        .len()
        .checked_add(cap.len())?
        .checked_add(fringe.iter().map(Vec::len).sum::<usize>())?;
    if n > 1024 || body.is_empty() || cap.len() != 6 {
        return None;
    }
    let origin = [body[0][0] as f64, body[0][1] as f64];
    let point = |v: &[f32; 2]| -> Option<P> {
        let x = v[0] as f64;
        let y = v[1] as f64;
        if !x.is_finite() || !y.is_finite() || x.abs() > 1e6 || y.abs() > 1e6 {
            return None;
        }
        Some([
            Q::i(((x - origin[0]) * grid).round() as i128),
            Q::i(((y - origin[1]) * grid).round() as i128),
        ])
    };
    let mut orient = |t: &[[f32; 2]]| -> Option<Option<Poly>> {
        let mut p: Poly = t.iter().map(&point).collect::<Option<_>>()?;
        let a = cross(p[0], p[1], p[2])?;
        let src = exact_orientation(t)?;
        if (a.n == 0) != (src == 0) || (a.n > 0 && src < 0) || (a.n < 0 && src > 0) {
            *collapsed = true;
            return None;
        }
        if a.n == 0 {
            return Some(None);
        }
        if p.len() > 3 {
            for i in 0..p.len() {
                let triple = [t[i], t[(i + 1) % p.len()], t[(i + 2) % p.len()]];
                let exact = exact_orientation(&triple)?;
                let quant = cross(p[i], p[(i + 1) % p.len()], p[(i + 2) % p.len()])?
                    .n
                    .signum() as i8;
                if exact == 0 || quant != exact || exact != (a.n.signum() as i8) {
                    *collapsed = true;
                    return None;
                }
            }
        }
        if a.n < 0 {
            p.reverse()
        }
        if p.len() > 3 {
            for i in 0..p.len() {
                if cross(p[i], p[(i + 1) % p.len()], p[(i + 2) % p.len()])?.n <= 0 {
                    return None;
                }
            }
        }
        Some(Some(p))
    };
    for side in &fringe {
        for t in side.windows(3) {
            if *ops >= limit {
                return None;
            }
            *ops = ops.checked_add(1)?;
            orient(t)?;
        }
    }
    let mut owners = vec![];
    for t in body.windows(3) {
        if let Some(p) = orient(t)? {
            owners.push(p)
        }
    }
    let c = [
        [cap[0][0], cap[0][1]],
        [cap[1][0], cap[1][1]],
        [cap[4][0], cap[4][1]],
        [cap[2][0], cap[2][1]],
    ];
    owners.push(orient(&c)??);
    let mut out = vec![];
    let mut total = 0usize;
    for side in &fringe {
        for t in side.windows(3) {
            let Some(tri) = orient(t)? else { continue };
            let box_tri = bbox(&tri);
            let mut pieces = vec![tri.clone()];
            for owner in &owners {
                if *ops >= limit {
                    return None;
                }
                *ops = ops.checked_add(1)?;
                let owner_bounds = bbox(owner);
                if !intersects(box_tri, owner_bounds) {
                    continue;
                }
                let mut next = vec![];
                for piece in pieces {
                    if piece_intersects(&piece, owner_bounds, ops, limit)? {
                        next.extend(minus(&piece, owner, ops, limit)?);
                    } else {
                        next.push(piece);
                    }
                    if next.len() > 256 {
                        return None;
                    }
                }
                pieces = next;
                if pieces.is_empty() {
                    break;
                }
            }
            total = total.checked_add(pieces.iter().map(Vec::len).sum::<usize>())?;
            if total > 12_000 {
                return None;
            }
            let converted: Vec<_> = pieces
                .iter()
                .map(|p| {
                    vec![
                        p.iter()
                            .map(|v| [v[0].f() / grid + origin[0], v[1].f() / grid + origin[1]])
                            .collect::<Vec<_>>(),
                    ]
                })
                .collect();
            out.push(converted);
            owners.push(tri);
        }
    }
    Some((out, total))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn exact_orientation_rejects_unbounded_exponent_spread() {
        assert_eq!(exact_orientation(&[[0., 0.], [1., 0.], [0., 1.]]), Some(1));
        assert_eq!(exact_orientation(&[[0., 0.], [0., 1.], [1., 0.]]), Some(-1));
        assert_eq!(exact_orientation(&[[0., 0.], [1., 1.], [2., 2.]]), Some(0));
        assert_eq!(
            exact_orientation(&[[0., 0.], [1., 0.], [0., f32::from_bits(1)]]),
            None
        );
    }
    #[test]
    fn clipping_stops_at_the_given_work_limit() {
        let mesh = crate::ink::legacy_marker_mesh::generate_bounded_mesh(
            &[
                [10., 20.],
                [10., 28.],
                [18., 28.],
                [18., 36.],
                [26., 36.],
                [26., 44.],
            ],
            12.,
            [1920, 1080],
        )
        .unwrap();
        let mut work = 0;
        let mut collapsed = false;
        assert!(run(&mesh, &mut work, 10, &mut collapsed, 10_000.).is_none());
        assert_eq!(work, 10);
    }
    #[test]
    fn exact_piece_bounds_skip_disjoint_or_touching_fragments() {
        let half = Q::new(1, 2).unwrap();
        let piece = vec![[half, half], [Q::i(1), half], [half, Q::i(1)]];
        let mut work = 0;
        assert_eq!(
            piece_intersects(&piece, [0, 0, 1, 1], &mut work, 100),
            Some(true)
        );
        assert_eq!(
            piece_intersects(&piece, [1, 0, 2, 1], &mut work, 100),
            Some(false)
        );
        assert_eq!(
            piece_intersects(&piece, [2, 0, 3, 1], &mut work, 100),
            Some(false)
        );
        let mut exhausted = 0;
        assert_eq!(
            piece_intersects(&piece, [0, 0, 1, 1], &mut exhausted, 0),
            None
        );
        assert_eq!(exhausted, 0);
    }
}
