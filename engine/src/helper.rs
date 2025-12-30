pub fn usize_into_u32(value: usize) -> u32 {
    value
        .try_into()
        .expect("failed converting usize into u32")
}