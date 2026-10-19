// SPDX-License-Identifier: LicenseRef-Proprietary-DornerIT

//! Wertungsgruppen and age rules.
//!
//! The WG is **ticked by the club** on registration — never derived. The
//! system only warns when a runner's age does not fit the chosen WG
//! (saving is never blocked). There is no gender field.
//!
//! Two different ages exist (Mirko, Q5):
//! - WG/participation age = age on 31.12. of the competition year
//!   (= competition year − birth year), see [`age_on_dec31`].
//! - Tagessieger age = completed years on the competition day, see
//!   [`age_on_day`].

use chrono::{Datelike, NaiveDate};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Wertungsgruppe {
    /// Jungen + Mädchen 8–10 (gemischt).
    Wg1,
    /// Jungen bis 14.
    Wg2,
    /// Mädchen bis 14.
    Wg3,
    /// Jungen bis 18.
    Wg4,
    /// Mädchen bis 18.
    Wg5,
}

impl Wertungsgruppe {
    pub const ALL: [Wertungsgruppe; 5] = [
        Wertungsgruppe::Wg1,
        Wertungsgruppe::Wg2,
        Wertungsgruppe::Wg3,
        Wertungsgruppe::Wg4,
        Wertungsgruppe::Wg5,
    ];

    /// `"WG1"` … `"WG5"`.
    pub fn label(self) -> &'static str {
        match self {
            Wertungsgruppe::Wg1 => "WG1",
            Wertungsgruppe::Wg2 => "WG2",
            Wertungsgruppe::Wg3 => "WG3",
            Wertungsgruppe::Wg4 => "WG4",
            Wertungsgruppe::Wg5 => "WG5",
        }
    }

    /// Long German label, e.g. `"Jungen + Mädchen 8–10"`, `"Mädchen bis 14"`.
    pub fn label_de(self) -> &'static str {
        match self {
            Wertungsgruppe::Wg1 => "Jungen + Mädchen 8–10",
            Wertungsgruppe::Wg2 => "Jungen bis 14",
            Wertungsgruppe::Wg3 => "Mädchen bis 14",
            Wertungsgruppe::Wg4 => "Jungen bis 18",
            Wertungsgruppe::Wg5 => "Mädchen bis 18",
        }
    }

    /// Inclusive age range (31.12.-age). `min` is `None` where the rules
    /// define no lower bound. WG1 = (Some(8), 10); WG2/3 = (None, 14);
    /// WG4/5 = (None, 18).
    pub fn age_range(self) -> (Option<i32>, i32) {
        match self {
            Wertungsgruppe::Wg1 => (Some(8), 10),
            Wertungsgruppe::Wg2 | Wertungsgruppe::Wg3 => (None, 14),
            Wertungsgruppe::Wg4 | Wertungsgruppe::Wg5 => (None, 18),
        }
    }
}

/// Age on 31.12. of `competition_year` = `competition_year - birth_year`.
pub fn age_on_dec31(birth_date: NaiveDate, competition_year: i32) -> i32 {
    competition_year - birth_date.year()
}

/// Completed years on `day`, matching Excel `DATEDIF(birth; day; "Y")`
/// from Kenny's Tagessieger sheet: the birthday itself counts as
/// completed; a 29 Feb birthday completes on 1 Mar in non-leap years.
///
/// If `day` lies before `birth_date` the result is negative (Excel would
/// return `#NUM!`); callers should not pass such data.
pub fn age_on_day(birth_date: NaiveDate, day: NaiveDate) -> i32 {
    let years = day.year() - birth_date.year();
    if (day.month(), day.day()) < (birth_date.month(), birth_date.day()) {
        years - 1
    } else {
        years
    }
}

/// A runner whose 31.12.-age does not fit the team's WG.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WgWarning {
    pub runner_id: Uuid,
    pub age: i32,
    pub wg: Wertungsgruppe,
    /// `"zu alt"` / `"zu jung"`.
    pub reason: String,
}

