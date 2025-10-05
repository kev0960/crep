#[cfg(test)]
pub mod test {
    use fst::Set;

    pub fn convert_fst_to_string_vec(set: &Set<Vec<u8>>) -> Vec<String> {
        set.stream()
            .into_bytes()
            .into_iter()
            .map(|v| str::from_utf8(v.as_slice()).unwrap().to_owned())
            .collect()
    }
}
