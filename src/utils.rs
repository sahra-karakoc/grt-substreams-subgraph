// -------------------- KEY GENERATORS --------------------
pub fn generate_key(account: &Vec<u8>) -> String {
    return Hex(account).to_string();
}

