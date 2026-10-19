// SPDX-License-Identifier: LicenseRef-Proprietary-DornerIT

//! `Wertungsgruppe` (WG) — five fixed classes for the Gruppenstafette.
//!
//! The WG is **ticked by the club** on registration and stored on the
//! `TeamEntry` as input — it is never derived. The system only *warns*
//! when a runner's 31.12.-age does not fit the chosen WG
//! ([`check_roster`]); saving is never blocked. There is no gender
//! logic anymore.
//!
//! Types and rules live in the shared `kido-core` crate; this module
//! re-exports them so existing call sites keep compiling.

pub use kido_core::wg::{Wertungsgruppe, WgWarning, age_on_dec31, check_roster};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wire_values() {
        for (wg, key) in [
            (Wertungsgruppe::Wg1, "wg1"),
            (Wertungsgruppe::Wg2, "wg2"),
            (Wertungsgruppe::Wg3, "wg3"),
            (Wertungsgruppe::Wg4, "wg4"),
            (Wertungsgruppe::Wg5, "wg5"),
        ] {
            assert_eq!(serde_json::to_value(wg).unwrap(), serde_json::json!(key));
            let back: Wertungsgruppe = serde_json::from_value(serde_json::json!(key)).unwrap();
            assert_eq!(back, wg);
        }
        assert!(serde_json::from_str::<Wertungsgruppe>("\"wg6\"").is_err());
    }
}
