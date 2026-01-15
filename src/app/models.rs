use anyhow::Result;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Question {
    pub origin: String,
    pub stem: String,
    #[serde(default)]
    pub origin_from_our_bank: Vec<String>,
    pub is_title: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub imgs: Option<Vec<String>>,
    pub screenshot: String, //我要存为base64字符串
}

impl Default for Question {
    fn default() -> Self {
        Self {
            origin: String::new(),
            stem: String::new(),
            origin_from_our_bank: Vec::new(),
            is_title: false,
            imgs: None,
            screenshot: String::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Paper {
    pub name: String,
    pub province: String,
    pub grade: String,
    #[serde(deserialize_with = "deserialize_year")]
    pub year: String,
    pub subject: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub page_id: Option<String>,
    pub stemlist: Vec<Question>,
    #[serde(skip)]
    pub name_for_pdf: String,

    pub url: String,
    #[serde(default)]
    pub is_exit: Option<bool>,
}

impl Paper {
    pub fn set_paper_id(&mut self, page_id: String) {
        self.page_id = Some(page_id);
    }
}

fn deserialize_year<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: serde::Deserializer<'de>,
{
    use serde::de::Visitor;
    use std::fmt;

    struct YearVisitor;

    impl<'de> Visitor<'de> for YearVisitor {
        type Value = String;

        fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
            formatter.write_str("a string or integer representing a year")
        }

        fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
        where
            E: serde::de::Error,
        {
            Ok(value.to_string())
        }

        fn visit_i64<E>(self, value: i64) -> Result<Self::Value, E>
        where
            E: serde::de::Error,
        {
            Ok(value.to_string())
        }

        fn visit_u64<E>(self, value: u64) -> Result<Self::Value, E>
        where
            E: serde::de::Error,
        {
            Ok(value.to_string())
        }
    }

    deserializer.deserialize_any(YearVisitor)
}
