extern crate serde;

use MouseButton;
use self::serde::ser::{Serialize, Serializer, Error};
use self::serde::de::{self, Deserialize, Deserializer, Visitor};
use std::fmt;

impl Serialize for MouseButton {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let (variant, index) = match *self {
            MouseButton::Left => ("left", 0),
            MouseButton::Middle => ("middle", 1),
            MouseButton::Right => ("right", 2),
            _ => return Err(S::Error::custom("Not a valid MouseButton type.")),
        };
        serializer.serialize_unit_variant("MouseButton", index, variant)
    }
}
impl<'de> Deserialize<'de> for MouseButton {
    fn deserialize<D>(deserializer: D) -> Result<MouseButton, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct MouseVisitor;

        impl<'de> Visitor<'de> for MouseVisitor {
            type Value = MouseButton;

            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                f.write_str("a valid MouseButton type")
            }

            fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                match v {
                    "left" => Ok(MouseButton::Left),
                    "middle" => Ok(MouseButton::Middle),
                    "right" => Ok(MouseButton::Right),
                    _ => Err(E::custom("Not a valid MouseButton type.")),
                }
            }
        }

        deserializer.deserialize_identifier(MouseVisitor)
    }
}

#[cfg(feature = "serde_test")]
#[cfg(test)]
mod test {
    extern crate serde_test;
    use self::serde_test::{Token, assert_tokens};
    use MouseButton;

    #[test]
    fn test() {
        assert_tokens(
            &MouseButton::Left,
            &[
                Token::UnitVariant {
                    name: "MouseButton",
                    variant: "left",
                },
            ],
        );
    }
}
