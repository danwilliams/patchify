//! This module provides server-side functionality to add to an API.
//! 
//! It is designed with flexibility and ease of use in mind. The primary
//! functionality is provided in the [`Core`] struct, which can be used directly
//! from endpoint handlers or similar. In addition, an [`Axum`] struct is also
//! provided, which contains ready-made handlers for use with the [Axum](https://crates.io/crates/axum)
//! web framework. These handlers call the methods on the [`Core`] struct, and
//! provide a convenient way to add the functionality to an existing Axum
//! application.
//! 
//! # Signing and verification
//! 
//! There is support for signing HTTP responses, to ensure that they have not
//! been tampered with, and to allow connecting clients to verify authenticity.
//! This is done using the server's private key, and is designed to be used with
//! responses that contain a fully-known body (i.e. not streams), as the
//! complete body data needs to be used to generate the signature.
//! 
//! The key format used is Ed25519, which is a modern and secure algorithm, more
//! secure and performant than RSA. Technically it's part of the family of
//! Edwards-curve Digital Signature Algorithm (`EdDSA`), and uses Curve25519 as
//! its underlying elliptic curve. It is designed for high performance, offering
//! fast signature generation and verification, significantly quicker than
//! traditional RSA signatures. The keys and signatures are also small, which is
//! beneficial for storage and transmission. RSA keys typically need to be at
//! least 2048 bits (and increasingly 3072 or 4096 bits for long-term security),
//! whereas Ed25519 keys are only 256 bits, yet deliver higher security.
//! 
//! Given its advantages, Ed25519 is often recommended for new cryptographic
//! applications where digital signatures are required. It's particularly suited
//! for scenarios where performance and security are critical, such as secure
//! communications, authentication, and blockchain technologies.
//! 
//! The design of this library is such that the [`Core`] functionality does not
//! implement signing, as it is not directly involved with creating HTTP
//! responses, but the [`Axum`] handlers do. The approach chosen is to return
//! the signature as an `X-Signature` header, rather than embedding it in the
//! response body payload. This is to keep the response body payload clean and
//! free from additional data, and to allow the signature to be verified
//! separately from the response body. The pattern used by this library is that
//! release file downloads are not signed, allowing them to be streamed if they
//! are large, with a SHA256 hash being available separately for verification.
//! The response containing the hash is signed, so the hash can be verified as
//! authentic.
//! 
//! Due to the short length of the Ed25519 keys and signatures, they are sent in
//! hexadecimal string format, instead of using base64. This is to ensure
//! maximum compatibility with all potential uses. Base64 would only offer a
//! minor saving in comparison.
//! 

//		Modules

#[cfg(test)]
#[path = "tests/server.rs"]
mod tests;



//		Packages

use axum::{
	Extension,
	Json,
	body::{Body, Bytes},
	extract::Path,
	http::StatusCode,
	response::{IntoResponse, Response},
};
use core::fmt::{Display, self};
use ed25519_dalek::{Signer, SigningKey};
use hex;
use rubedo::http::ResponseExt;
use semver::Version;
use serde_json::json;
use sha2::{Sha256, Digest};
use std::{
	collections::HashMap,
	error::Error,
	fs::File,
	io::{ErrorKind as IoErrorKind, Read},
	path::PathBuf,
	sync::Arc,
};



//		Enums

//		ReleaseError															
/// Errors that can occur in relation to releases.
#[derive(Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum ReleaseError {
	/// A release file failed the SHA256 hash check.
	Invalid(Version, PathBuf),
	
	/// A release file does not exist.
	Missing(Version, PathBuf),
	
	/// A release file is unreadable.
	Unreadable(Version, IoErrorKind, String),
}

//󰭅		Display																	
impl Display for ReleaseError {
	//		fmt																	
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		write!(f, "{}", match *self {
			Self::Invalid(ref version, ref path)            => format!("The release file for version {version} failed hash verification: {path:?}"),
			Self::Missing(ref version, ref path)            => format!("The release file for version {version} is missing: {path:?}"),
			Self::Unreadable(ref version, ref err, ref msg) => format!("The release file for version {version} cannot be read: {err}: {msg}"),
		})
	}
}

//󰭅		Error																	
impl Error for ReleaseError {}



//		Structs

//		Config																	
/// The configuration options for the server.
/// 
/// Notably, the filename format for the binary release files is expected to be
/// `appname-version`, where `appname` is the name of the application, and
/// `version` is the version number. This is used to match against the files in
/// the [`releases`](Self::releases) directory, to ensure that the correct files
/// are served. The version number is expected to be in the format `x.y.z`,
/// where `x`, `y`, and `z` are the major, minor, and patch version numbers
/// respectively, following the [Semantic Versioning](https://semver.org/)
/// specification.
/// 
/// At present, file extensions are not supported, as initial support is for
/// Linux only. The release files are expected to be straightforward binaries,
/// with no additional packaging or compression. Additionally, only one
/// architecture is supported at present, which is undetermined and up to the
/// implementer to decide. All releases are expected to be stable, and there is
/// no way to specify a release as a beta.
/// 
#[cfg_attr(    feature = "reasons",  allow(clippy::exhaustive_structs, reason = "Provided for configuration"))]
#[cfg_attr(not(feature = "reasons"), allow(clippy::exhaustive_structs))]
#[derive(Clone, Debug)]
pub struct Config {
	//		Public properties													
	/// The name of the application. This is used to match against the files in
	/// the [`releases`](Self::releases) directory, to ensure that the correct
	/// files are served.
	pub appname:  String,
	
