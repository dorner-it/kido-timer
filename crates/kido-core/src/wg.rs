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

use chrono::NaiveDate;
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
        todo!()
    }

    /// Long German label, e.g. `"Jungen + Mädchen 8–10"`, `"Mädchen bis 14"`.
    pub fn label_de(self) -> &'static str {
        todo!()
    }

    /// Inclusive age range (31.12.-age). `min` is `None` where the rules
    /// define no lower bound. WG1 = (Some(8), 10); WG2/3 = (None, 14);
    /// WG4/5 = (None, 18).
    pub fn age_range(self) -> (Option<i32>, i32) {
        todo!()
    }
}

/// Age on 31.12. of `competition_year` = `competition_year - birth_year`.
pub fn age_on_dec31(birth_date: NaiveDate, competition_year: i32) -> i32 {
    todo!()
}

/// Completed years on `day`, matching Excel `DATEDIF(birth; day; "Y")`
/// from Kenny's Tagessieger sheet: the birthday itself counts as
/// completed; a 29 Feb birthday completes on 1 Mar in non-leap years.
pub fn age_on_day(birth_date: NaiveDate, day: NaiveDate) -> i32 {
    todo!()
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
    todo!()
}
