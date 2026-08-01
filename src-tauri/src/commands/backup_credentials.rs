use keyring::{Entry, Error};
use zeroize::Zeroizing;

const SERVICE: &str = "com.yunqi.watchhouse.encrypted-backup";
const ACCOUNT: &str = "automatic-backup-password";

fn entry() -> Result<Entry, String> {
    Entry::new(SERVICE, ACCOUNT)
        .map_err(|error| format!("secure password storage is unavailable: {error}"))
}

pub fn save(password: &str) -> Result<(), String> {
    entry()?
        .set_password(password)
        .map_err(|error| format!("could not save the automatic backup password: {error}"))
}

pub fn load() -> Result<Option<Zeroizing<String>>, String> {
    match entry()?.get_password() {
        Ok(password) => Ok(Some(Zeroizing::new(password))),
        Err(Error::NoEntry) => Ok(None),
        Err(error) => Err(format!(
            "could not read the automatic backup password: {error}"
        )),
    }
}

pub fn delete() -> Result<(), String> {
    match entry()?.delete_credential() {
        Ok(()) | Err(Error::NoEntry) => Ok(()),
        Err(error) => Err(format!(
            "could not remove the automatic backup password: {error}"
        )),
    }
}
