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
//! # Streaming
//! 
//! The behaviour implemented in the provided [`Axum`] handlers is that large
//! release files will be streamed. For more information on how to configure the
//! various streaming parameters, see the [`Config`] struct documentation.
//! 



//		Modules

#[cfg(test)]
#[path = "tests/server.rs"]
mod tests;



//		Packages

use crate::responses::{LatestVersionResponse, VersionHashResponse};
use axum::{
	Extension,
	Json,
	body::{Body, Bytes},
	extract::Path,
	http::{StatusCode, header::CONTENT_LENGTH, header::CONTENT_TYPE},
	response::{IntoResponse, Response},
};
use ed25519_dalek::Signer as _;
use rubedo::{
	crypto::{Sha256Hash, SigningKey},
	http::ResponseExt as _,
	std::FileExt as _,
	sugar::s,
};
use semver::Version;
use std::{
	collections::HashMap,
	fs::File,
	io::ErrorKind as IoErrorKind,
	path::PathBuf,
	sync::Arc,
};
use thiserror::Error as ThisError;
use tokio::{
	fs::File as AsyncFile,
	io::{AsyncReadExt as _, BufReader},
};
use tokio_util::io::ReaderStream;
use tracing::error;



//		Enums

//		ReleaseError															
/// Errors that can occur in relation to releases.
#[derive(Clone, Debug, Eq, PartialEq, ThisError)]
#[non_exhaustive]
pub enum ReleaseError {
	/// A release file failed the SHA256 hash check.
	#[error("The release file for version {0} failed hash verification: {1:?}")]
	Invalid(Version, PathBuf),
	
	/// A release file does not exist.
	#[error("The release file for version {0} is missing: {1:?}")]
	Missing(Version, PathBuf),
	
	/// A release file is unreadable.
	#[error("The release file for version {0} cannot be read: {1}: {2}")]
	Unreadable(Version, IoErrorKind, String),
}



//		Structs

//		Config																	
/// The configuration options for the server.
/// 
/// Most configuration options should be fairly self-explanatory, according to
/// their individual documentation. However, there are some areas that merit
/// further commentary, given in the following sections.
/// 
/// # Release file naming conventions
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
/// # Release file streaming
/// 
/// If the release files are larger than a (configurable) size they will be
/// streamed to the client, rather than read into memory all at once. This is to
/// ensure that the server can handle large files without running out of memory.
/// 
/// Note that the sizes of the stream buffer and read buffer are hugely
/// important to performance, with smaller buffers greatly impacting download
/// speeds. The recommended default values have been carefully chosen based on
/// extensive testing, and should not generally need to be changed. However, on
/// a system with lots of users and very few large files it *may* be worth
/// decreasing the buffer sizes to reduce memory usage when those files are
/// requested, and on a system with very few users and lots of large files it
/// *may* be worth increasing the buffer sizes to improve throughput. However,
/// the chosen values are already within 5-10% of the very best possible speeds,
/// so any increase should be made with caution. It is more likely that they
/// would need to be decreased a little on a very busy system with a lot of
/// large files, where the memory usage could become a problem and the raw speed
/// of each download becomes a secondary concern.
/// 
#[expect(clippy::exhaustive_structs, reason = "Provided for configuration")]
#[derive(Clone, Debug)]
pub struct Config {
	//		Public properties													
	/// The name of the application. This is used to match against the files in
	/// the [`releases`](Self::releases) directory, to ensure that the correct
	/// files are served.
	pub appname:          String,
	
	/// The private key for the server. This is used to sign the HTTP responses
	/// to ensure that they have not been tampered with. The format used is
	/// Ed25519, which is a modern and secure algorithm.
	pub key:              SigningKey,
	
	/// The path to the directory containing the binary release files. This
	/// should follow a flat structure, with the files named according to the
	/// [`appname`](Self::appname) and [version number](Self::versions).
	pub releases:         PathBuf,
	
	/// The file size at which to start streaming, in KB. Below this size, the
	/// file will be read into memory and served all at once. A sensible default
	/// is `1000` (1MB).
	pub stream_threshold: u64,
	
	/// The size of the stream buffer to use when streaming files, in KB. A
	/// sensible default is `256` (256KB).
	pub stream_buffer:    usize,
	
	/// The size of the read buffer to use when streaming files, in KB. A
	/// sensible default is `128` (128KB).
	pub read_buffer:      usize,
	
