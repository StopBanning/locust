//! A QR code, square for square what Electron's `qrcode-generator` 2.0.4 makes for `qrcode(0, 'M')` with one
//! `addData(text)` (PhoneSettings.tsx's `QrCode`): byte mode, error correction M, the smallest version that holds the
//! text, and the library's own mask choice. Ported from its source rather than a crate (no new crate without Colin's
//! yes). Versions 1 to 10 only: the phone's link is about 60 characters, and version 10 at M holds 213 bytes.

/// Dark squares, row by row.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct QrCode {
    pub size: usize,
    dark: Vec<bool>,
}

impl QrCode {
    pub fn is_dark(&self, row: usize, col: usize) -> bool {
        self.dark[row * self.size + col]
    }

    /// PhoneSettings.tsx's SVG path: one `M{col+4} {row+4}h1v1h-1z` per dark square, inside a four-square margin, so
    /// the picture is `size + 8` squares wide.
    pub fn svg_path(&self) -> String {
        let mut d = String::new();
        for row in 0..self.size {
            for col in 0..self.size {
                if self.is_dark(row, col) {
                    d.push_str(&format!("M{} {}h1v1h-1z", col + 4, row + 4));
                }
            }
        }
        d
    }
}

/// `(count, total, data)` triples for error correction M, versions 1 to 10.
const RS_BLOCKS_M: [&[usize]; 10] = [
    &[1, 26, 16],
    &[1, 44, 28],
    &[1, 70, 44],
    &[2, 50, 32],
    &[2, 67, 43],
    &[4, 43, 27],
    &[4, 49, 31],
    &[2, 60, 38, 2, 61, 39],
    &[3, 58, 36, 2, 59, 37],
    &[4, 69, 43, 1, 70, 44],
];

const PATTERN_POSITION: [&[usize]; 10] = [
    &[],
    &[6, 18],
    &[6, 22],
    &[6, 26],
    &[6, 30],
    &[6, 34],
    &[6, 22, 38],
    &[6, 24, 42],
    &[6, 26, 46],
    &[6, 28, 50],
];

/// Error correction M's two bits.
const LEVEL_M: u32 = 0;
const MODE_BYTE: u32 = 4;
const PAD0: u32 = 0xEC;
const PAD1: u32 = 0x11;

const G15: u32 = (1 << 10) | (1 << 8) | (1 << 5) | (1 << 4) | (1 << 2) | (1 << 1) | 1;
const G18: u32 = (1 << 12) | (1 << 11) | (1 << 10) | (1 << 9) | (1 << 8) | (1 << 5) | (1 << 2) | 1;
const G15_MASK: u32 = (1 << 14) | (1 << 12) | (1 << 10) | (1 << 4) | (1 << 1);

fn bch_digit(mut data: u32) -> i32 {
    let mut digit = 0;
    while data != 0 {
        digit += 1;
        data >>= 1;
    }
    digit
}

fn bch_type_info(data: u32) -> u32 {
    let mut d = data << 10;
    while bch_digit(d) - bch_digit(G15) >= 0 {
        d ^= G15 << (bch_digit(d) - bch_digit(G15));
    }
    ((data << 10) | d) ^ G15_MASK
}

fn bch_type_number(data: u32) -> u32 {
    let mut d = data << 12;
    while bch_digit(d) - bch_digit(G18) >= 0 {
        d ^= G18 << (bch_digit(d) - bch_digit(G18));
    }
    (data << 12) | d
}

fn mask(pattern: u8, i: usize, j: usize) -> bool {
    match pattern {
        0 => (i + j) % 2 == 0,
        1 => i % 2 == 0,
        2 => j % 3 == 0,
        3 => (i + j) % 3 == 0,
        4 => (i / 2 + j / 3) % 2 == 0,
        5 => (i * j) % 2 + (i * j) % 3 == 0,
        6 => ((i * j) % 2 + (i * j) % 3) % 2 == 0,
        _ => ((i * j) % 3 + (i + j) % 2) % 2 == 0,
    }
}

struct Gf {
    exp: [u32; 256],
    log: [u32; 256],
}

