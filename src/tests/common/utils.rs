//		Packages

use ed25519_dalek::SigningKey;
use rand::rngs::OsRng;



//		Functions

//		generate_new_private_key												
pub(crate) fn generate_new_private_key() -> SigningKey {
	SigningKey::generate(&mut OsRng::default())
}