	/// The available versions of the application. This is a map of [SemVer](https://semver.org/)
	/// version numbers against the SHA256 hashes of the binary release files.
	/// The hashes are required so that the server can verify the integrity of
	/// the files before serving them to clients.
	pub versions:         HashMap<Version, Sha256Hash>
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
		#[expect(clippy::iter_over_hash_type, reason = "Order doesn't matter here")]
		for (version, hash) in &config.versions {
			let path = config.releases.join(format!("{}-{}", config.appname, version));
			if !path.exists() || !path.is_file() {
				return Err(ReleaseError::Missing(version.clone(), path));
			}
			let file_hash: Sha256Hash = File::hash(&path).map_err(|err|
				ReleaseError::Unreadable(version.clone(), err.kind(), err.to_string())
			)?;
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
	pub fn versions(&self) -> HashMap<Version, Sha256Hash> {
		self.config.versions.clone()
	}
	
	//		release_file														
	/// The release file for a given version of the application.
	/// 
	/// This function returns the path to the release file for the specified
	/// version of the application, as per the configured version list.
	/// 
	/// No attempt will be made to verify the existence, readability, or
	/// integrity of the release file.
	/// 
	/// If the specified version does not exist, this function will return
	/// `None`.
	/// 
	/// # Parameters
	/// 
	/// * `version` - The version of the application to retrieve the release
	///               file for.
	/// 
	#[must_use]
	pub fn release_file(&self, version: &Version) -> Option<PathBuf> {
		self.versions()
			.get(version)
			.map(|_hash| self.config.releases.join(format!("{}-{}", self.config.appname, version)))
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
///     .route("/api/latest",            get(Axum::get_latest_version))
///     .route("/api/hashes/:version",   get(Axum::get_hash_for_version))
///     .route("/api/releases/:version", get(Axum::get_release_file))
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
	#[expect(clippy::unused_async, reason = "Consistent and future-proof")]
	pub async fn get_latest_version(
		Extension(core): Extension<Arc<Core>>,
	) -> impl IntoResponse {
		Self::sign_response(&core.config.key, Json(LatestVersionResponse {
			version: core.latest_version(),
		}).into_response())
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
	#[expect(clippy::unused_async, reason = "Consistent and future-proof")]
	pub async fn get_hash_for_version(
		Extension(core): Extension<Arc<Core>>,
		Path(version):   Path<Version>,
	) -> impl IntoResponse {
		match core.versions().get(&version) {
			Some(hash) => Ok(Self::sign_response(&core.config.key, Json(VersionHashResponse {
				version,
				hash:    *hash,
			}).into_response())),
			None       => Err((StatusCode::NOT_FOUND, format!("Version {version} not found"))),
		}
	}
	
	//		get_release_file													
	/// Release file for a given version of the application.
	/// 
	/// This function returns the release file for the specified version of the
	/// application, as per the configured version list. It will stream the file
	/// if it is large.
	/// 
	/// # Parameters
	/// 
	/// * `core`    - The core server instance.
	/// * `version` - The version of the application to retrieve the release
	///               file for.
	/// 
	/// # Errors
	/// 
	///   - A `400 Bad Request` status will be returned if the version format is
	///     invalid.
	///   - A `404 Not Found` status will be returned if the specified version
	///     does not exist.
	///   - A `500 Internal Server Error` status will be returned if the file
	///     is missing or cannot be read. In this situation a message to this
	///     effect will be provided — this is useful for testing the endpoint
	///     directly, but in a production environment it would be sensible to
	///     strip it out rather than show it to an end user.
	/// 
	#[expect(clippy::missing_panics_doc, reason = "Infallible")]
	pub async fn get_release_file(
		Extension(core): Extension<Arc<Core>>,
		Path(version):   Path<Version>,
	) -> impl IntoResponse {
		let Some(path) = core.release_file(&version) else {
			return Err((StatusCode::NOT_FOUND, format!("Version {version} not found")));
		};
		if !path.exists() || !path.is_file() {
			error!("Release file missing: {path:?}");
			return Err((StatusCode::INTERNAL_SERVER_ERROR, s!("Release file missing")));
		}
		let mut file  = match AsyncFile::open(&path).await {
			Ok(file) => file,
			Err(err) => {
				error!("Cannot open release file: {path:?}, error: {err}");
				return Err((StatusCode::INTERNAL_SERVER_ERROR, s!("Cannot open release file")));
			},
		};
		let metadata = match file.metadata().await {
			Ok(metadata) => metadata,
			Err(err)     => {
				error!("Cannot read release file metadata: {path:?}, error: {err}");
				return Err((StatusCode::INTERNAL_SERVER_ERROR, s!("Cannot read release file metadata")));
			},
		};
		let body = if metadata.len() > core.config.stream_threshold.saturating_mul(1024) {
			let reader = BufReader::with_capacity(core.config.read_buffer.saturating_mul(1024), file);
			let stream = ReaderStream::with_capacity(reader, core.config.stream_buffer.saturating_mul(1024));
			Body::from_stream(stream)
		} else {
			let mut contents = vec![];
			match file.read_to_end(&mut contents).await {
				Ok(_)    => (),
				Err(err) => {
					error!("Cannot read release file: {path:?}, error: {err}");
					return Err((StatusCode::INTERNAL_SERVER_ERROR, s!("Cannot read release file")));
				},
			}
			Body::from(contents)
		};
		#[expect(clippy::unwrap_used, reason = "Infallible")]
		Ok(Response::builder()
			.status(StatusCode::OK)
			.header(CONTENT_TYPE,   "application/octet-stream")
			.header(CONTENT_LENGTH, metadata.len())
			.body(body)
			.unwrap()
		)
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
	#[expect(clippy::missing_panics_doc, reason = "Infallible")]
	#[expect(clippy::unwrap_used,        reason = "Infallible")]
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


