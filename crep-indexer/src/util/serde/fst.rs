pub mod fst_set_to_vec {
    use fst::Set;
    use serde::de::Error as DeError;
    use serde::{Deserialize, Deserializer, Serialize, Serializer};

    pub fn serialize<S>(
        set: &Set<Vec<u8>>,
        serializer: S,
    ) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let v = set.stream().into_bytes();
        v.serialize(serializer)
    }

    pub fn deserialize<'de, D>(
        deserializer: D,
    ) -> Result<Set<Vec<u8>>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let v = Vec::<Vec<u8>>::deserialize(deserializer)?;
        Set::<Vec<u8>>::from_iter(v.iter())
            .map_err(|e| D::Error::custom(format!("conversion error {e}")))
    }

    pub mod option {
        use super::*;

        pub fn serialize<S>(
            set: &Option<Set<Vec<u8>>>,
            serializer: S,
        ) -> Result<S::Ok, S::Error>
        where
            S: Serializer,
        {
            match set {
                Some(inner) => super::serialize(inner, serializer),
                None => serializer.serialize_none(),
            }
        }

        pub fn deserialize<'de, D>(
            deserializer: D,
        ) -> Result<Option<Set<Vec<u8>>>, D::Error>
        where
            D: Deserializer<'de>,
        {
            let v = Option::<Vec<Vec<u8>>>::deserialize(deserializer)?;
            match v {
                Some(inner) => Set::<Vec<u8>>::from_iter(inner.iter())
                    .map_err(|e| {
                        D::Error::custom(format!("conversion error {e}"))
                    })
                    .map(|s| Some(s)),
                None => Ok(None),
            }
        }
    }
}
