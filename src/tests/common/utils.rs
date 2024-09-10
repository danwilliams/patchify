//! Common shared utility functionality for tests and examples.

//		Packages

use rand::rngs::OsRng;
use rubedo::crypto::SigningKey;



//		Functions

//		generate_new_private_key												
/// Generate a new private key for use in signing.
pub fn generate_new_private_key() -> SigningKey {
	SigningKey::generate(&mut OsRng)
}