impl Gf {
    fn new() -> Self {
        let mut exp = [0u32; 256];
        let mut log = [0u32; 256];
        for i in 0..8 {
            exp[i] = 1 << i;
        }
        for i in 8..256 {
            exp[i] = exp[i - 4] ^ exp[i - 5] ^ exp[i - 6] ^ exp[i - 8];
        }
        for i in 0..255 {
            log[exp[i] as usize] = i as u32;
        }
        Self { exp, log }
    }

    fn glog(&self, n: u32) -> i64 {
        self.log[n as usize] as i64
    }

    fn gexp(&self, mut n: i64) -> u32 {
        while n < 0 {
            n += 255;
        }
        while n >= 256 {
            n -= 255;
        }
        self.exp[n as usize]
    }
}

/// `qrPolynomial`: leading zeros dropped, `shift` zeros appended.
fn poly(num: &[u32], shift: usize) -> Vec<u32> {
    let offset = num.iter().take_while(|n| **n == 0).count();
    let mut out = num[offset..].to_vec();
    out.extend(std::iter::repeat_n(0, shift));
    out
}

fn poly_multiply(gf: &Gf, a: &[u32], e: &[u32]) -> Vec<u32> {
    let mut num = vec![0u32; a.len() + e.len() - 1];
    for (i, x) in a.iter().enumerate() {
        for (j, y) in e.iter().enumerate() {
            num[i + j] ^= gf.gexp(gf.glog(*x) + gf.glog(*y));
        }
    }
    poly(&num, 0)
}

fn poly_mod(gf: &Gf, a: Vec<u32>, e: &[u32]) -> Vec<u32> {
    let mut a = a;
    loop {
        if a.len() < e.len() {
            return a;
        }
        let ratio = gf.glog(a[0]) - gf.glog(e[0]);
        let mut num = a.clone();
        for (i, y) in e.iter().enumerate() {
            num[i] ^= gf.gexp(gf.glog(*y) + ratio);
        }
        a = poly(&num, 0);
    }
}

fn error_correct_polynomial(gf: &Gf, length: usize) -> Vec<u32> {
    let mut a = poly(&[1], 0);
    for i in 0..length {
        a = poly_multiply(gf, &a, &poly(&[1, gf.gexp(i as i64)], 0));
    }
    a
}

#[derive(Default)]
struct Bits {
    buffer: Vec<u8>,
    length: usize,
}

impl Bits {
    fn put(&mut self, num: u32, length: usize) {
        for i in 0..length {
            self.put_bit(((num >> (length - i - 1)) & 1) == 1);
        }
    }

    fn put_bit(&mut self, bit: bool) {
        let at = self.length / 8;
        if self.buffer.len() <= at {
            self.buffer.push(0);
        }
        if bit {
            self.buffer[at] |= 0x80 >> (self.length % 8);
        }
        self.length += 1;
    }
}

fn rs_blocks(version: usize) -> Vec<(usize, usize)> {
    let row = RS_BLOCKS_M[version - 1];
    let mut list = Vec::new();
    for triple in row.chunks(3) {
        for _ in 0..triple[0] {
            list.push((triple[1], triple[2]));
        }
    }
    list
}

fn length_bits(version: usize) -> usize {
    if version < 10 { 8 } else { 16 }
}

fn data_bits(bytes: &[u8], version: usize) -> Bits {
    let mut bits = Bits::default();
    bits.put(MODE_BYTE, 4);
    bits.put(bytes.len() as u32, length_bits(version));
    for byte in bytes {
        bits.put(u32::from(*byte), 8);
    }
    bits
}

