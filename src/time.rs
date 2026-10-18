//! Calendar arithmetic with no UI or platform dependencies.

/// Days-since-epoch to (year, month, day) — Howard Hinnant's `civil_from_days`.
pub fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719468;
    let era = if z >= 0 { z } else { z - 146096 } / 146097;
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    (y, m as u32, d)
}

#[cfg(test)]
mod tests {
    use super::civil_from_days;

    /// The date math behind the Imports list: known dates.
    #[test]
    fn civil_days_matches_known_dates() {
        assert_eq!(civil_from_days(0), (1970, 1, 1));
        assert_eq!(civil_from_days(19_468), (2023, 4, 21));
        assert_eq!(civil_from_days(19_782), (2024, 2, 29)); // leap day
    }
}
