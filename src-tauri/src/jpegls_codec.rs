//! The JPEG-LS (ITU-T T.87, LOCO-I) sample coder: contexts, regular and run modes, limited Golomb codes.

const J: [u32; 32] = [0, 0, 0, 0, 1, 1, 1, 1, 2, 2, 2, 2, 3, 3, 3, 3, 4, 4, 5, 5, 6, 6, 7, 7, 8, 9, 10, 11, 12, 13, 14, 15];
const RUN0: usize = 365; // run-interruption contexts follow the 365 regular ones

struct Bits<'a> {
    d: &'a [u8],
    pos: usize,
    val: u8,
    left: u32,
    prev_ff: bool,
}

impl Bits<'_> {
    fn bit(&mut self) -> Result<u32, String> {
        if self.left == 0 {
            self.val = *self.d.get(self.pos).ok_or("truncated JPEG-LS data")?;
            self.pos += 1;
            self.left = if self.prev_ff { 7 } else { 8 }; // the byte after 0xFF carries 7 bits
            self.prev_ff = self.val == 0xFF;
        }
        self.left -= 1;
        Ok(u32::from(self.val >> self.left) & 1)
    }
    fn bits(&mut self, n: u32) -> Result<u32, String> {
        (0..n).try_fold(0, |a, _| Ok(a << 1 | self.bit()?))
    }
}

pub(crate) struct Params {
    maxval: i32,
    near: i32,
    range: i32,
    qbpp: u32,
    limit: u32,
    t: [i32; 3],
    reset: i32,
}

fn ceil_log2(v: i32) -> u32 {
    (0..32).find(|b| (1i64 << b) >= i64::from(v)).unwrap_or(32)
}

pub(crate) fn params(maxval: i32, near: i32, preset: [i32; 4]) -> Result<Params, String> {
    let (b, lo) = ([3, 7, 21], near + 1);
    let mut t = [0; 3];
    for i in 0..3 {
        let (base, add) = ([2, 3, 4][i], [3, 5, 7][i] * near);
        t[i] = if maxval >= 128 {
            ((maxval.min(4095) + 128) / 256 * (b[i] - base) + base + add).clamp(lo, maxval)
        } else {
            ((b[i] / (256 / (maxval + 1))).max(base) + add).clamp(lo, maxval)
        };
        if i > 0 {
            t[i] = t[i].max(t[i - 1]);
        }
        if preset[i] != 0 {
            t[i] = preset[i];
        }
    }
    let reset = if preset[3] != 0 { preset[3] } else { 64 };
    if !(lo <= t[0] && t[0] <= t[1] && t[1] <= t[2] && t[2] <= maxval) || !(3..=255).contains(&reset) {
        return Err("invalid JPEG-LS coding parameters".into());
    }
    let range = (maxval + 2 * near) / (2 * near + 1) + 1;
    let bpp = ceil_log2(maxval + 1).max(2);
    Ok(Params { maxval, near, range, qbpp: ceil_log2(range), limit: 2 * (bpp + bpp.max(8)), t, reset })
}

/// One component's current and previous line, each padded by one sample on both sides.
pub(crate) struct Comp {
    pub prev: Vec<i32>,
    pub cur: Vec<i32>,
}

pub(crate) struct Dec<'a> {
    p: Params,
    bits: Bits<'a>,
    a: Vec<i32>,
    b: Vec<i32>,
    c: Vec<i32>,
    n: Vec<i32>,
    nn: [i32; 2],
}

impl<'a> Dec<'a> {
    pub fn new(p: Params, data: &'a [u8]) -> Self {
        let a0 = ((p.range + 32) / 64).max(2);
        Dec { p, bits: Bits { d: data, pos: 0, val: 0, left: 0, prev_ff: false }, a: vec![a0; 367], b: vec![0; 367], c: vec![0; 367], n: vec![1; 367], nn: [0; 2] }
    }

    fn golomb(&mut self, k: u32, limit: u32) -> Result<i32, String> {
        let max_z = limit.saturating_sub(self.p.qbpp + 1);
        let mut z = 0;
        while self.bits.bit()? == 0 {
            z += 1;
            if z > max_z {
                return Err("corrupt JPEG-LS data".into());
            }
        }
        Ok(if z >= max_z { self.bits.bits(self.p.qbpp)? as i32 + 1 } else { ((z as i32) << k) + self.bits.bits(k)? as i32 })
    }

    fn fix(&self, v: i32) -> i32 {
        let step = self.p.range * (2 * self.p.near + 1);
        (if v < -self.p.near { v + step } else if v > self.p.maxval + self.p.near { v - step } else { v }).clamp(0, self.p.maxval)
    }

    fn quant(&self, d: i32) -> i32 {
        let (t, n) = (&self.p.t, self.p.near);
        match d {
            d if d <= -t[2] => -4,
            d if d <= -t[1] => -3,
            d if d <= -t[0] => -2,
            d if d < -n => -1,
            d if d <= n => 0,
            d if d < t[0] => 1,
            d if d < t[1] => 2,
            d if d < t[2] => 3,
            _ => 4,
        }
    }

    fn gradients(&self, [ra, rb, rc, rd]: [i32; 4]) -> [i32; 3] {
        [self.quant(rd - rb), self.quant(rb - rc), self.quant(rc - ra)]
    }

