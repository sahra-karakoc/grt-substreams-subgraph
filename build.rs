use anyhow::{Ok, Result};
use substreams_ethereum::Abigen;


fn main() -> Result<(), anyhow::Error> {
    Abigen::new("erc20", "./abis/erc20.json")?
        .generate()?
        .write_to_file("src/abi/erc20.rs")?;
    Ok(())
}
