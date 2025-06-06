//! This module provides client-side functionality to add to an application.
//! 
//! The main struct is [`Updater`], which provides a service to check for
//! updates at intervals, and upgrade the application. This is done by querying
//! the specified API server, and if an update is found, it will be downloaded
//! and installed, and the application will be restarted. This will only occur
//! once any critical actions that have been registered are complete.
//! 
//! # Critical actions
//! 
//! Critical actions are registered using the [`Updater::register_action()`]
//! method, and deregistered using the [`Updater::deregister_action()`] method.
//! When a critical action is in progress, the updater will not restart the
//! application, and will wait until all critical actions are complete before
//! doing so.
//! 
//! # Status
//! 
//! The [`Status`] enum is used to represent the possible statuses that the
//! updater can have. The current status can be obtained at any time, using the
//! [`Updater::status()`] method, and the status change events can be
//! subscribed to using the [`Updater::subscribe()`] method.
//! 
//! # Failure
//! 
//! If an error occurs when trying to update, it will be logged, and the
//! updater will stop checking for updates.
//! 



//		Modules																											

#[cfg(test)]
#[path = "tests/client.rs"]
mod tests;



//		Packages																										

use crate::responses::{LatestVersionResponse, VersionHashResponse};
use core::{
	fmt::{Display, self},
	str::FromStr,
	sync::atomic::{AtomicUsize, Ordering},
};
use ed25519_dalek::Signature;
use flume::{Sender, self};
use futures_util::StreamExt as _;
use hex;
use parking_lot::RwLock;
use reqwest::{
	StatusCode,
	Url,
	header::{AsHeaderName, CONTENT_LENGTH, CONTENT_TYPE},
};
use rubedo::{
	crypto::{Sha256Hash, VerifyingKey},
	sugar::s,
};
use semver::Version;
use serde::de::DeserializeOwned;
use sha2::{Sha256, Digest as _};
use std::{
	env::args,
	io::Error as IoError,
	os::unix::fs::PermissionsExt as _,
	path::PathBuf,
	sync::Arc,
};
use tempfile::{tempdir, TempDir};
use thiserror::Error as ThisError;
use tokio::{
	fs::{File as AsyncFile, self},
	io::AsyncWriteExt as _,
	select,
	spawn,
	sync::broadcast::{Receiver as Listener, Sender as Broadcaster, self},
	time::{Duration, interval},
};
use tracing::{debug, error, info, warn};

#[cfg(not(test))]
use ::{
	reqwest::{Client, Response},
	std::{
		env::current_exe,
		os::unix::process::CommandExt as _,
		process::{Command, Stdio, exit},
	},
};
#[cfg(test)]
use crate::mocks::std_env::mock_current_exe as current_exe;
#[cfg(test)]
use sham::{
	reqwest::{MockClient as Client, MockResponse as Response},
	std_process::{FakeCommand as Command, MockStdio as Stdio, mock_exit as exit}
};



//		Enums																											

//		Status																	
/// The possible statuses that an [`Updater`] can have.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
#[non_exhaustive]
pub enum Status {
	/// Nothing interesting is currently happening — there is no active upgrade
	/// underway or pending.
	Idle,
	
	/// The updater is currently checking whether there is a newer version of
	/// the application available.
	Checking,
	
	/// A newer version of the application is available, and the updater is
	/// currently downloading the release file.
	Downloading(Version, u8),
	
	/// A newer version of the application is available, and the updater is
	/// currently installing it.
	Installing(Version),
	
	/// A newer version of the application is available, and the updater is
	/// currently waiting to start the upgrade process, but is blocked from
	/// doing so due to one or more critical actions being in progress.
	PendingRestart(Version),
	
	/// A newer version of the application is available, and the updater is
	/// currently in the process of restarting the application to apply the
	/// upgrade. No new critical actions are allowed to start.
	Restarting(Version),
}

