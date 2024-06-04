use std::convert::TryFrom;

#[derive(Debug)]
pub struct HttpHeader {
    pub name: String,
    pub value: String
}

impl TryFrom<String> for HttpHeader {
    type Error = ();
    fn try_from(value: String) -> Result<Self, Self::Error> {
        let parts: Vec<&str> = value.splitn(2, ": ").collect();
        let name = *parts.get(0).unwrap();
        let value = *parts.get(1).unwrap();
        let value = &value[..value.len() - 2];
        Ok(HttpHeader {
            // todo replace unwrap with ?
            name: String::from(name),
            value: String::from(value)
        })
    }
}