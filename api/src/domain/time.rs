//! Formats de date de l'API : ISO 8601 en UTC, millisecondes, suffixe `Z`
//! (identique à `Date.prototype.toISOString`).

use chrono::{DateTime, SecondsFormat, Utc};
use serde::Serializer;

pub fn iso(datetime: &DateTime<Utc>) -> String {
    datetime.to_rfc3339_opts(SecondsFormat::Millis, true)
}

pub fn iso_now() -> String {
    iso(&Utc::now())
}

pub fn serialize_iso<S: Serializer>(datetime: &DateTime<Utc>, serializer: S) -> Result<S::Ok, S::Error> {
    serializer.serialize_str(&iso(datetime))
}

pub fn serialize_iso_opt<S: Serializer>(datetime: &Option<DateTime<Utc>>, serializer: S) -> Result<S::Ok, S::Error> {
    match datetime {
        Some(value) => serializer.serialize_str(&iso(value)),
        None => serializer.serialize_none(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    #[test]
    fn format_to_iso_string() {
        let date = Utc.with_ymd_and_hms(2026, 7, 2, 9, 5, 3).unwrap();
        assert_eq!(iso(&date), "2026-07-02T09:05:03.000Z");
    }
}