//󰭅		Display																	
impl Display for Status {
	//		fmt																	
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		write!(f, "{}", match *self {
			Self::Idle                                  => s!(     "Idle"),
			Self::Checking                              => s!(     "Checking"),
			Self::Installing(ref version)               => format!("Installing: {version}"),
			Self::Downloading(ref version, ref percent) => format!("Downloading: {version} ({percent}%)"),
			Self::PendingRestart(ref version)           => format!("Pending restart: {version}"),
			Self::Restarting(ref version)               => format!("Restarting: {version}"),
		})
	}
}

//		UpdaterError															
/// Errors that can occur when trying to update.
#[derive(Clone, Debug, Eq, PartialEq, ThisError)]
#[non_exhaustive]
pub enum UpdaterError {
	/// Verification of the SHA256 hash of the downloaded file against the
	/// server's hash data failed.
	#[error("Failed hash verification for downloaded version {0}")]
	FailedHashVerification(Version),
	
	/// Verification of the HTTP response body against the signature header
	/// using the configured public key failed.
	#[error("Failed signature verification for response from {0}")]
	FailedSignatureVerification(Url),
	
	/// An HTTP error occurred, i.e. the status code returned is not `200`. No
	/// other codes are expected, as this library only performs `GET` requests.
	#[error("HTTP status code {1} received when calling {0}")]
	HttpError(Url, StatusCode),
	
	/// The HTTP request to the API server failed.
	#[error("HTTP request to {0} failed: {1}")]
	HttpRequestFailed(Url, String),
	
	/// The response from the API server could not be decoded. This could be due
	/// to malformed text that is not valid UTF-8 for endpoints that return text
	/// or JSON, or a truncated response.
	#[error("Invalid HTTP body received from {0}")]
	InvalidBody(Url),
	
	/// The response from the API server could not be parsed. This could be due
	/// to invalid JSON, or the JSON not matching the expected structure.
	#[error("Invalid payload received from {0}")]
	InvalidPayload(Url),
	
	/// The signature header from the API server could not be decoded.
	#[error(r#"Invalid signature header "{1}" received from {0}"#)]
	InvalidSignature(Url, String),
	
	/// The URL specified to use to make an HTTP request is invalid. The API URL
	/// should be okay due to type validation, so something must have happened
	/// when adding a particular endpoint to it, as the outcome is invalid.
	#[error("Invalid URL specified: {0} plus {1}")]
	InvalidUrl(Url, String),
	
	/// The HTTP response body from the API server is shorter than expected.
	#[error("HTTP response body from {0} is shorter than expected: {1} < {2}")]
	MissingData(Url, usize, usize),
	
	/// The HTTP response from the API server does not contain a signature
	/// header.
	#[error("HTTP response from {0} does not contain a signature header")]
	MissingSignature(Url),
	
	/// The HTTP response body from the API server is longer than expected.
	#[error("HTTP response body from {0} is longer than expected: {1} > {2}")]
	TooMuchData(Url, usize, usize),
	
	/// A problem was encountered when trying to create a file for the download.
	#[error(r#"Unable to create download file "{0:?}": {1}"#)]
	UnableToCreateDownload(PathBuf, String),
	
	/// A problem was encountered when trying to create a temporary directory.
	#[error("Unable to create temporary directory: {0}")]
	UnableToCreateTempDir(String),
	
	/// A problem was encountered when trying to get the metadata for the new
	/// executable.
	#[error(r#"Unable to get file metadata for the new executable "{0:?}": {1}"#)]
	UnableToGetFileMetadata(PathBuf, String),
	
	/// A problem was encountered when trying to move the new executable into
	/// the place of the current running application.
	#[error("Unable to move the new executable {0:?}: {1}")]
	UnableToMoveNewExe(PathBuf, String),
	
	/// A problem was encountered when trying to obtain the path of the current
	/// running application.
	#[error("Unable to obtain current executable path: {0}")]
	UnableToObtainCurrentExePath(String),
	
	/// A problem was encountered when trying to rename the current running
	/// application.
	#[error("Unable to rename the current executable {0:?}: {1}")]
	UnableToRenameCurrentExe(PathBuf, String),
	
	/// A problem was encountered when trying to set the new executable's file
	/// permissions.
	#[error(r#"Unable to set file permissions for the new executable "{0:?}": {1}"#)]
	UnableToSetFilePermissions(PathBuf, String),
	