	/// The private key for the server. This is used to sign the HTTP responses
	/// to ensure that they have not been tampered with. The format used is
	/// Ed25519, which is a modern and secure algorithm.
	pub key:      SigningKey,
	
	/// The path to the directory containing the binary release files. This
	/// should follow a flat structure, with the files named according to the
	/// [`appname`](Self::appname) and [version number](Self::versions).
	pub releases: PathBuf,
	
	/// The available versions of the application. This is a map of [SemVer](https://semver.org/)
	/// version numbers against the SHA256 hashes of the binary release files.
	/// The hashes are required so that the server can verify the integrity of
	/// the files before serving them to clients.
	pub versions: HashMap<Version, [u8; 32]>
}

//		Core																	
/// The core functionality of the server.
/// 
/// This struct provides the core functionality of the server, and is designed
/// to be used directly from endpoint handlers or similar. It is also used by
/// the [`Axum`] struct, which contains ready-made handlers for use with the
/// [Axum](https://crates.io/crates/axum) web framework.
/// 
#[derive(Clone, Debug)]
pub struct Core {
	//		Private properties													
	/// The configuration for the server.
	config: Config,
	
	/// The latest version of the application. This is determined by examining
	/// the version list supplied, and finding the highest number. It is then
	/// cached here for efficiency.
	latest: Version,
}

//󰭅		Core																	
impl Core {
	//		new																	
	/// Creates a new core server instance.
	/// 
	/// This function creates a new core server instance, with the specified
	/// configuration.
	/// 
	/// Note that if the supplied version list is empty, the latest version will
	/// be set to `0.0.0`.
	/// 
	/// This function will check the release files for the versions specified in
	/// the list, and will return an error if any of the files are missing,
	/// unreadable, or fail the SHA256 hash check.
	/// 
	/// # Parameters
	/// 
	/// * `config` - The configuration for the server.
	/// 
	/// # Errors
	/// 
	/// * [`ReleaseError::Invalid`]
	/// * [`ReleaseError::Missing`]
	/// * [`ReleaseError::Unreadable`]
	/// 
	pub fn new(config: Config) -> Result<Self, ReleaseError> {
		for (version, hash) in &config.versions {
			let path = config.releases.join(&format!("{}-{}", config.appname, version));
			if !path.exists() || !path.is_file() {
				return Err(ReleaseError::Missing(version.clone(), path));
			}
			let mut file   = File::open(&path).map_err(|err|
				ReleaseError::Unreadable(version.clone(), err.kind(), err.to_string())
			)?;
			let mut hasher = Sha256::new();
			let mut buffer = vec![0; 0x0010_0000].into_boxed_slice();  //  1M read buffer on the heap
			loop {
				let count = file.read(&mut buffer).map_err(|err|
					ReleaseError::Unreadable(version.clone(), err.kind(), err.to_string())
				)?;
				if count == 0 {
					break;
				}
				#[cfg_attr(    feature = "reasons",  allow(clippy::indexing_slicing, reason = "Infallible"))]
				#[cfg_attr(not(feature = "reasons"), allow(clippy::indexing_slicing))]
				hasher.update(&buffer[..count]);
			}
			let file_hash: [u8; 32] = hasher.finalize().into();
			if file_hash != *hash {
				return Err(ReleaseError::Invalid(version.clone(), path));
			}
		}
		let latest = config.versions.keys().max().unwrap_or(&Version::new(0, 0, 0)).clone();
		Ok(Self {
			config,
			latest,
		})
	}
	
	//		latest_version														
	/// The latest version of the application.
	/// 
	/// This function returns the latest version of the application, as per the
	/// configured version list.
	/// 
	#[must_use]
	pub fn latest_version(&self) -> Version {
		self.latest.clone()
	}
	
	//		versions															
	/// The available versions of the application.
	/// 
	/// This function returns the available versions of the application, as
	/// specified in the configuration.
	/// 
	#[must_use]
	pub fn versions(&self) -> HashMap<Version, [u8; 32]> {
		self.config.versions.clone()
	}
}

