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

//		Modules

#[cfg(test)]
#[path = "tests/server.rs"]
mod tests;



//		Packages

use axum::{
	Extension,
	Json,
	extract::Path,
	http::StatusCode,
	response::IntoResponse,
};
use core::fmt::{Display, self};
use hex;
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
		Json(json!({ "version": core.latest_version() }))
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
			Some(hash) => Ok(Json(json!({ "version": version, "hash": hex::encode(hash) }))),
			None       => Err((StatusCode::NOT_FOUND, format!("Version {version} not found"))),
		}
	}
}


