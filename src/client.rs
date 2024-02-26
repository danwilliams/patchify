//! This module provides client-side functionality to add to an application.

//		Modules

#[cfg(test)]
#[path = "tests/client.rs"]
mod tests;



//		Packages

use core::{
	fmt::{Display, self},
	sync::atomic::{AtomicUsize, Ordering},
};
use ed25519_dalek::{Signature, VerifyingKey};
use flume::{Sender, self};
use futures_util::StreamExt;
use hex;
use reqwest::{
	StatusCode,
	Url,
	header::CONTENT_TYPE,
};
use rubedo::sugar::s;
use semver::Version;
use serde::{
	Deserialize,
	de::DeserializeOwned
};
use sha2::{Sha256, Digest};
use std::{
	error::Error,
	path::PathBuf,
	sync::Arc,
};
use tempfile::{tempdir, TempDir};
use tokio::{
	fs::File as AsyncFile,
	io::AsyncWriteExt,
	select,
	spawn,
	time::{Duration, interval},
};
use tracing::{error, info};

#[cfg(not(test))]
use reqwest::{Client, Response};
#[cfg(test)]
use crate::mocks::{Client as HttpClient, MockClient as Client, MockResponse as Response, RequestBuilder};



//		Enums

//		UpdaterError															
/// Errors that can occur when trying to update.
#[derive(Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum UpdaterError {
	/// Verification of the SHA256 hash of the downloaded file against the
	/// server's hash data failed.
	FailedHashVerification(Version),
	
	/// Verification of the HTTP response body against the signature header
	/// using the configured public key failed.
	FailedSignatureVerification(Url),
	
	/// An HTTP error occurred, i.e. the status code returned is not `200`. No
	/// other codes are expected, as this library only performs `GET` requests.
	HttpError(Url, StatusCode),
	
	/// The HTTP request to the API server failed.
	HttpRequestFailed(Url, String),
	
	/// The response from the API server could not be decoded. This could be due
	/// to malformed text that is not valid UTF-8 for endpoints that return text
	/// or JSON, or a truncated response.
	InvalidBody(Url),
	
	/// The response from the API server could not be parsed. This could be due
	/// to invalid JSON, or the JSON not matching the expected structure.
	InvalidPayload(Url),
	
	/// The signature header from the API server could not be decoded.
	InvalidSignature(Url, String),
	
	/// The URL specified to use to make an HTTP request is invalid. The API URL
	/// should be okay due to type validation, so something must have happened
	/// when adding a particular endpoint to it, as the outcome is invalid.
	InvalidUrl(Url, String),
	
	/// The HTTP response from the API server does not contain a signature
	/// header.
	MissingSignature(Url),
	
	/// A problem was encountered when trying to create a file for the download.
	UnableToCreateDownload(PathBuf, String),
	
	/// A problem was encountered when trying to create a temporary directory.
	UnableToCreateTempDir(String),
	
	/// A problem was encountered when trying to write to the download file.
	UnableToWriteToDownload(PathBuf, String),
	
	/// The content type of the response is not as expected.
	UnexpectedContentType(Url, String, String),
}

