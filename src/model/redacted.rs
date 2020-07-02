use serde::{Deserialize, Serialize, Serializer};
use shrinkwraprs::Shrinkwrap;
use std::{fmt, iter};
use unicode_width::UnicodeWidthStr;

#[derive(
    Shrinkwrap, Deserialize, sqlx::Type, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Default,
)]
#[sqlx(transparent)]
pub struct Redacted<T>(T);

impl<T> Redacted<T> {
    pub fn _new(value: T) -> Self {
        Self(value)
    }
}

impl<T> Serialize for Redacted<T> {
    fn serialize<S: Serializer>(&self, ser: S) -> Result<S::Ok, S::Error> {
        ser.serialize_none()
    }
}

impl<T: fmt::Debug> fmt::Debug for Redacted<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}",
            iter::repeat("█")
                .take(UnicodeWidthStr::width(format!("{:?}", self.0).as_str()))
                .collect::<String>()
        )
    }
}

impl<T: fmt::Display> fmt::Display for Redacted<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}",
            iter::repeat("█")
                .take(UnicodeWidthStr::width(format!("{}", self.0).as_str()))
                .collect::<String>()
        )
    }
}
