// SPDX-License-Identifier: LicenseRef-Proprietary-DornerIT

//! `Wertungsgruppe` (WG) — five-way classification of a team-entry
//! roster, derived from the runners' birth-year and gender against the
//! event's `Stichtag` (the event date).
//!
//! Definitions (verbatim from competition rules):
//!
//! - **WG1:** Jungen + Mädchen 8-10 (gemischt)
//! - **WG2:** Jungen bis 14 (nur männlich)
//! - **WG3:** Mädchen bis 14 (nur weiblich)
//! - **WG4:** Jungen bis 18 (nur männlich)
//! - **WG5:** Mädchen bis 18 (nur weiblich)
//!
//! The WG is stamped on the `TeamEntry` at create/update time and
//! preserved across later roster edits to keep results comparable —
//! ranking integrity beats live recomputation.

use chrono::{Datelike, NaiveDate};
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::models::runner::{Gender, Runner};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Wertungsgruppe {
    /// Jungen + Mädchen 8-10 (gemischt).
    Wg1,
    /// Jungen bis 14 (nur männlich).
    Wg2,
    /// Mädchen bis 14 (nur weiblich).
    Wg3,
    /// Jungen bis 18 (nur männlich).
    Wg4,
    /// Mädchen bis 18 (nur weiblich).
    Wg5,
}

