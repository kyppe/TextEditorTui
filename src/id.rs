use rand::Rng;

/// Generates a random 6-character lowercase hex ID, e.g. "a83f2c".
///
/// Uniqueness against existing entries is enforced by the caller
/// (see `Store::unique_id`), so this function alone does not guarantee
/// global uniqueness.
pub fn generate() -> String {
    let mut rng = rand::thread_rng();
    let n: u32 = rng.gen_range(0..0x0100_0000);
    format!("{:06x}", n)
}
