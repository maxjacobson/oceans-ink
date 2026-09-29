use std::collections::HashMap;

use gio::Cancellable;
use libsecret::{Schema, SchemaAttributeType, SchemaFlags};

const SCHEMA_NAME: &str = "net.hardscrabble.oceans-ink";
const APP_ATTRIBUTE_VALUE: &str = "oceans-ink";
const LABEL: &str = "Instapaper access token";

fn schema() -> Schema {
    let mut attribute_names_and_types = HashMap::new();
    attribute_names_and_types.insert("app", SchemaAttributeType::String);
    Schema::new(SCHEMA_NAME, SchemaFlags::NONE, attribute_names_and_types)
}

fn attributes() -> HashMap<&'static str, &'static str> {
    HashMap::from([("app", APP_ATTRIBUTE_VALUE)])
}

/// Look up the stored access token. Runs blocking, so call it off the main thread.
pub fn get_token() -> Option<String> {
    libsecret::password_lookup_sync(Some(&schema()), attributes(), Cancellable::NONE)
        .ok()
        .flatten()
        .map(|s| s.to_string())
}

/// Store (or replace) the access token. Runs blocking, so call it off the main thread.
pub fn save_token(token: &str) -> Result<(), glib::Error> {
    libsecret::password_store_sync(
        Some(&schema()),
        attributes(),
        Some(libsecret::COLLECTION_DEFAULT),
        LABEL,
        token,
        Cancellable::NONE,
    )
}

/// Remove the access token. Runs blocking, so call it off the main thread.
#[allow(dead_code)]
pub fn delete_token() -> Result<(), glib::Error> {
    libsecret::password_clear_sync(Some(&schema()), attributes(), Cancellable::NONE)
}
