//! Serde pretty printer for `OsString`
//!
//! This module provides a custom serializer and deserializer for `OsString` that
//! pretty-prints the string where possible (i.e. when it's a valid UTF-8 string).
//!
//! This is useful for displaying `OsString` values in a more readable format,
//! especially when they contain non-ASCII characters or special characters.

use serde::{
    Deserialize, Deserializer, Serialize, Serializer,
    de::{self, MapAccess, SeqAccess, Visitor},
};
use std::{ffi::OsString, fmt};

pub fn serialize<S>(os_string: &OsString, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    match os_string.to_str() {
        Some(s) => s.serialize(serializer),
        // Use normal serialization (OsString::serialize) for invalid UTF-8 strings
        None => OsString::serialize(os_string, serializer),
    }
}

pub fn deserialize<'de, D>(deserializer: D) -> Result<OsString, D::Error>
where
    D: Deserializer<'de>,
{
    // Visitor implementation to handle different JSON value types
    struct OsStringVisitor;

    impl<'de> Visitor<'de> for OsStringVisitor {
        type Value = OsString; // The type we want to produce

        fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
            formatter.write_str("a UTF-8 string or the platform-specific OsString map/sequence")
        }

        // Handle the case where the JSON value is a String "..."
        fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            // If it's a string, simply convert it to OsString
            Ok(OsString::from(value))
        }

        // Also handle visit_string for completeness
        fn visit_string<E>(self, value: String) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            Ok(OsString::from(value))
        }

        // Handle the case where the JSON value is a Sequence [...] (e.g., bytes on Unix)
        fn visit_seq<A>(self, seq: A) -> Result<Self::Value, A::Error>
        where
            A: SeqAccess<'de>,
        {
            // Reconstruct a Deserializer from the SeqAccess to delegate
            // This calls OsString's default deserialize logic for sequences
            OsString::deserialize(de::value::SeqAccessDeserializer::new(seq))
        }

        // Handle the case where the JSON value is a Map { ... } (e.g., {"Windows": [...]} )
        fn visit_map<A>(self, map: A) -> Result<Self::Value, A::Error>
        where
            A: MapAccess<'de>,
        {
            // Reconstruct a Deserializer from the MapAccess to delegate
            // This calls OsString's default deserialize logic for maps
            OsString::deserialize(de::value::MapAccessDeserializer::new(map))
        }
    }

    // Tell the deserializer to drive our Visitor. It will call the appropriate
    // visit_* method based on the actual JSON data encountered.
    deserializer.deserialize_any(OsStringVisitor)
}

#[cfg(test)]
mod tests {
    use serde::{Deserialize, Serialize};
    use std::ffi::OsString;

    fn serialize_as_json(os_string: OsString) -> Result<String, anyhow::Error> {
        println!("os_string: {:?}", os_string);
        let mut output = Vec::new();
        super::serialize(&os_string, &mut serde_json::Serializer::new(&mut output))?;
        let output_string = String::from_utf8(output)?;
        Ok(output_string)
    }

    fn deserialize_from_json(json_string: &str) -> Result<OsString, anyhow::Error> {
        println!("json_string: {}", json_string);
        let mut deserializer = serde_json::Deserializer::from_str(json_string);
        let os_string = super::deserialize(&mut deserializer)?;
        Ok(os_string)
    }

    #[test]
    fn test_serialize() -> anyhow::Result<()> {
        assert_eq!(
            "\"Hello, world!\"",
            serialize_as_json(OsString::from("Hello, world!"))?
        );
        // Test with nonprintable characters
        assert_eq!(
            "\"Hello, \\u0001!\"",
            serialize_as_json(OsString::from("Hello, \u{0001}!"))?
        );
        // Test with larger unicode characters
        assert_eq!(
            "\"Hello, \u{1F600}!\"",
            serialize_as_json(OsString::from("Hello, \u{1F600}!"))?
        );
        Ok(())
    }

    #[test]
    pub fn test_in_struct() -> anyhow::Result<()> {
        #[derive(Serialize, Deserialize)]
        struct Test {
            #[serde(with = "super")]
            os_string: OsString,
        }
        assert_eq!(
            "{\"os_string\":\"Hello, world!\"}",
            serde_json::to_string(&Test {
                os_string: OsString::from("Hello, world!"),
            })?
        );
        assert_eq!(
            "{\"os_string\":\"Hello, \\u0001!\"}",
            serde_json::to_string(&Test {
                os_string: OsString::from("Hello, \u{0001}!"),
            })?
        );
        Ok(())
    }

