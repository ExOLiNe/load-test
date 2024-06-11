use std::convert::TryFrom;
use std::fmt::{Display, Formatter};

#[derive(Debug)]
pub struct HttpHeader {
    pub name: String,
    pub value: String
}

impl Display for HttpHeader {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str(format!("{}: {}", self.name, self.value).as_str())
    }
}

impl TryFrom<String> for HttpHeader {
    type Error = crate::error::Error;
    fn try_from(value: String) -> Result<Self, Self::Error> {
        let parts: Vec<&str> = value.splitn(2, ": ").collect();
        let name = *parts.first().ok_or(Self::Error::HeaderParse("Parsing error"))?;
        let value = *parts.get(1).ok_or(Self::Error::HeaderParse("Parsing error"))?;
        let value = &value[..value.len() - 2];
        Ok(HttpHeader {
            name: String::from(name),
            value: String::from(value)
        })
    }
}