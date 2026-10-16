use keyring::Entry;

use super::types::PairedAccount;

const SERVICE: &str = "de.dorner-it.kido-timer";
const ACCOUNT: &str = "cloud-account";

fn entry() -> Result<Entry, String> {
    Entry::new(SERVICE, ACCOUNT).map_err(|e| format!("keychain init failed: {e}"))
}

pub fn save(account: &PairedAccount) -> Result<(), String> {
    let json = serde_json::to_string(account).map_err(|e| format!("serialize failed: {e}"))?;
    entry()?
        .set_password(&json)
        .map_err(|e| format!("keychain write failed: {e}"))
}

pub fn load() -> Result<Option<PairedAccount>, String> {
    match entry()?.get_password() {
        Ok(json) => {
            let account: PairedAccount =
                serde_json::from_str(&json).map_err(|e| format!("keychain parse failed: {e}"))?;
            Ok(Some(account))
        }
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(e) => Err(format!("keychain read failed: {e}")),
    }
}

pub fn clear() -> Result<(), String> {
    match entry()?.delete_credential() {
        Ok(()) => Ok(()),
        Err(keyring::Error::NoEntry) => Ok(()),
        Err(e) => Err(format!("keychain delete failed: {e}")),
    }
}
