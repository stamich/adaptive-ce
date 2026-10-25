//! Record corpus (ACE 0.8.0): files that are tables of fixed-size records.
//!
//! The corpus has two jobs. The columnar tests and the `storage` benchmark family use it to
//! measure what a schema buys, and `layout` states the schema of each workload so that a
//! consumer needs no copy of it. It is not part of [`crate::Workload`]: the golden files and the
//! `corpus` families keep their historical set, and a record workload is only meaningful together
//! with its layout.

use crate::XorShift64;

/// What a field of a record holds (mirrors `ace_core::ColumnKind`; this crate has no
/// dependencies).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FieldKind {
    /// Opaque bytes.
    Bytes,
    /// Unsigned little-endian integer.
    Unsigned,
    /// Signed little-endian integer.
    Signed,
    /// IEEE-754 little-endian float.
    Float,
}

/// A deterministic table of fixed-size records.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecordWorkload {
    /// 24-byte trades: `time u64 | price f64 | quantity u32 | flags [u8; 4]`.
    Trades,
    /// 16-byte sensor readings: `device u16 | status u8 | battery u8 | time u32 | temp f32 | seq u32`.
    Telemetry,
    /// 32-byte events: `user u64 | kind u8 | time u64 | payload [u8; 15]`.
    Events,
}

/// Inherent methods of [`RecordWorkload`].
impl RecordWorkload {
    /// Every record workload, in a fixed order.
    pub const ALL: [Self; 3] = [Self::Trades, Self::Telemetry, Self::Events];

    /// Stable kebab-case identifier used in reports.
    pub const fn name(self) -> &'static str {
        match self {
            Self::Trades => "records-trades",
            Self::Telemetry => "records-telemetry",
            Self::Events => "records-events",
        }
    }

    /// The workload with this [`name`](Self::name), if there is one.
    pub fn from_name(name: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|workload| workload.name() == name)
    }

    /// The fields of one record as `(width in bytes, kind)`, in record order.
    pub const fn layout(self) -> &'static [(u16, FieldKind)] {
        match self {
            Self::Trades => &[
                (8, FieldKind::Unsigned),
                (8, FieldKind::Float),
                (4, FieldKind::Unsigned),
                (4, FieldKind::Bytes),
            ],
            Self::Telemetry => &[
                (2, FieldKind::Unsigned),
                (1, FieldKind::Unsigned),
                (1, FieldKind::Unsigned),
                (4, FieldKind::Unsigned),
                (4, FieldKind::Float),
                (4, FieldKind::Unsigned),
            ],
            Self::Events => &[
                (8, FieldKind::Unsigned),
                (1, FieldKind::Unsigned),
                (8, FieldKind::Unsigned),
                (15, FieldKind::Bytes),
            ],
        }
    }

    /// Bytes of one record: the sum of the layout widths.
    pub fn record_size(self) -> usize {
        self.layout()
            .iter()
            .map(|&(width, _)| usize::from(width))
            .sum()
    }

    /// Generates `records` whole records. The same workload and count always give the same
    /// bytes on every platform.
    pub fn generate_records(self, records: usize) -> Vec<u8> {
        let mut out = Vec::with_capacity(records.saturating_mul(self.record_size()));
        match self {
            Self::Trades => trades(&mut out, records),
            Self::Telemetry => telemetry(&mut out, records),
            Self::Events => events(&mut out, records),
        }
        out
    }

    /// Generates about `bytes` bytes: as many whole records as fit.
    pub fn generate(self, bytes: usize) -> Vec<u8> {
        self.generate_records(bytes / self.record_size())
    }
}

/// Decimal price with two digits walking by cents.
fn trades(out: &mut Vec<u8>, records: usize) {
    let mut rng = XorShift64::new(0x7261_6465_7331);
    let mut cents = 10_000i64;
    for i in 0..records {
        let draw = rng.next_u64();
        cents += (draw % 5) as i64 - 2;
        out.extend_from_slice(
            &(1_700_000_000_000u64 + i as u64 * 250 + (draw >> 60)).to_le_bytes(),
        );
        out.extend_from_slice(&(cents as f64 / 100.0).to_le_bytes());
        out.extend_from_slice(&((1 + (draw >> 8) % 20) as u32 * 100).to_le_bytes());
        out.extend_from_slice(&[b'A' + (i % 3) as u8, 0, 0, u8::from(draw & 1 == 0)]);
    }
}

/// A fleet of 64 devices reporting in round-robin; temperature quantised to 0.1.
fn telemetry(out: &mut Vec<u8>, records: usize) {
    let mut rng = XorShift64::new(0x7465_6c65_6d65);
    for i in 0..records {
        let draw = rng.next_u64();
        let device = (i % 64) as u16;
        let battery = 100u8.saturating_sub((i / 4096 % 100) as u8);
        let temperature = 20.0f32 + ((i / 64) % 40) as f32 * 0.1 + (draw % 3) as f32 * 0.1;
        out.extend_from_slice(&device.to_le_bytes());
        out.push(u8::from(draw.is_multiple_of(97)) * 2);
        out.push(battery);
        out.extend_from_slice(&(1_700_000_000u32 + (i / 64) as u32 * 10).to_le_bytes());
        out.extend_from_slice(&((temperature * 10.0).round() / 10.0).to_le_bytes());
        out.extend_from_slice(&(i as u32).to_le_bytes());
    }
}

/// Users drawn from a skewed distribution; a short templated payload.
fn events(out: &mut Vec<u8>, records: usize) {
    let mut rng = XorShift64::new(0x6576_656e_7473);
    let templates: [&[u8; 15]; 4] = [
        b"page.view/home\0",
        b"page.view/cart\0",
        b"click.buy/item\0",
        b"session.start\0\0",
    ];
    let mut time = 1_700_000_000_000u64;
    for _ in 0..records {
        let draw = rng.next_u64();
        // Skew: the square of a uniform value concentrates ids near zero.
        let uniform = (draw >> 40) % 1000;
        let user = 1_000_000 + uniform * uniform / 100;
        time += draw % 40;
        out.extend_from_slice(&user.to_le_bytes());
        out.push((draw % 4) as u8);
        out.extend_from_slice(&time.to_le_bytes());
        out.extend_from_slice(templates[(draw >> 20) as usize % templates.len()]);
    }
}
