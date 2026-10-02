//! Distances on the Earth's surface, through [`crate::dmath`] so they are
//! bit-identical on every machine.
//!
//! The Earth is a sphere of the IUGG mean radius (R₁ = 6371.0088 km). Over
//! the distances people move, the error against the WGS84 ellipsoid is
//! under 0.5%, far below what residence models resolve.

use crate::dmath;

/// IUGG mean Earth radius R₁, in kilometres.
pub const EARTH_RADIUS_KM: f64 = 6371.0088;

/// Kilometres per international mile.
pub const KM_PER_MILE: f64 = 1.609344;

/// A point by latitude and longitude, in degrees.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LatLon {
    pub lat: f64,
    pub lon: f64,
}

impl LatLon {
    #[inline]
    pub const fn new(lat: f64, lon: f64) -> Self {
        Self { lat, lon }
    }
}

/// Great-circle distance in kilometres (the haversine formula, stable at
/// short range). `asin`'s argument is clamped to 1 against rounding at
/// antipodes.
#[inline]
pub fn haversine_km(a: LatLon, b: LatLon) -> f64 {
    let (p1, p2) = (a.lat.to_radians(), b.lat.to_radians());
    let dp = p2 - p1;
    let dl = (b.lon - a.lon).to_radians();
    let s1 = dmath::sin(dp / 2.0);
    let s2 = dmath::sin(dl / 2.0);
    let h = s1 * s1 + dmath::cos(p1) * dmath::cos(p2) * s2 * s2;
    2.0 * EARTH_RADIUS_KM * dmath::asin(h.sqrt().min(1.0))
}

/// Great-circle distance in miles.
#[inline]
pub fn haversine_miles(a: LatLon, b: LatLon) -> f64 {
    haversine_km(a, b) / KM_PER_MILE
}

#[cfg(test)]
mod tests {
    use super::*;

    const NYC: LatLon = LatLon::new(40.7128, -74.0060);
    const LA: LatLon = LatLon::new(34.0522, -118.2437);

    #[test]
    fn known_distances() {
        // References from the same formula in f64 (Python's math module).
        assert!((haversine_km(NYC, LA) - 3935.751690893986).abs() < 1e-9);
        let london = LatLon::new(51.5, 0.0);
        let paris = LatLon::new(48.8566, 2.3522);
        assert!((haversine_km(london, paris) - 338.2657325805489).abs() < 1e-9);
        assert!((haversine_miles(NYC, LA) - 3935.751690893986 / 1.609344).abs() < 1e-9);
    }

    #[test]
    fn golden_bits() {
        assert_eq!(haversine_km(NYC, LA).to_bits(), 0x40AEBF80DDA0FCBF);
    }

    #[test]
    fn metric_laws() {
        let mut x = 0x9E37_79B9_7F4A_7C15u64;
        let mut next = || {
            x ^= x << 13;
            x ^= x >> 7;
            x ^= x << 17;
            x
        };
        let mut point = || {
            let lat = (next() % 1_800_000) as f64 / 10_000.0 - 90.0;
            let lon = (next() % 3_600_000) as f64 / 10_000.0 - 180.0;
            LatLon::new(lat, lon)
        };
        for _ in 0..2000 {
            let (a, b, c) = (point(), point(), point());
            assert_eq!(haversine_km(a, a), 0.0);
            assert_eq!(haversine_km(a, b).to_bits(), haversine_km(b, a).to_bits());
            let (ab, bc, ac) = (haversine_km(a, b), haversine_km(b, c), haversine_km(a, c));
            assert!(ac <= ab + bc + 1e-9, "triangle inequality");
            assert!(ab <= std::f64::consts::PI * EARTH_RADIUS_KM + 1e-9);
        }
        // Haversine loses precision at antipodes (asin near 1): about 1e-4 km.
        let antipode = haversine_km(LatLon::new(10.0, 20.0), LatLon::new(-10.0, -160.0));
        assert!((antipode - std::f64::consts::PI * EARTH_RADIUS_KM).abs() < 1e-3);
    }
}