    #[test]
    fn test_deserialize() -> anyhow::Result<()> {
        assert_eq!(
            OsString::from("Hello, world!"),
            deserialize_from_json("\"Hello, world!\"")?
        );
        assert_eq!(
            OsString::from("Hello, \u{0001}!"),
            deserialize_from_json("\"Hello, \\u0001!\"")?
        );
        assert_eq!(
            OsString::from("Hello, \n!"),
            deserialize_from_json("\"Hello, \\n!\"")?
        );
        assert_eq!(
            OsString::from("Hello, \u{1F600}!"),
            deserialize_from_json("\"Hello, \u{1F600}!\"")?
        );
        assert!(deserialize_from_json("null").is_err());
        assert!(deserialize_from_json("{}").is_err());
        assert!(deserialize_from_json("[]").is_err());
        Ok(())
    }

    #[cfg(windows)]
    mod windows {
        use std::ffi::OsString;
        use std::os::windows::ffi::OsStringExt;

        use super::{deserialize_from_json, serialize_as_json};

        #[test]
        fn test_serialize() -> anyhow::Result<()> {
            // Example: D:\<unpaired surrogate>path - UTF-16 code units: [D, :, \, 0xD800, p, a, t, h]
            let invalid_utf16_bytes: Vec<u16> = vec![
                0x0044, 0x003A, 0x005C, 0xD800, 0x0070, 0x0061, 0x0074, 0x0068,
            ];
            let invalid_os_string = OsString::from_wide(&invalid_utf16_bytes);
            assert_eq!(
                "{\"Windows\":[68,58,92,55296,112,97,116,104]}",
                serialize_as_json(invalid_os_string)?
            );

            // "\\?\PCI#VEN_10DE&DEV_1E84&SUBSYS_450919DA&REV_A1#4&1fc990d7&0&0019#{5b45201d-f2f2-4f3b-85bb-30ff1f953e97}"
            let device_path_os_string = OsString::from(
                "\\\\?\\PCI#VEN_10DE&DEV_1E84&SUBSYS_450919DA&REV_A1#4&1fc990d7&0&0019#{5b45201d-f2f2-4f3b-85bb-30ff1f953e97}",
            );
            assert_eq!(
                "\"\\\\\\\\?\\\\PCI#VEN_10DE&DEV_1E84&SUBSYS_450919DA&REV_A1#4&1fc990d7&0&0019#{5b45201d-f2f2-4f3b-85bb-30ff1f953e97}\"",
                serialize_as_json(device_path_os_string)?
            );

            Ok(())
        }

        #[test]
        fn test_deserialize() -> anyhow::Result<()> {
            let invalid_utf16_bytes: Vec<u16> = vec![
                0x0044, 0x003A, 0x005C, 0xD800, 0x0070, 0x0061, 0x0074, 0x0068,
            ];
            let invalid_os_string = OsString::from_wide(&invalid_utf16_bytes);
            assert_eq!(
                invalid_os_string,
                deserialize_from_json("{\"Windows\":[68,58,92,55296,112,97,116,104]}")?
            );

            let device_path_os_string = OsString::from(
                "\\\\?\\PCI#VEN_10DE&DEV_1E84&SUBSYS_450919DA&REV_A1#4&1fc990d7&0&0019#{5b45201d-f2f2-4f3b-85bb-30ff1f953e97}",
            );
            assert_eq!(
                device_path_os_string,
                deserialize_from_json(
                    "\"\\\\\\\\?\\\\PCI#VEN_10DE&DEV_1E84&SUBSYS_450919DA&REV_A1#4&1fc990d7&0&0019#{5b45201d-f2f2-4f3b-85bb-30ff1f953e97}\""
                )?
            );
            Ok(())
        }
    }

    // TODO: Actually test on Unix
    #[cfg(unix)]
    mod unix {
        use super::{deserialize_from_json, serialize_as_json};
        use std::ffi::OsString;
        use std::os::unix::ffi::OsStringExt;

        #[test]
        fn test_serialize() -> anyhow::Result<()> {
            let non_utf8_bytes = vec![0x5c, 0x66, 0x6f, 0x80, 0x6f]; // \fo<invalid>o
            let invalid_os_string = OsString::from_vec(non_utf8_bytes);
            assert_eq!(
                "{\"Unix\":[92,102,111,128,111]}",
                serialize_as_json(invalid_os_string)?
            );
            Ok(())
        }

        #[test]
        fn test_deserialize() -> anyhow::Result<()> {
            let non_utf8_bytes = vec![0x5c, 0x66, 0x6f, 0x80, 0x6f]; // \fo<invalid>o
            let invalid_os_string = OsString::from_vec(non_utf8_bytes);
            assert_eq!(
                invalid_os_string,
                deserialize_from_json("{\"Unix\":[92,102,111,128,111]}")?
            );
            Ok(())
        }
    }
}