//󰭅		Display																	
impl Display for UpdaterError {
	//		fmt																	
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		write!(f, "{}", match *self {
			Self::FailedHashVerification(ref version)                     => format!(  "Failed hash verification for downloaded version {version}"),
			Self::FailedSignatureVerification(ref url)                    => format!(  "Failed signature verification for response from {url}"),
			Self::HttpError(ref url, ref status)                          => format!(  "HTTP status code {status} received when calling {url}"),
			Self::HttpRequestFailed(ref url, ref msg)                     => format!(  "HTTP request to {url} failed: {msg}"),
			Self::InvalidBody(ref url)                                    => format!(  "Invalid HTTP body received from {url}"),
			Self::InvalidPayload(ref url)                                 => format!(  "Invalid payload received from {url}"),
			Self::InvalidSignature(ref url, ref signature)                => format!(r#"Invalid signature header "{signature}" received from {url}"#),
			Self::InvalidUrl(ref base, ref endpoint)                      => format!(  "Invalid URL specified: {base} plus {endpoint}"),
			Self::MissingSignature(ref url)                               => format!(  "HTTP response from {url} does not contain a signature header"),
			Self::UnableToCreateDownload(ref path, ref msg)               => format!(r#"Unable to create download file "{path:?}": {msg}"#),
			Self::UnableToCreateTempDir(ref msg)                          => format!(  "Unable to create temporary directory: {msg}"),
			Self::UnableToWriteToDownload(ref path, ref msg)              => format!(r#"Unable to write to download file "{path:?}": {msg}"#),
			Self::UnexpectedContentType(ref url, ref value, ref expected) => format!(r#"HTTP response from {url} had unexpected content type: "{value}", expected: "{expected}""#),
		})
	}
}

//󰭅		Error																	
impl Error for UpdaterError {}



//		Structs

//		Config																	
/// The configuration options for the client.
#[cfg_attr(    feature = "reasons",  allow(clippy::exhaustive_structs, reason = "Provided for configuration"))]
#[cfg_attr(not(feature = "reasons"), allow(clippy::exhaustive_structs))]
#[derive(Clone, Debug)]
pub struct Config {
	//		Public properties													
	/// The current version of the application. This is reported to the server
	/// when making requests such as checking for updates.
	pub version:          Version,
	
	/// The URL of the API. This is used to make requests to the server, such as
	/// checking for updates. It needs to be an FQDN (Fully-Qualified Domain
	/// Name), and should include the protocol (e.g. `https://`), plus any base
	/// path (e.g. `/api`). For example, `https://api.example.com/api/v2`.
	pub api:              Url,
	
	/// The public key for the server. This is used to verify the HTTP responses
	/// from the server, to ensure that they have not been tampered with. The
	/// format used is Ed25519, which is a modern and secure algorithm.
	pub key:              VerifyingKey,
	
	/// Whether to check for updates on startup.
	pub check_on_startup: bool,
	
	/// How often to check for updates. This is optional.
	pub check_interval:   Option<Duration>,
}

//		Updater																	
/// A service to check for updates at intervals, and upgrade the application.
/// 
/// This struct provides a service that will query the API server at defined
/// intervals, to check for updates. If an update is found, it will be
/// downloaded and installed, and the application will be restarted.
/// 
#[derive(Debug)]
pub struct Updater {
	//		Private properties													
	/// A counter of critical actions that are currently active. This is used to
	/// prevent the updater from stopping the application while a critical
	/// action is in progress.
	actions:     AtomicUsize,
	
	/// The configuration for the updater service.
	config:      Config,
	
	/// The HTTP client instance that is used for communicating with the API
	/// server.
	http_client: Client,
	
	/// The updater queue that is used for communicating with the interval
	/// timer. This is the sender side only. A queue is used so that the timer
	/// can run in a separate thread, but be stopped when required.
	queue:       Sender<()>,
}

//󰭅		Updater																	
impl Updater {
	//		Constructors														
	
	//		new																	
	/// Creates a new updater service instance.
	/// 
	/// This function creates a new updater service instance, with the specified
	/// configuration. As soon as the service is created, it will start checking
	/// for updates.
	/// 
	/// In order to shut down nicely, [`Drop`] is implemented, and will send a
	/// message to the internal queue to stop the timer.
	/// 
	/// # Parameters
	/// 
	/// * `config` - The configuration for the updater service.
	/// 
	#[must_use]
	pub fn new(config: Config) -> Arc<Self> {
		let http_client        = Client::new();
		let (sender, receiver) = flume::unbounded();
		let updater            = Arc::new(Self {
			actions:     AtomicUsize::new(0),
			config,
			http_client,
			queue:       sender,
		});
		if updater.config.check_on_startup {
			let startup_updater = Arc::clone(&updater);
			drop(spawn(async move {
				startup_updater.check_for_updates().await;
			}));
		}
		if let Some(check_interval) = updater.config.check_interval {
			let mut timer      = interval(check_interval);
			let mut first_tick = true;
			let timer_updater  = Arc::clone(&updater);
			//	Event-handling loop
			drop(spawn(async move { loop { select!{
				//	Wait for timer to tick
				_ = timer.tick() => {
					if first_tick {
						first_tick = false;
						continue;
					}
					timer_updater.check_for_updates().await;
				}
				//	Wait for message from queue - this is a blocking call
				_ = receiver.recv_async() => {
					info!("Stopping updater");
					break;
				}
			}}}));
		}
		updater
	}
	
	//		Public methods														
	
	//		register_action														
	/// Registers a critical action.
	/// 
	/// This function increments the critical actions counter, preventing the
	/// application from being updated while the critical action is in progress.
	/// 
	/// It returns the *likely new value* of the counter, or [`None`] if the
	/// counter overflows. The new value is likely rather than guaranteed due to
	/// the effect of concurrent updates, and therefore is the value known and
	/// set at the time it was incremented, and may not be the value by the time
	/// the function returns.
	/// 
	pub fn register_action(&self) -> Option<usize> {
		let value = self.actions
			.fetch_update(Ordering::SeqCst, Ordering::SeqCst, |value| { value.checked_add(1) })
			.ok()?
		;
		Some(value.saturating_add(1))
	}
	
	//		deregister_action													
	/// Deregisters a critical action.
	/// 
	/// This function decrements the critical actions counter, allowing the
	/// application to be updated once the count reaches zero.
	/// 
	/// It returns the *likely new value* of the counter, or [`None`] if the
	/// counter underflows. The new value is likely rather than guaranteed due
	/// to the effect of concurrent updates, and therefore is the value known
	/// and set at the time it was incremented, and may not be the value by the
	/// time the function returns.
	/// 
	pub fn deregister_action(&self) -> Option<usize> {
		let value = self.actions
			.fetch_update(Ordering::SeqCst, Ordering::SeqCst, |value| { value.checked_sub(1) })
			.ok()?
		;
		Some(value.saturating_sub(1))
	}
	
	//		is_safe_to_update													
	/// Checks if it is safe to update.
	/// 
	/// This function checks the critical actions counter, to see if it is safe
	/// to update the application — i.e. if the counter is zero.
	/// 
	/// Note that at present this is a naive implementation that does not lock
	/// the counter, and so it is possible that the counter could change between
	/// the time of checking and the time of updating.
	/// 
	pub fn is_safe_to_update(&self) -> bool {
		self.actions.load(Ordering::SeqCst) == 0
	}
	
	//		Private methods														
	
	//		check_for_updates													
	/// Checks for updates.
	/// 
	/// This function checks for updates by querying the API server. If an
	/// update is found, it will be downloaded and installed, and the
	/// application will be restarted.
	/// 
	async fn check_for_updates(&self) {
		//		Get latest version												
		info!("Checking for updates");
		let (url, response) = match self.request("latest").await {
			Ok(data) => data,
			Err(err) => {
				error!("Error checking for updates: {err}");
				return;
			},
		};
		let version = match self.decode_and_verify::<LatestVersionResponse>(url, response).await {
			Ok(json) => json.version,
			Err(err) => {
				error!("Error checking for updates: {err}");
				return;
			},
		};
		//		Compare to current version										
		if version <= self.config.version {
			info!("The current version {} is the latest available", self.config.version);
			return;
		}
		info!("New version {} available", version);
		//		Download update file											
		let (_download_dir, _update_path, file_hash) = match self.download_update(&version).await {
			Ok(data) => data,
			Err(err) => {
				error!("Error downloading update file: {err}");
				return;
			},
		};
		if let Err(err) = self.verify_update(&version, &file_hash).await {
			error!("Error verifying update file: {err}");
			return;
		}
		info!("Update file verified");
		//		Install update													
		//	TODO: Install update
		//		Restart application												
		//	TODO: Restart application
	}
	
	//		download_update														
	/// Downloads an application update.
	/// 
	/// This function downloads an application update from the API server, in
	/// the form of an executable binary, and calculates the SHA256 hash of the
	/// downloaded file.
	/// 
	/// # Errors
	/// 
	/// * [`UpdaterError::UnableToCreateDownload`]
	/// * [`UpdaterError::UnableToCreateTempDir`]
	/// * [`UpdaterError::UnableToWriteToDownload`]
	/// * [`UpdaterError::UnexpectedContentType`]
	/// 
	async fn download_update(&self, version: &Version) -> Result<(TempDir, PathBuf, [u8; 32]), UpdaterError> {
		info!("Downloading update {version}");
		//		Prepare file to download to										
		let download_dir = tempdir().map_err(|err| UpdaterError::UnableToCreateTempDir(err.to_string()))?;
		let update_path  = download_dir.path().join(format!("update-{version}"));
		let mut file     = AsyncFile::create(&update_path).await.map_err(|err|
			UpdaterError::UnableToCreateDownload(update_path.clone(), err.to_string())
		)?;
		//		Check content type												
		let (url, response) = self.request(&format!("releases/{version}")).await?;
		let content_type = response.headers().get(CONTENT_TYPE).and_then(|h| h.to_str().ok()).unwrap_or("").to_owned();
		if content_type != "application/octet-stream" {
			return Err(UpdaterError::UnexpectedContentType(url, content_type, s!("application/octet-stream")));
		}
		//		Download release to file										
		let mut response_stream = response.bytes_stream();
		let mut hasher          = Sha256::new();
		//	Download in chunks, and update the SHA256 hash along the way
		while let Some(Ok(chunk)) = response_stream.next().await {
			file.write_all(&chunk).await.map_err(|err|
				UpdaterError::UnableToWriteToDownload(update_path.clone(), err.to_string())
			)?;
			hasher.update(&chunk);
		}
		let file_hash: [u8; 32] = hasher.finalize().into();
		Ok((download_dir, update_path, file_hash))
	}
	
	//		verify_update														
	/// Verifies an application update.
	/// 
	/// This function checks that the SHA256 hash of a downloaded file matches
	/// the hash provided by the API server.
	/// 
	/// # Errors
	/// 
	/// * [`UpdaterError::InvalidPayload`]
	/// * [`UpdaterError::FailedHashVerification`]
	/// 
	async fn verify_update(&self, version: &Version, hash: &[u8; 32]) -> Result<(), UpdaterError> {
		info!("Verifying update {version}");
		let (url, response) = self.request(&format!("hashes/{version}")).await?;
		match self.decode_and_verify::<VersionHashResponse>(url.clone(), response).await {
			Ok(json) => {
				if json.version != *version {
					return Err(UpdaterError::InvalidPayload(url));
				}
				if json.hash != hex::encode(hash) {
					return Err(UpdaterError::FailedHashVerification(version.clone()));
				}
				Ok(())
			},
			Err(err) => Err(err),
		}
	}
	
	//		request																
	/// Make HTTP request.
	/// 
	/// This function is responsible for handling communications with the API
	/// server.
	/// 
	/// # Errors
	/// 
	/// * [`UpdaterError::HttpError`]
	/// * [`UpdaterError::HttpRequestFailed`]
	/// * [`UpdaterError::InvalidUrl`]
	/// 
	async fn request(&self, endpoint: &str) -> Result<(Url, Response), UpdaterError> {
		//		Perform request													
		let Ok(url)  = self.config.api.join(endpoint) else {
			return Err(UpdaterError::InvalidUrl(self.config.api.clone(), endpoint.to_owned()));
		};
		let response = self.http_client.get(url.clone()).send().await.map_err(|err|
			UpdaterError::HttpRequestFailed(url.clone(), err.to_string())
		)?;
		//		Check status													
		let status = response.status();
		if !status.is_success() {
			return Err(UpdaterError::HttpError(url, status));
		}
		Ok((url, response))
	}
	
	//		decode_and_verify													
	/// Decodes a JSON HTTP response body and verifies signature.
	/// 
	/// This function accepts an HTTP response that contains a JSON payload,
	/// decodes it, and verifies the signature against the public key.
	/// 
	/// # Errors
	/// 
	/// * [`UpdaterError::FailedSignatureVerification`]
	/// * [`UpdaterError::InvalidBody`]
	/// * [`UpdaterError::InvalidPayload`]
	/// * [`UpdaterError::InvalidSignature`]
	/// * [`UpdaterError::MissingSignature`]
	/// * [`UpdaterError::UnexpectedContentType`]
	/// 
	async fn decode_and_verify<T: DeserializeOwned>(&self, url: Url, response: Response) -> Result<T, UpdaterError> {
		//		Check content type												
		let content_type = response.headers().get(CONTENT_TYPE).and_then(|h| h.to_str().ok()).unwrap_or("").to_owned();
		if content_type != "application/json" {
			return Err(UpdaterError::UnexpectedContentType(url, content_type, s!("application/json")));
		}
		//		Verify payload against signature								
		let signature = response.headers().get("x-signature").and_then(|h| h.to_str().ok()).unwrap_or("").to_owned();
		if signature.is_empty() {
			return Err(UpdaterError::MissingSignature(url));
		}
		let Ok(body) = response.text().await else {
			return Err(UpdaterError::InvalidBody(url))
		};
		let Ok(signature_bytes) = hex::decode(&signature) else {
			return Err(UpdaterError::InvalidSignature(url, signature))
		};
		let signature_array: &[u8; 64] = signature_bytes.as_slice().try_into().map_err(|_err|
			UpdaterError::InvalidSignature(url.clone(), signature)
		)?;
		if self.config.key.verify_strict(body.as_bytes(), &Signature::from_bytes(signature_array)).is_err() {
			return Err(UpdaterError::FailedSignatureVerification(url));
		}
		//		Decode payload													
		let Ok(parsed) = serde_json::from_str::<T>(&body) else {
			return Err(UpdaterError::InvalidPayload(url));
		};
		Ok(parsed)
	}
	
	//																			
}

//󰭅		Drop																	
impl Drop for Updater {
	//		drop																
    fn drop(&mut self) {
		//	Send a message to the queue to stop the timer, ignoring any errors
		let _ignored = self.queue.send(());
    }
}

//		LatestVersionResponse													
/// The application version returned by the `latest` endpoint.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
struct LatestVersionResponse {
	//		Private properties													
	/// The latest version of the application.
	version: Version,
}

//		VersionHashResponse														
/// The application hash and version returned by the `hashes/:version` endpoint.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
struct VersionHashResponse {
	//		Private properties													
	/// The requested version of the application.
	version: Version,
	
	/// The SHA256 hash of the application binary for this version.
	hash:    String,
}