impl Wertungsgruppe {
    /// Short display label (`"WG1"` … `"WG5"`) for UI helpers and logs.
    pub fn label(self) -> &'static str {
        match self {
            Wertungsgruppe::Wg1 => "WG1",
            Wertungsgruppe::Wg2 => "WG2",
            Wertungsgruppe::Wg3 => "WG3",
            Wertungsgruppe::Wg4 => "WG4",
            Wertungsgruppe::Wg5 => "WG5",
        }
    }

    /// Derive the matching `Wertungsgruppe` from a roster on a given
    /// reference date (the event's `date`). See module docs for the
    /// rules; ages are computed as the simple year-difference, **not**
    /// adjusted for the birthday itself.
    pub fn derive(runners: &[Runner], stichtag: NaiveDate) -> Result<Self, WgError> {
        if runners.is_empty() {
            return Err(WgError::Inconsistent {
                reason: "Mannschaft ohne Läufer".into(),
            });
        }

        let mut max_age: i32 = i32::MIN;
        let mut any_male = false;
        let mut any_female = false;
        let mut missing_gender: usize = 0;

        for r in runners {
            let age = stichtag.year() - r.birth_date.year();
            if age > max_age {
                max_age = age;
            }
            match r.gender {
                Some(Gender::Male) => any_male = true,
                Some(Gender::Female) => any_female = true,
                None => missing_gender += 1,
            }
        }

        if missing_gender > 0 {
            return Err(WgError::Inconsistent {
                reason: format!(
                    "Geschlecht fehlt für {missing_gender} Läufer; Wertungsgruppe nicht ableitbar"
                ),
            });
        }

        if max_age <= 10 {
            // WG1 explicitly allows mixed-gender rosters.
            return Ok(Wertungsgruppe::Wg1);
        }

        if any_male && any_female {
            return Err(WgError::Inconsistent {
                reason: format!(
                    "gemischtes Geschlecht bei max. Alter {max_age} unzulässig (nur WG1 ist gemischt)"
                ),
            });
        }

        if max_age <= 14 {
            return Ok(if any_male {
                Wertungsgruppe::Wg2
            } else {
                Wertungsgruppe::Wg3
            });
        }

        if max_age <= 18 {
            return Ok(if any_male {
                Wertungsgruppe::Wg4
            } else {
                Wertungsgruppe::Wg5
            });
        }

        Err(WgError::Inconsistent {
            reason: format!("Läufer älter als 18 ({max_age}) — keine passende Wertungsgruppe"),
        })
    }
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum WgError {
    #[error("Wertungsgruppe nicht ableitbar: {reason}")]
    Inconsistent { reason: String },
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{TimeZone, Utc};
    use uuid::Uuid;

    fn mk_runner(birth_year: i32, gender: Option<Gender>) -> Runner {
        let now = Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap();
        Runner {
            id: Uuid::nil(),
            team_id: Uuid::nil(),
            event_id: Uuid::nil(),
            first_name: "X".into(),
            last_name: "Y".into(),
            birth_date: NaiveDate::from_ymd_opt(birth_year, 6, 15).unwrap(),
            position: 1,
            gender,
            created_at: now,
            updated_at: now,
        }
    }

    fn stichtag(year: i32) -> NaiveDate {
        NaiveDate::from_ymd_opt(year, 6, 1).unwrap()
    }

    #[test]
    fn wg1_mixed_under_10() {
        // ages 8 + 10 (mixed gender → still WG1 because <=10)
        let roster = vec![
            mk_runner(2018, Some(Gender::Female)),
            mk_runner(2016, Some(Gender::Male)),
        ];
        assert_eq!(
            Wertungsgruppe::derive(&roster, stichtag(2026)).unwrap(),
            Wertungsgruppe::Wg1,
        );
    }

    #[test]
    fn wg2_all_male_under_14() {
        // max age 14, all male → WG2
        let roster = vec![
            mk_runner(2012, Some(Gender::Male)),
            mk_runner(2014, Some(Gender::Male)),
        ];
        assert_eq!(
            Wertungsgruppe::derive(&roster, stichtag(2026)).unwrap(),
            Wertungsgruppe::Wg2,
        );
    }

    #[test]
    fn wg3_all_female_under_14() {
        // max age 13, all female → WG3
        let roster = vec![
            mk_runner(2013, Some(Gender::Female)),
            mk_runner(2014, Some(Gender::Female)),
        ];
        assert_eq!(
            Wertungsgruppe::derive(&roster, stichtag(2026)).unwrap(),
            Wertungsgruppe::Wg3,
        );
    }

    #[test]
    fn wg4_all_male_under_18() {
        // max age 18, all male → WG4
        let roster = vec![
            mk_runner(2008, Some(Gender::Male)),
            mk_runner(2010, Some(Gender::Male)),
        ];
        assert_eq!(
            Wertungsgruppe::derive(&roster, stichtag(2026)).unwrap(),
            Wertungsgruppe::Wg4,
        );
    }

    #[test]
    fn wg5_all_female_under_18() {
        // max age 16, all female → WG5
        let roster = vec![
            mk_runner(2010, Some(Gender::Female)),
            mk_runner(2012, Some(Gender::Female)),
        ];
        assert_eq!(
            Wertungsgruppe::derive(&roster, stichtag(2026)).unwrap(),
            Wertungsgruppe::Wg5,
        );
    }

    #[test]
    fn mixed_over_10_rejected() {
        // max age 14, mixed gender → Inconsistent
        let roster = vec![
            mk_runner(2014, Some(Gender::Male)),
            mk_runner(2014, Some(Gender::Female)),
        ];
        let err = Wertungsgruppe::derive(&roster, stichtag(2026)).unwrap_err();
        assert!(matches!(err, WgError::Inconsistent { .. }));
    }

    #[test]
    fn over_18_rejected() {
        // max age 22 → no WG
        let roster = vec![mk_runner(2004, Some(Gender::Male))];
        let err = Wertungsgruppe::derive(&roster, stichtag(2026)).unwrap_err();
        assert!(matches!(err, WgError::Inconsistent { .. }));
    }

    #[test]
    fn missing_gender_rejected() {
        // age 16, gender unknown → can't dispatch all-male vs all-female
        let roster = vec![
            mk_runner(2010, Some(Gender::Male)),
            mk_runner(2011, None),
        ];
        let err = Wertungsgruppe::derive(&roster, stichtag(2026)).unwrap_err();
        assert!(matches!(err, WgError::Inconsistent { .. }));
    }

    #[test]
    fn empty_roster_rejected() {
        let err = Wertungsgruppe::derive(&[], stichtag(2026)).unwrap_err();
        assert!(matches!(err, WgError::Inconsistent { .. }));
    }
}
