const SERVICE: &str = "net.hardscrabble.oceans-ink";
const ACCOUNT: &str = "instapaper-access-token";

fn entry() -> Result<keyring::Entry, keyring::Error> {
    keyring::Entry::new(SERVICE, ACCOUNT)
}

pub fn get_token() -> Option<String> {
    entry().ok()?.get_password().ok()
}

pub fn save_token(token: &str) -> Result<(), keyring::Error> {
    entry()?.set_password(token)
}

#[allow(dead_code)]
pub fn delete_token() -> Result<(), keyring::Error> {
    entry()?.delete_credential()
}