//		Axum																	
/// Endpoint handlers for use with the Axum web framework.
/// 
/// This struct contains ready-made handlers for use with the [Axum](https://crates.io/crates/axum)
/// web framework. These handlers call the methods on the [`Core`] struct, and
/// provide a convenient way to add the functionality to an existing Axum-based
/// application API.
/// 
/// Note the following:
/// 
///   1. The methods on the [`Core`] struct require [`Core`] to be instantiated.
///      The instance should be wrapped in an [`Arc`] and added to the Axum
///      router as an extension, to be extracted by the handlers.
///   2. The handlers are static methods, and stateless, and obtain their state
///      by extracting the [`Core`] instance from the request extensions.
/// 
/// It is not intended that this struct should be instantiated, and so the
/// ability to do so is not provided.
/// 
/// # Examples
/// 
/// ```ignore
/// let config = Config { /* ... */ };
/// let core   = Arc::new(Core::new(config));
/// let app    = Router::new()
///     .route("/api/latest",          get(Axum::get_latest_version))
///     .route("/api/hashes/:version", get(Axum::get_hash_for_version))
///     .layer(Extension(core))
/// ;
/// ```
/// 
#[derive(Copy, Clone, Debug)]
#[non_exhaustive]
pub struct Axum;

//󰭅		Axum																	
impl Axum {
	//		get_latest_version													
	/// Latest version number of the application.
	/// 
	/// This handler returns a response containing the latest version number of
	/// the application, as per the configured version list.
	/// 
	/// It does not include the SHA256 hash, to keep the response size to a
	/// minimum.
	/// 
	/// # Parameters
	/// 
	/// * `core`    - The core server instance.
	/// 
	#[cfg_attr(    feature = "reasons",  allow(clippy::unused_async, reason = "Consistent and future-proof"))]
	#[cfg_attr(not(feature = "reasons"), allow(clippy::unused_async))]
	pub async fn get_latest_version(
		Extension(core): Extension<Arc<Core>>,
	) -> impl IntoResponse {
		Self::sign_response(&core.config.key, Json(json!({ "version": core.latest_version() })).into_response())
	}
	
	//		get_hash_for_version												
	/// SHA256 hash for a given version of the application.
	/// 
	/// This function checks the configured version list and returns the
	/// matching SHA256 hash for the specified version of the application.
	/// 
	/// # Parameters
	/// 
	/// * `core`    - The core server instance.
	/// * `version` - The version of the application to retrieve the hash for.
	/// 
	/// # Errors
	/// 
	///   - A `400 Bad Request` status will be returned if the version format is
	///     invalid.
	///   - A `404 Not Found` status will be returned if the specified version
	///     does not exist.
	/// 
	#[cfg_attr(    feature = "reasons",  allow(clippy::unused_async, reason = "Consistent and future-proof"))]
	#[cfg_attr(not(feature = "reasons"), allow(clippy::unused_async))]
	pub async fn get_hash_for_version(
		Extension(core): Extension<Arc<Core>>,
		Path(version):   Path<Version>,
	) -> impl IntoResponse {
		#[cfg_attr(    feature = "reasons",  allow(clippy::option_if_let_else, reason = "Match is more readable here"))]
		#[cfg_attr(not(feature = "reasons"), allow(clippy::option_if_let_else))]
		match core.versions().get(&version) {
			Some(hash) => Ok(Self::sign_response(&core.config.key, Json(json!({
				"version": version,
				"hash":    hex::encode(hash),
			})).into_response())),
			None       => Err((StatusCode::NOT_FOUND, format!("Version {version} not found"))),
		}
	}
	
	//		sign_response														
	/// Signs a response by adding a signature header.
	/// 
	/// This function accepts a [`Response`] and signs it by adding an
	/// `X-Signature` header. The signature is generated against the response
	/// body using the server's private key.
	/// 
	/// Note that this function is only suitable for use with responses that
	/// contain a fully-known body, as the complete body data needs to be used
	/// to generate the signature. It is therefore not suitable for use with
	/// streaming responses, as the entire body must be known in advance in
	/// order to be signed. As large files are often streamed, the implication
	/// is that these should be unsigned, with their authenticity verified by
	/// other means.
	/// 
	/// The pattern used by this library is that release file downloads are not
	/// signed, allowing them to be streamed if they are large, with a SHA256
	/// hash being available separately for verification. The response
	/// containing the hash is signed, so the hash can be verified as authentic.
	/// 
	/// # Parameters
	/// 
	/// * `key`      - The server's private key.
	/// * `response` - The [`Response`] to sign.
	/// 
	#[cfg_attr(    feature = "reasons",  allow(clippy::missing_panics_doc, reason = "Infallible"))]
	#[cfg_attr(not(feature = "reasons"), allow(clippy::missing_panics_doc))]
	#[cfg_attr(    feature = "reasons",  allow(clippy::unwrap_used, reason = "Infallible"))]
	#[cfg_attr(not(feature = "reasons"), allow(clippy::unwrap_used))]
	#[must_use]
	pub fn sign_response(key: &SigningKey, mut response: Response) -> Response {
		let unpacked_response   = response.unpack().unwrap();
		let mut signed_response = Response::builder()
			.status(unpacked_response.status)
			.header("X-Signature", key.sign(unpacked_response.body.as_ref()).to_string())
			.body(Body::from(Bytes::from(unpacked_response.body.into_bytes())))
			.unwrap()
		;
		signed_response.headers_mut().extend(response.headers().clone());
		signed_response.into_response()
	}
}