    fn regular(&mut self, [ra, rb, rc, _]: [i32; 4], q: [i32; 3]) -> Result<i32, String> {
        let sign = if q[0] < 0 || (q[0] == 0 && (q[1] < 0 || (q[1] == 0 && q[2] < 0))) { -1 } else { 1 };
        let ctx = (sign * (81 * q[0] + 9 * q[1] + q[2])) as usize;
        let med = if rc >= ra.max(rb) { ra.min(rb) } else if rc <= ra.min(rb) { ra.max(rb) } else { ra + rb - rc };
        let px = (med + sign * self.c[ctx]).clamp(0, self.p.maxval);
        let mut k = 0;
        while (self.n[ctx] << k) < self.a[ctx] {
            k += 1;
        }
        let m = self.golomb(k, self.p.limit)?;
        let mut e = if m & 1 == 1 { -((m + 1) / 2) } else { m / 2 };
        if self.p.near == 0 && k == 0 && 2 * self.b[ctx] <= -self.n[ctx] {
            e = -(e + 1);
        }
        self.b[ctx] += e * (2 * self.p.near + 1);
        self.a[ctx] += e.abs();
        if self.n[ctx] == self.p.reset {
            self.a[ctx] >>= 1;
            self.b[ctx] >>= 1;
            self.n[ctx] >>= 1;
        }
        self.n[ctx] += 1;
        let n = self.n[ctx];
        if self.b[ctx] <= -n {
            self.b[ctx] = (self.b[ctx] + n).max(-n + 1);
            self.c[ctx] = (self.c[ctx] - 1).max(-128);
        } else if self.b[ctx] > 0 {
            self.b[ctx] = (self.b[ctx] - n).min(0);
            self.c[ctx] = (self.c[ctx] + 1).min(127);
        }
        Ok(self.fix(px + sign * e * (2 * self.p.near + 1)))
    }

    /// Length of a run of at most `rem` samples, ending at the line end or before an interruption sample.
    fn run(&mut self, rem: usize, ri: &mut usize) -> Result<usize, String> {
        let mut count = 0;
        while self.bits.bit()? == 1 {
            let full = 1usize << J[*ri];
            let take = full.min(rem - count);
            count += take;
            if take == full && *ri < 31 {
                *ri += 1;
            }
            if count == rem {
                return Ok(count);
            }
        }
        count += self.bits.bits(J[*ri])? as usize;
        if count > rem {
            return Err("corrupt JPEG-LS run".into());
        }
        Ok(count)
    }

    /// Error of a run-interruption sample in context `RUN0 + ty`.
    fn interruption_error(&mut self, ty: usize, ri: usize) -> Result<i32, String> {
        let idx = RUN0 + ty;
        let temp = self.a[idx] + (self.n[idx] >> 1) * ty as i32;
        let mut k = 0;
        while (self.n[idx] << k) < temp {
            k += 1;
        }
        let m = self.golomb(k, self.p.limit - J[ri] - 1)?;
        let t = m + ty as i32;
        let map = t & 1 == 1;
        let abs = (t + i32::from(map)) / 2;
        let e = if (k != 0 || 2 * self.nn[ty] >= self.n[idx]) == map { -abs } else { abs };
        if e < 0 {
            self.nn[ty] += 1;
        }
        self.a[idx] += (m + 1 - ty as i32) >> 1;
        if self.n[idx] == self.p.reset {
            self.a[idx] >>= 1;
            self.n[idx] >>= 1;
            self.nn[ty] >>= 1;
        }
        self.n[idx] += 1;
        Ok(e)
    }

    /// Sample-interleaved scans always use the "Ra differs from Rb" context.
    fn interruption(&mut self, ra: i32, rb: i32, ri: usize, joint: bool) -> Result<i32, String> {
        let step = 2 * self.p.near + 1;
        if !joint && (ra - rb).abs() <= self.p.near {
            let e = self.interruption_error(1, ri)?;
            Ok(self.fix(ra + e * step))
        } else {
            let e = self.interruption_error(0, ri)?;
            Ok(self.fix(rb + e * step * if rb < ra { -1 } else { 1 }))
        }
    }

    /// One line of the components in `cs` (several only when sample-interleaved); the result ends up in `prev`.
    pub fn line(&mut self, cs: &mut [Comp], ri: &mut usize, w: usize) -> Result<(), String> {
        for c in cs.iter_mut() {
            c.cur[0] = c.prev[1];
            c.prev[w + 1] = c.prev[w];
        }
        let joint = cs.len() > 1;
        let mut i = 1;
        while i <= w {
            let mut q = [[0; 3]; 3];
            for (k, c) in cs.iter().enumerate() {
                q[k] = self.gradients([c.cur[i - 1], c.prev[i], c.prev[i - 1], c.prev[i + 1]]);
            }
            if q.iter().take(cs.len()).all(|g| *g == [0; 3]) {
                let n = self.run(w - i + 1, ri)?;
                for c in cs.iter_mut() {
                    let ra = c.cur[i - 1];
                    c.cur[i..i + n].fill(ra);
                }
                i += n;
                if i <= w {
                    for c in cs.iter_mut() {
                        c.cur[i] = self.interruption(c.cur[i - 1], c.prev[i], *ri, joint)?;
                    }
                    *ri = ri.saturating_sub(1);
                    i += 1;
                }
            } else {
                for (k, c) in cs.iter_mut().enumerate() {
                    c.cur[i] = self.regular([c.cur[i - 1], c.prev[i], c.prev[i - 1], c.prev[i + 1]], q[k])?;
                }
                i += 1;
            }
        }
        for c in cs.iter_mut() {
            std::mem::swap(&mut c.prev, &mut c.cur);
        }
        Ok(())
    }
}