/// Check every runner of a roster against the chosen WG. Returns one
/// warning per runner outside the range; empty = all fine. Never errors.
pub fn check_roster(
    wg: Wertungsgruppe,
    runners: &[(Uuid, NaiveDate)],
    competition_year: i32,
) -> Vec<WgWarning> {
    let (min, max) = wg.age_range();
    runners
        .iter()
        .filter_map(|&(runner_id, birth_date)| {
            let age = age_on_dec31(birth_date, competition_year);
            let reason = if age > max {
                "zu alt"
            } else if min.is_some_and(|min| age < min) {
                "zu jung"
            } else {
                return None;
            };
            Some(WgWarning {
                runner_id,
                age,
                wg,
                reason: reason.to_string(),
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d(y: i32, m: u32, day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, day).unwrap()
    }

    fn id(n: u128) -> Uuid {
        Uuid::from_u128(n)
    }

    #[test]
    fn wire_values() {
        for (wg, s) in Wertungsgruppe::ALL
            .iter()
            .zip(["wg1", "wg2", "wg3", "wg4", "wg5"])
        {
            assert_eq!(serde_json::to_string(wg).unwrap(), format!("\"{s}\""));
            let back: Wertungsgruppe = serde_json::from_str(&format!("\"{s}\"")).unwrap();
            assert_eq!(back, *wg);
        }
        assert!(serde_json::from_str::<Wertungsgruppe>("\"WG1\"").is_err());
        assert!(serde_json::from_str::<Wertungsgruppe>("\"wg6\"").is_err());
    }

    #[test]
    fn labels() {
        let short: Vec<&str> = Wertungsgruppe::ALL.iter().map(|w| w.label()).collect();
        assert_eq!(short, ["WG1", "WG2", "WG3", "WG4", "WG5"]);
        assert_eq!(Wertungsgruppe::Wg1.label_de(), "Jungen + Mädchen 8–10");
        assert_eq!(Wertungsgruppe::Wg2.label_de(), "Jungen bis 14");
        assert_eq!(Wertungsgruppe::Wg3.label_de(), "Mädchen bis 14");
        assert_eq!(Wertungsgruppe::Wg4.label_de(), "Jungen bis 18");
        assert_eq!(Wertungsgruppe::Wg5.label_de(), "Mädchen bis 18");
    }

    #[test]
    fn age_ranges() {
        assert_eq!(Wertungsgruppe::Wg1.age_range(), (Some(8), 10));
        assert_eq!(Wertungsgruppe::Wg2.age_range(), (None, 14));
        assert_eq!(Wertungsgruppe::Wg3.age_range(), (None, 14));
        assert_eq!(Wertungsgruppe::Wg4.age_range(), (None, 18));
        assert_eq!(Wertungsgruppe::Wg5.age_range(), (None, 18));
    }

    #[test]
    fn ordering_wg1_first() {
        let mut v = vec![
            Wertungsgruppe::Wg5,
            Wertungsgruppe::Wg1,
            Wertungsgruppe::Wg3,
        ];
        v.sort();
        assert_eq!(
            v,
            [
                Wertungsgruppe::Wg1,
                Wertungsgruppe::Wg3,
                Wertungsgruppe::Wg5
            ]
        );
    }

    #[test]
    fn age_on_dec31_is_year_difference() {
        assert_eq!(age_on_dec31(d(2017, 1, 1), 2027), 10);
        assert_eq!(age_on_dec31(d(2017, 12, 31), 2027), 10);
        assert_eq!(age_on_dec31(d(2012, 6, 15), 2027), 15);
    }

    #[test]
    fn age_on_day_birthday_semantics() {
        let day = d(2027, 6, 12);
        assert_eq!(age_on_day(d(2015, 6, 12), day), 12); // birthday on the day
        assert_eq!(age_on_day(d(2015, 6, 13), day), 11); // birthday tomorrow
        assert_eq!(age_on_day(d(2015, 6, 11), day), 12); // birthday yesterday
        assert_eq!(age_on_day(d(2015, 7, 1), day), 11);
        assert_eq!(age_on_day(d(2015, 1, 1), day), 12);
        assert_eq!(age_on_day(d(2015, 12, 31), day), 11);
        // day before the birthday
        assert_eq!(age_on_day(d(2015, 6, 12), d(2027, 6, 11)), 11);
    }

    #[test]
    fn age_on_day_leap_birthday() {
        let b = d(2012, 2, 29);
        // 2027 is not a leap year: 29 Feb completes on 1 Mar (DATEDIF).
        assert_eq!(age_on_day(b, d(2027, 2, 28)), 14);
        assert_eq!(age_on_day(b, d(2027, 3, 1)), 15);
        // leap year: completes on 29 Feb itself.
        assert_eq!(age_on_day(b, d(2028, 2, 28)), 15);
        assert_eq!(age_on_day(b, d(2028, 2, 29)), 16);
    }

    #[test]
    fn age_on_day_vs_dec31() {
        // Both ages differ before the birthday.
        let b = d(2015, 9, 1);
        assert_eq!(age_on_dec31(b, 2027), 12);
        assert_eq!(age_on_day(b, d(2027, 6, 12)), 11);
    }

    #[test]
    fn check_roster_wg1() {
        let runners = [
            (id(1), d(2020, 1, 1)),   // 7 → zu jung
            (id(2), d(2019, 12, 31)), // 8 ok
            (id(3), d(2017, 1, 1)),   // 10 ok
            (id(4), d(2016, 12, 31)), // 11 → zu alt
        ];
        let w = check_roster(Wertungsgruppe::Wg1, &runners, 2027);
        assert_eq!(
            w,
            vec![
                WgWarning {
                    runner_id: id(1),
                    age: 7,
                    wg: Wertungsgruppe::Wg1,
                    reason: "zu jung".into()
                },
                WgWarning {
                    runner_id: id(4),
                    age: 11,
                    wg: Wertungsgruppe::Wg1,
                    reason: "zu alt".into()
                },
            ]
        );
    }

    #[test]
    fn check_roster_upper_bounds_no_lower_bound() {
        for wg in [Wertungsgruppe::Wg2, Wertungsgruppe::Wg3] {
            let ok = [(id(1), d(2013, 1, 1)), (id(2), d(2021, 1, 1))]; // 14, 6
            assert!(check_roster(wg, &ok, 2027).is_empty());
            let w = check_roster(wg, &[(id(3), d(2012, 12, 31))], 2027); // 15
            assert_eq!(w.len(), 1);
            assert_eq!(
                (w[0].age, w[0].reason.as_str(), w[0].wg),
                (15, "zu alt", wg)
            );
        }
        for wg in [Wertungsgruppe::Wg4, Wertungsgruppe::Wg5] {
            let ok = [(id(1), d(2009, 1, 1)), (id(2), d(2021, 1, 1))]; // 18, 6
            assert!(check_roster(wg, &ok, 2027).is_empty());
            let w = check_roster(wg, &[(id(3), d(2008, 12, 31))], 2027); // 19
            assert_eq!(w.len(), 1);
            assert_eq!(
                (w[0].age, w[0].reason.as_str(), w[0].wg),
                (19, "zu alt", wg)
            );
        }
    }

    #[test]
    fn check_roster_empty_and_multiple() {
        assert!(check_roster(Wertungsgruppe::Wg1, &[], 2027).is_empty());
        let runners = [
            (id(1), d(2010, 1, 1)), // 17
            (id(2), d(2013, 1, 1)), // 14 ok
            (id(3), d(2011, 1, 1)), // 16
            (id(4), d(2005, 1, 1)), // 22
        ];
        let w = check_roster(Wertungsgruppe::Wg2, &runners, 2027);
        let ids: Vec<Uuid> = w.iter().map(|w| w.runner_id).collect();
        assert_eq!(ids, [id(1), id(3), id(4)]);
        assert!(w.iter().all(|w| w.reason == "zu alt"));
    }

    #[test]
    fn warning_serializes_snake_case() {
        let w = WgWarning {
            runner_id: id(1),
            age: 15,
            wg: Wertungsgruppe::Wg2,
            reason: "zu alt".into(),
        };
        let v: serde_json::Value = serde_json::to_value(&w).unwrap();
        assert_eq!(v["wg"], "wg2");
        assert_eq!(v["age"], 15);
        assert_eq!(v["reason"], "zu alt");
        assert_eq!(v["runner_id"], id(1).to_string());
    }
}
