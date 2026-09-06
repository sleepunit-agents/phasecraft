//! Exact musical durations on the 960 PPQN clock; no floating-point accumulation.
use serde::{Deserialize, Deserializer, Serialize, Serializer};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NoteValue(pub u64);
impl Default for NoteValue {
    fn default() -> Self {
        Self(super::STEP_TICKS)
    }
}
impl NoteValue {
    pub fn is_default(&self) -> bool {
        *self == Self::default()
    }
    pub fn parse(s: &str) -> Result<Self, String> {
        let (base, num, den) = if let Some(s) = s.strip_suffix('T') {
            (s, 2, 3)
        } else if let Some(s) = s.strip_suffix('.') {
            (s, 3, 2)
        } else {
            (s, 1, 1)
        };
        let divisor: u64 = base.strip_prefix("1/").and_then(|s| s.parse().ok())
            .ok_or("note value must be 1/4, 1/8, 1/16 etc., optionally followed by T (triplet) or . (dotted)")?;
        if ![1, 2, 4, 8, 16, 32, 64].contains(&divisor) {
            return Err("note denominator must be 1, 2, 4, 8, 16, 32 or 64".into());
        }
        Ok(Self(super::PPQN * 4 * num / (divisor * den)))
    }
    pub fn valid(self) -> bool {
        [1, 2, 4, 8, 16, 32, 64].iter().any(|d| {
            ["", "T", "."]
                .iter()
                .any(|s| Self::parse(&format!("1/{d}{s}")) == Ok(self))
        })
    }
    pub fn label(self) -> String {
        for denominator in [1, 2, 4, 8, 16, 32, 64] {
            for suffix in ["", "T", "."] {
                let label = format!("1/{denominator}{suffix}");
                if Self::parse(&label) == Ok(self) {
                    return label;
                }
            }
        }
        unreachable!("validated note value")
    }
}
impl Serialize for NoteValue {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.label())
    }
}
impl<'de> Deserialize<'de> for NoteValue {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        Self::parse(&String::deserialize(deserializer)?).map_err(serde::de::Error::custom)
    }
}
