//		Packages

use rand::rngs::OsRng;
use rubedo::crypto::SigningKey;



//		Functions

//		generate_new_private_key												
pub(crate) fn generate_new_private_key() -> SigningKey {
	SigningKey::generate(&mut OsRng::default())
}