fn create_data(gf: &Gf, bytes: &[u8], version: usize) -> Vec<u8> {
    let blocks = rs_blocks(version);
    let mut bits = data_bits(bytes, version);
    let total_data: usize = blocks.iter().map(|(_, data)| data).sum();
    if bits.length + 4 <= total_data * 8 {
        bits.put(0, 4);
    }
    while bits.length % 8 != 0 {
        bits.put_bit(false);
    }
    loop {
        if bits.length >= total_data * 8 {
            break;
        }
        bits.put(PAD0, 8);
        if bits.length >= total_data * 8 {
            break;
        }
        bits.put(PAD1, 8);
    }
    let mut offset = 0;
    let (mut dc, mut ec) = (Vec::new(), Vec::new());
    for (total, data) in &blocks {
        let block: Vec<u32> = bits.buffer[offset..offset + data].iter().map(|b| u32::from(*b)).collect();
        offset += data;
        let rs = error_correct_polynomial(gf, total - data);
        let modded = poly_mod(gf, poly(&block, rs.len() - 1), &rs);
        let ec_len = rs.len() - 1;
        let ecs: Vec<u32> = (0..ec_len)
            .map(|i| {
                let at = i as i64 + modded.len() as i64 - ec_len as i64;
                if at >= 0 { modded[at as usize] } else { 0 }
            })
            .collect();
        dc.push(block);
        ec.push(ecs);
    }
    let mut out = Vec::new();
    let most_dc = dc.iter().map(Vec::len).max().unwrap_or(0);
    let most_ec = ec.iter().map(Vec::len).max().unwrap_or(0);
    for i in 0..most_dc {
        out.extend(dc.iter().filter_map(|block| block.get(i)).map(|b| *b as u8));
    }
    for i in 0..most_ec {
        out.extend(ec.iter().filter_map(|block| block.get(i)).map(|b| *b as u8));
    }
    out
}

struct Grid {
    size: usize,
    cells: Vec<Option<bool>>,
}

impl Grid {
    fn get(&self, row: usize, col: usize) -> Option<bool> {
        self.cells[row * self.size + col]
    }
    fn set(&mut self, row: usize, col: usize, dark: bool) {
        self.cells[row * self.size + col] = Some(dark);
    }
    fn dark(&self, row: usize, col: usize) -> bool {
        self.get(row, col) == Some(true)
    }
}

fn probe(grid: &mut Grid, row: usize, col: usize) {
    for r in -1i64..=7 {
        let rr = row as i64 + r;
        if rr <= -1 || grid.size as i64 <= rr {
            continue;
        }
        for c in -1i64..=7 {
            let cc = col as i64 + c;
            if cc <= -1 || grid.size as i64 <= cc {
                continue;
            }
            let dark = ((0..=6).contains(&r) && (c == 0 || c == 6))
                || ((0..=6).contains(&c) && (r == 0 || r == 6))
                || ((2..=4).contains(&r) && (2..=4).contains(&c));
            grid.set(rr as usize, cc as usize, dark);
        }
    }
}

fn make(version: usize, data: &[u8], test: bool, pattern: u8) -> Grid {
    let size = version * 4 + 17;
    let mut grid = Grid { size, cells: vec![None; size * size] };
    probe(&mut grid, 0, 0);
    probe(&mut grid, size - 7, 0);
    probe(&mut grid, 0, size - 7);
    let pos = PATTERN_POSITION[version - 1];
    for &row in pos {
        for &col in pos {
            if grid.get(row, col).is_some() {
                continue;
            }
            for r in -2i64..=2 {
                for c in -2i64..=2 {
                    let dark = r == -2 || r == 2 || c == -2 || c == 2 || (r == 0 && c == 0);
                    grid.set((row as i64 + r) as usize, (col as i64 + c) as usize, dark);
                }
            }
        }
    }
    for r in 8..size - 8 {
        if grid.get(r, 6).is_none() {
            grid.set(r, 6, r % 2 == 0);
        }
    }
    for c in 8..size - 8 {
        if grid.get(6, c).is_none() {
            grid.set(6, c, c % 2 == 0);
        }
    }
    let bits = bch_type_info((LEVEL_M << 3) | u32::from(pattern));
    for i in 0..15 {
        let dark = !test && ((bits >> i) & 1) == 1;
        if i < 6 {
            grid.set(i, 8, dark);
        } else if i < 8 {
            grid.set(i + 1, 8, dark);
        } else {
            grid.set(size - 15 + i, 8, dark);
        }
    }
    for i in 0..15 {
        let dark = !test && ((bits >> i) & 1) == 1;
        if i < 8 {
            grid.set(8, size - i - 1, dark);
        } else if i < 9 {
            grid.set(8, 15 - i, dark);
        } else {
            grid.set(8, 15 - i - 1, dark);
        }
    }
    grid.set(size - 8, 8, !test);
    if version >= 7 {
        let bits = bch_type_number(version as u32);
        for i in 0..18 {
            let dark = !test && ((bits >> i) & 1) == 1;
            grid.set(i / 3, i % 3 + size - 8 - 3, dark);
        }
        for i in 0..18 {
            let dark = !test && ((bits >> i) & 1) == 1;
            grid.set(i % 3 + size - 8 - 3, i / 3, dark);
        }
    }
    // mapData
    let mut inc: i64 = -1;
    let mut row = size as i64 - 1;
    let mut bit_index: i32 = 7;
    let mut byte_index = 0usize;
    let mut col = size as i64 - 1;
    while col > 0 {
        if col == 6 {
            col -= 1;
        }
        loop {
            for c in 0..2 {
                let (r, cc) = (row as usize, (col - c) as usize);
                if grid.get(r, cc).is_none() {
                    let mut dark = byte_index < data.len() && ((data[byte_index] >> bit_index) & 1) == 1;
                    if mask(pattern, r, cc) {
                        dark = !dark;
                    }
                    grid.set(r, cc, dark);
                    bit_index -= 1;
                    if bit_index == -1 {
                        byte_index += 1;
                        bit_index = 7;
                    }
                }
            }
            row += inc;
            if row < 0 || size as i64 <= row {
                row -= inc;
                inc = -inc;
                break;
            }
        }
        col -= 2;
    }
    grid
}