	/// A problem was encountered when trying to write to the download file.
	#[error(r#"Unable to write to download file "{0:?}": {1}"#)]
	UnableToWriteToDownload(PathBuf, String),
	
	/// The content type of the response is not as expected.
	#[error(r#"HTTP response from {0} had unexpected content type: "{1}", expected: "{2}""#)]
	UnexpectedContentType(Url, String, String),
}



//		Structs																											

//		Config																	
/// The configuration options for the client.
#[expect(clippy::exhaustive_structs, reason = "Provided for configuration")]
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
	
	/// The status broadcast channel that status changes are added to. This is
	/// the sender side only. Each interested party can subscribe to this
	/// channel to receive status changes on a real-time basis.
	broadcast:   Broadcaster<Status>,
	
	/// The configuration for the updater service.
	config:      Config,
	
	/// The path to the current running executable. This is used to replace the
	/// executable with the new version when upgrading. It is checked at startup
	/// and stored here as a reliable reference so that the updater can use it
	/// later on.
	exe_path:    PathBuf,
	
	/// The HTTP client instance that is used for communicating with the API
	/// server.
	http_client: Client,
	
	/// The updater queue that is used for communicating with the interval
	/// timer. This is the sender side only. A queue is used so that the timer
	/// can run in a separate thread, but be stopped when required.
	queue:       Sender<()>,
	
	/// The current status of the updater.
	status:      RwLock<Status>,
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
	/// # Errors
	/// 
	/// * [`UpdaterError::UnableToObtainCurrentExePath`]
	/// 
	#[expect(clippy::result_large_err, reason = "Doesn't matter here")]
	pub fn new(config: Config) -> Result<Arc<Self>, UpdaterError> {
		//		Set up updater instance											
		let http_client        = Client::new();
		let (sender, receiver) = flume::unbounded();
		let (tx, mut rx)       = broadcast::channel(1);
		let updater            = Arc::new(Self {
			actions:     AtomicUsize::new(0),
			broadcast:   tx,
			config,
			exe_path:    current_exe().map_err(|err| UpdaterError::UnableToObtainCurrentExePath(err.to_string()))?,
			http_client,
			queue:       sender,
			status:      RwLock::new(Status::Idle),
		});
		//		Listen for status change events									
		//	It's useful to listen for status changes, so that they can be logged.
		//	However, a persistent subscriber is also necessary to keep the broadcast
		//	channel open, as it will be closed when the last subscriber is dropped.
		#[expect(clippy::pattern_type_mismatch, reason = "Cannot dereference here")]
		drop(spawn(async move { loop { select! {
			//	Wait for data from the broadcast channel
			Ok(status) = rx.recv() => {
				debug!("Status changed: {status}");
			}
			else => break,
		}}}));
		//		Check for updates at startup									
		if updater.config.check_on_startup {
			let startup_updater = Arc::clone(&updater);
			drop(spawn(async move {
				startup_updater.check_for_updates().await;
			}));
		}
		//		Check for updates at intervals									
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
		Ok(updater)
	}
	
	//		Public methods														
	
	//		register_action														
	/// Registers a critical action.
	/// 
	/// This function increments the critical actions counter, preventing the
	/// application from being updated while the critical action is in progress.
	/// 
	/// It returns the *likely new value* of the counter, or [`None`] if the
	/// counter overflows or if starting a new action is not permitted due to a
	/// pending update. The new value is likely rather than guaranteed due to
	/// the effect of concurrent updates, and therefore is the value known and
	/// set at the time it was incremented, and may not be the value by the time
	/// the function returns.
	/// 
	pub fn register_action(&self) -> Option<usize> {
		match self.status() {
			Status::Idle              |
			Status::Checking          |
			Status::Downloading(_, _) |
			Status::Installing(_)     => {},
			Status::PendingRestart(_) |
			Status::Restarting(_)     => return None,
		}
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
	/// If a restart is pending, then when the critical actions counter reaches
	/// zero, the restart will be triggered.
	/// 
	pub fn deregister_action(&self) -> Option<usize> {
		let mut value = self.actions
			.fetch_update(Ordering::SeqCst, Ordering::SeqCst, |value| { value.checked_sub(1) })
			.ok()?
		;
		value = value.saturating_sub(1);
		if let Status::PendingRestart(version) = self.status() {
			if value > 0 {
				info!("Pending restart: {} critical actions in progress", self.actions.load(Ordering::SeqCst));
			} else {
				self.set_status(Status::Restarting(version));
				info!("Restarting");
				self.restart();
			}
		}
		Some(value)
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
	
	//		status																
	/// Gets the current status of the updater.
	/// 
	/// This function returns the current status of the updater, correct at the
	/// time of calling.
	/// 
	/// Note that the status may change between the time of calling and the time
	/// of processing the result.
	/// 
	pub fn status(&self) -> Status {
		let lock = self.status.read();                                   //  //
		(*lock).clone()
	}
	
	//		set_status															
	/// Sets the current status of the updater.
	/// 
	/// This function changes the current status of the updater to the specified
	/// value.
	/// 
	pub fn set_status(&self, status: Status) {
		let mut lock = self.status.write();                              //  //
		*lock        = status.clone();
		drop(lock);                                                      //  //
		if let Err(err) = self.broadcast.send(status) {
			error!("Failed to broadcast status change: {err}");
		}
	}
	
	//		subscribe															
	/// Subscribes to the status change event broadcaster.
	/// 
	/// This function provides a receiver that is subscribed to the status
	/// change event broadcaster, so that every time the status changes, it will
	/// be notified.
	/// 
	/// At present this simply subscribes to all status change events, but it
	/// may be enhanced in future to allow for filtering.
	/// 
	pub fn subscribe(&self) -> Listener<Status> {
		self.broadcast.subscribe()
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
		//		Ensure no updates are already underway							
		if self.status() != Status::Idle {
			return;
		}
		//		Get latest version												
		self.set_status(Status::Checking);
		info!("Checking for updates");
		let (url, response) = match self.request("latest").await {
			Ok(data) => data,
			Err(err) => {
				self.set_status(Status::Idle);
				error!("Error checking for updates: {err}");
				return;
			},
		};
		let version = match self.decode_and_verify::<LatestVersionResponse>(url, response).await {
			Ok(json) => json.version,
			Err(err) => {
				self.set_status(Status::Idle);
				error!("Error checking for updates: {err}");
				return;
			},
		};
		//		Compare to current version										
		if version <= self.config.version {
			self.set_status(Status::Idle);
			info!("The current version {} is the latest available", self.config.version);
			return;
		}
		info!("New version {} available", version);
		//		Download update file											
		self.set_status(Status::Downloading(version.clone(), 0));
		info!("Downloading update {version}");
		let (_download_dir, update_path, file_hash) = match self.download_update(&version).await {
			Ok(data) => data,
			Err(err) => {
				error!("Error downloading update file: {err}");
				return;
			},
		};
		info!("Update file downloaded");
		//		Verify update file												
		info!("Verifying update {version}");
		if let Err(err) = self.verify_update(&version, file_hash).await {
			error!("Error verifying update file: {err}");
			return;
		}
		info!("Update file verified");
		//		Install update													
		self.set_status(Status::Installing(version.clone()));
		info!("Installing update");
		if let Err(err) = self.replace_executable(&update_path).await {
			error!("Error installing update: {err}");
			return;
		}
		//		Restart application												
		if !self.is_safe_to_update() {
			self.set_status(Status::PendingRestart(version.clone()));
			info!("Pending restart: {} critical actions in progress", self.actions.load(Ordering::SeqCst));
			return;
		}
		self.set_status(Status::Restarting(version.clone()));
		info!("Restarting");
		self.restart();
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
	/// * [`UpdaterError::MissingData`]
	/// * [`UpdaterError::TooMuchData`]
	/// * [`UpdaterError::UnableToCreateDownload`]
	/// * [`UpdaterError::UnableToCreateTempDir`]
	/// * [`UpdaterError::UnableToWriteToDownload`]
	/// * [`UpdaterError::UnexpectedContentType`]
	/// 
	async fn download_update(&self, version: &Version) -> Result<(TempDir, PathBuf, Sha256Hash), UpdaterError> {
		//		Prepare file to download to										
		let download_dir = tempdir().map_err(|err| UpdaterError::UnableToCreateTempDir(err.to_string()))?;
		let update_path  = download_dir.path().join(format!("update-{version}"));
		let mut file     = AsyncFile::create(&update_path).await.map_err(|err|
			UpdaterError::UnableToCreateDownload(update_path.clone(), err.to_string())
		)?;
		//		Get headers														
		let (url, response) = self.request(&format!("releases/{version}")).await?;
		let content_type:   String = get_header(&response, CONTENT_TYPE);
		let content_length: usize  = get_header(&response, CONTENT_LENGTH);
		//		Check content type												
		if content_type != "application/octet-stream" {
			return Err(UpdaterError::UnexpectedContentType(url, content_type, s!("application/octet-stream")));
		}
		//		Download release to file										
		let mut response_stream = response.bytes_stream();
		let mut hasher          = Sha256::new();
		let mut body_len        = 0_usize;
		//	Download in chunks, and update the SHA256 hash along the way
		while let Some(Ok(chunk)) = response_stream.next().await {
			file.write_all(&chunk).await.map_err(|err|
				UpdaterError::UnableToWriteToDownload(update_path.clone(), err.to_string())
			)?;
			hasher.update(&chunk);
			body_len = body_len.saturating_add(chunk.len());
			#[expect(clippy::cast_possible_truncation, reason = "Loss of precision is not important here")]
			#[expect(clippy::cast_precision_loss,      reason = "Loss of precision is not important here")]
			#[expect(clippy::cast_sign_loss,           reason = "Loss of sign is not important here")]
			self.set_status(Status::Downloading(version.clone(), (body_len as f64 / content_length as f64 * 100.0) as u8));
		}
		//		Check content length											
		if body_len < content_length {
			return Err(UpdaterError::MissingData(url, body_len, content_length));
		}
		if body_len > content_length {
			return Err(UpdaterError::TooMuchData(url, body_len, content_length));
		}
		Ok((download_dir, update_path, hasher.finalize().into()))
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
	async fn verify_update(&self, version: &Version, hash: Sha256Hash) -> Result<(), UpdaterError> {
		let (url, response) = self.request(&format!("hashes/{version}")).await?;
		match self.decode_and_verify::<VersionHashResponse>(url.clone(), response).await {
			Ok(json) => {
				if json.version != *version {
					return Err(UpdaterError::InvalidPayload(url));
				}
				if json.hash != hash {
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
	/// * [`UpdaterError::MissingData`]
	/// * [`UpdaterError::MissingSignature`]
	/// * [`UpdaterError::TooMuchData`]
	/// * [`UpdaterError::UnexpectedContentType`]
	/// 
	async fn decode_and_verify<T: DeserializeOwned>(&self, url: Url, response: Response) -> Result<T, UpdaterError> {
		//		Get headers														
		let content_type:   String = get_header(&response, CONTENT_TYPE);
		let content_length: usize  = get_header(&response, CONTENT_LENGTH);
		let signature:      String = get_header(&response, "x-signature");
		//		Get body														
		let Ok(body) = response.text().await else {
			return Err(UpdaterError::InvalidBody(url))
		};
		//		Check headers													
		if content_type != "application/json" {
			return Err(UpdaterError::UnexpectedContentType(url, content_type, s!("application/json")));
		}
		if body.len() < content_length {
			return Err(UpdaterError::MissingData(url, body.len(), content_length));
		}
		if body.len() > content_length {
			return Err(UpdaterError::TooMuchData(url, body.len(), content_length));
		}
		if signature.is_empty() {
			return Err(UpdaterError::MissingSignature(url));
		}
		//		Verify payload against signature								
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
	
	//		replace_executable													
	/// Replaces the current executable with the updated one.
	/// 
	/// This function renames the currently-running executable with a `.old`
	/// suffix, and moves the downloaded update into its place.
	/// 
	/// Note that at present it naively assumes that the backup filename doesn't
	/// exist. It also does not attempt to rename the backup executable back to
	/// the original name if moving the new executable fails. This behaviour
	/// will be improved in future.
	/// 
	/// # Errors
	/// 
	/// * [`UpdaterError::UnableToGetFileMetadata`]
	/// * [`UpdaterError::UnableToMoveNewExe`]
	/// * [`UpdaterError::UnableToRenameCurrentExe`]
	/// * [`UpdaterError::UnableToSetFilePermissions`]
	/// 
	async fn replace_executable(&self, update_path: &PathBuf) -> Result<(), UpdaterError> {
		let current_path = self.exe_path.clone();
		let backup_path  = current_path.with_extension("old");
		let move_error   = |err: IoError| -> UpdaterError {
			UpdaterError::UnableToMoveNewExe(update_path.clone(), err.to_string())
		};
		fs::rename(&current_path, &backup_path).await.map_err(|err|
			UpdaterError::UnableToRenameCurrentExe(current_path.clone(), err.to_string())
		)?;
		if let Err(err) = fs::rename(&update_path, &current_path).await {
			//	Check for cross-device move error and fall back to copy + delete. 18 is
			//	a magic number for the error code for `EXDEV` (cross-device link), which
			//	is not available in the standard library.
			if err.raw_os_error() != Some(18_i32) {
				return Err(move_error(err));
			}
			let _size = fs::copy(&update_path, &current_path).await.map_err(move_error)?;
			if let Err(err2) = fs::remove_file(&update_path).await {
				warn!("Failed to delete temporary update file {update_path:?}: {err2}");
			}
		}
		let mut permissions = fs::metadata(&current_path).await.map_err(|err|
			UpdaterError::UnableToGetFileMetadata(current_path.clone(), err.to_string())
		)?.permissions();
		//	Add executable bits for all (owner, group, others)
		permissions.set_mode(permissions.mode() | 0o111);
		fs::set_permissions(&current_path, permissions).await.map_err(|err|
			UpdaterError::UnableToSetFilePermissions(current_path.clone(), err.to_string())
		)?;
		Ok(())
	}
	
	//		restart																
	/// Restarts the application.
	/// 
	/// This function restarts the currently-running application, for the
	/// primary purpose of replacing it with the newer version.
	/// 
	/// It does so while preserving all arguments originally specified. It will
	/// inherit the standard I/O streams from the current process, ensuring
	/// seamless input and output behaviour, and will replace the currently
	/// running process with the new one.
	/// 
	/// If the application fails to restart, this function will log an error,
	/// and then exit. In this situation there's not a lot else to do at
	/// present, as behaviour in such a scenario is undefined, and given that
	/// all critical actions have been paused, exiting seems the most sensible
	/// option. This behaviour will be considered carefully and improved in
	/// future when it becomes clearer how to handle it.
	/// 
	fn restart(&self) {
		//	Skip the first argument (current executable name)
		let args = args().skip(1).collect::<Vec<_>>();
		let err  = Command::new(self.exe_path.clone())
			.args(args)
			.stdin(Stdio::inherit())
			.stdout(Stdio::inherit())
			.stderr(Stdio::inherit())
			.exec()
		;
		//	A failure to restart the application is fatal to the installer
		//	process, so although we won't panic, we also won't continue. We'll
		//	just exit the application. This is a candidate for potential
		//	improvement in future, to allow for more graceful handling of this
		//	situation.
		error!("Failed to restart application: {err}");
		exit(1);
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



//		Functions																										

//		get_header																
/// Gets a header from an HTTP response.
/// 
/// This function gets a header from an HTTP response, and converts it to the
/// specified type.
/// 
/// # Parameters
/// 
/// * `response` - The HTTP response to get the header from.
/// * `header`   - The header to get from the response.
/// 
fn get_header<K, T>(response: &Response, header: K) -> T
where
	K: AsHeaderName,
	T: Default + FromStr
{
	response.headers()
		.get(header)
		.and_then(|h| h.to_str().ok())
		.and_then(|s| T::from_str(s).ok())
		.unwrap_or_default()
}


