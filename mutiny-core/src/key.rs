use bitcoin::{
    secp256k1::Secp256k1,
    util::bip32::{ChildNumber, DerivationPath, ExtendedPrivKey},
};

use crate::error::MutinyError;

pub(crate) enum ChildKey {
    NodeChildKey,
    FederationChildKey,
}

impl ChildKey {
    pub(crate) fn to_child_number(&self) -> u32 {
        match self {
            ChildKey::NodeChildKey => 0,
            ChildKey::FederationChildKey => 1,
        }
    }
}

pub(crate) fn create_root_child_key(
    context: &Secp256k1<bitcoin::secp256k1::All>,
    xprivkey: ExtendedPrivKey,
    child_key: ChildKey,
) -> Result<ExtendedPrivKey, MutinyError> {
    let child_number = ChildNumber::from_hardened_idx(child_key.to_child_number())?;

    Ok(xprivkey.derive_priv(context, &DerivationPath::from(vec![child_number]))?)
}

#[cfg(test)]
fn run_key_generation_tests() {
    use bip39::Mnemonic;
    use bitcoin::Network;
    use std::str::FromStr;

    let context = Secp256k1::new();
    let mnemonic = Mnemonic::from_str("abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about").expect("could not generate");
    let xpriv = ExtendedPrivKey::new_master(Network::Testnet, &mnemonic.to_seed("")).unwrap();

    let first_root_key = create_root_child_key(&context, xpriv, ChildKey::NodeChildKey);
    let copy_root_key = create_root_child_key(&context, xpriv, ChildKey::NodeChildKey);
    assert_eq!(first_root_key, copy_root_key);

    let federation_root_key = create_root_child_key(&context, xpriv, ChildKey::FederationChildKey);
    assert_ne!(first_root_key, federation_root_key);
}

#[cfg(test)]
#[cfg(not(target_arch = "wasm32"))]
mod tests {
    use crate::key::run_key_generation_tests;

    #[test]
    fn key_generation_tests() {
        run_key_generation_tests();
    }
}

#[cfg(test)]
#[cfg(target_arch = "wasm32")]
mod wasm_tests {
    use wasm_bindgen_test::{wasm_bindgen_test as test, wasm_bindgen_test_configure};

    use crate::key::run_key_generation_tests;

    wasm_bindgen_test_configure!(run_in_browser);

    #[test]
    fn key_generation_tests() {
        run_key_generation_tests();
    }
}