/// `QRUtil.getLostPoint`, the library's own scoring (not the standard's), so the same mask is picked.
fn lost_point(grid: &Grid) -> f64 {
    let n = grid.size;
    let mut lost = 0.0;
    for row in 0..n {
        for col in 0..n {
            let dark = grid.dark(row, col);
            let mut same = 0;
            for r in -1i64..=1 {
                let rr = row as i64 + r;
                if rr < 0 || n as i64 <= rr {
                    continue;
                }
                for c in -1i64..=1 {
                    let cc = col as i64 + c;
                    if cc < 0 || n as i64 <= cc || (r == 0 && c == 0) {
                        continue;
                    }
                    if dark == grid.dark(rr as usize, cc as usize) {
                        same += 1;
                    }
                }
            }
            if same > 5 {
                lost += f64::from(3 + same - 5);
            }
        }
    }
    for row in 0..n - 1 {
        for col in 0..n - 1 {
            let count = [(row, col), (row + 1, col), (row, col + 1), (row + 1, col + 1)]
                .iter()
                .filter(|(r, c)| grid.dark(*r, *c))
                .count();
            if count == 0 || count == 4 {
                lost += 3.0;
            }
        }
    }
    let finder = [true, false, true, true, true, false, true];
    for row in 0..n {
        for col in 0..n - 6 {
            if (0..7).all(|k| grid.dark(row, col + k) == finder[k]) {
                lost += 40.0;
            }
        }
    }
    for col in 0..n {
        for row in 0..n - 6 {
            if (0..7).all(|k| grid.dark(row + k, col) == finder[k]) {
                lost += 40.0;
            }
        }
    }
    let dark_count = grid.cells.iter().filter(|cell| **cell == Some(true)).count();
    let ratio = (100.0 * dark_count as f64 / n as f64 / n as f64 - 50.0).abs() / 5.0;
    lost + ratio * 10.0
}

/// The code for `text`, or None when it is longer than version 10 holds.
pub fn qr_code(text: &str) -> Option<QrCode> {
    // The library's default stringToBytes: each UTF-16 unit's low byte.
    let bytes: Vec<u8> = text.encode_utf16().map(|unit| (unit & 0xff) as u8).collect();
    let version = (1..=10).find(|&version| {
        let total: usize = rs_blocks(version).iter().map(|(_, data)| data).sum();
        data_bits(&bytes, version).length <= total * 8
    })?;
    let gf = Gf::new();
    let data = create_data(&gf, &bytes, version);
    let mut best = (0u8, 0.0f64);
    for pattern in 0..8u8 {
        let lost = lost_point(&make(version, &data, true, pattern));
        if pattern == 0 || best.1 > lost {
            best = (pattern, lost);
        }
    }
    let grid = make(version, &data, false, best.0);
    Some(QrCode { size: grid.size, dark: grid.cells.iter().map(|cell| *cell == Some(true)).collect() })
}

#[cfg(test)]
#[path = "qr_tests.rs"]
mod tests;
