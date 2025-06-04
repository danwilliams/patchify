#![allow(clippy::bool_assert_comparison, reason = "Clarity")]

//		Packages

use super::*;
use crate::common::utils::*;
use crate::mocks::{
	MockSubscriber,
	Subscriber as _,
	reqwest::{create_mock_binary_response, create_mock_response},
	std_env::MOCK_EXE,
};
use assert_json_diff::assert_json_eq;
use claims::{assert_err_eq, assert_ok, assert_none, assert_some};
use futures_util::future::FutureExt as _;
use parking_lot::ReentrantMutexGuard;
use reqwest::StatusCode;
use rubedo::std::ByteSized as _;
use serde_json::{Value as JsonValue, json};
use core::cell::RefCell;
use sham::reqwest::{MockClient, create_mock_client, create_mock_response as create_sham_response};
use std::{
	fs::{File, self},
	io::Write as _,
	sync::LazyLock,
};
use tokio::{
	fs as async_fs,
	time::sleep,
};
use tempfile::{TempDir, tempdir};



//		Statics

static EMPTY_PUBLIC_KEY: LazyLock<VerifyingKey> = LazyLock::new(|| VerifyingKey::from_bytes([0; 32]));



//		Common

//		setup_safe_updater														
/// This function sets up a safe `Updater` instance for testing.
/// 
/// The `Updater` instance is created by bypassing `Updater::new()`, so that the
/// real checks are not triggered. Additionally, it is created with a mock
/// `Client` instance, so that no actual network requests will be made.
/// 
fn setup_safe_updater(
	version:     Version,
	api:         &str,
	key:         VerifyingKey,
	mock_client: MockClient,
) -> Updater {
	//	These are needed for creation, but won't be used in tests
	let (sender, _) = flume::unbounded();
	let (tx, _rx)   = broadcast::channel(1);
	//	The Updater instance needs to be created manually in order to bypass the
	//	actions performed in the new() method
	Updater {
		actions:     AtomicUsize::new(0),
		broadcast:   tx,
		config:      Config {
			version,
			api:     api.parse().unwrap(),
			key,
			check_on_startup: false,
			check_interval:   None,
		},
		exe_path:    MOCK_EXE.lock().borrow().as_ref().map_or_else(PathBuf::new, Clone::clone),
		http_client: mock_client,
		queue:       sender,
		status:      RwLock::new(Status::Idle),
	}
}

//		setup_files																
fn setup_files<'lock>() -> (
	ReentrantMutexGuard<'lock, RefCell<Option<PathBuf>>>,
	TempDir,
	PathBuf,
	PathBuf,
	PathBuf,
) {
	let temp_dir = tempdir().unwrap();
	let exe_path = temp_dir.path().join("mock_exe");
	let old_path = exe_path.with_extension("old");
	let new_path = temp_dir.path().join("update");
	let lock     = MOCK_EXE.lock();
	drop(lock.borrow_mut().replace(exe_path.clone()));
	File::create(&exe_path).unwrap().write_all(b"mock_exe contents").unwrap();
	File::create(&new_path).unwrap().write_all(b"update contents").unwrap();
	(lock, temp_dir, exe_path, old_path, new_path)
}



//		Tests

//		Updater																	
#[cfg(test)]
mod updater_construction {
	use super::*;
	
	//		new																	
	#[tokio::test]
	async fn new() {
		//	The lock needs to be maintained for the duration of the test. We call
		//	setup_files() to ensure that the mock executable path is set.
		let (_lock, _temp_dir, _, _, _) = setup_files();
		let order   = Ordering::SeqCst;
		let updater = Updater::new(Config {
			version:          Version::new(1, 0, 0),
			api:              "https://api.example.com".parse().unwrap(),
			key:              *EMPTY_PUBLIC_KEY,
			check_on_startup: false,
			check_interval:   Some(Duration::from_secs(60 * 60)),
		}).unwrap();
		assert_eq!(updater.actions.load(order),     0);
		assert_eq!(updater.config.version,          Version::new(1, 0, 0));
		assert_eq!(updater.config.api,              "https://api.example.com".parse().unwrap());
		assert_eq!(updater.config.key,              *EMPTY_PUBLIC_KEY);
		assert_eq!(updater.config.check_on_startup, false);
		assert_eq!(updater.config.check_interval,   Some(Duration::from_secs(60 * 60)));
		assert_eq!(updater.exe_path,                *MOCK_EXE.lock().borrow().as_ref().unwrap());
		assert_eq!(*updater.status.read(),          Status::Idle);
	}
	#[tokio::test]
	async fn new__err_unable_to_obtain_current_exe_path() {
		//	No test for this at present, as it is difficult to simulate a failure.
		//	It's also quite unlikely to occur.
	}
}

#[cfg(test)]
mod updater_public {
	use super::*;
	
	//		register_action														
	#[tokio::test]
	async fn register_action() {
		let order   = Ordering::SeqCst;
		let updater = setup_safe_updater(
			Version::new(1, 0, 0),
			"https://api.example.com/api/",
			*EMPTY_PUBLIC_KEY,
			MockClient::new(),
		);
		assert_eq!(updater.actions.load(order), 0);
		assert_eq!(updater.register_action(),   Some(1));
		assert_eq!(updater.actions.load(order), 1);
		assert_eq!(updater.register_action(),   Some(2));
		assert_eq!(updater.actions.load(order), 2);
	}
	#[tokio::test]
	async fn register_action__when_updating() {
		let updater = setup_safe_updater(
			Version::new(1, 0, 0),
			"https://api.example.com/api/",
			*EMPTY_PUBLIC_KEY,
			MockClient::new(),
		);
		updater.set_status(Status::Checking);
		assert_some!(updater.register_action());
		updater.set_status(Status::Downloading(Version::new(1, 0, 0), 25));
		assert_some!(updater.register_action());
		updater.set_status(Status::Installing(Version::new(1, 0, 0)));
		assert_some!(updater.register_action());
		updater.set_status(Status::PendingRestart(Version::new(1, 0, 0)));
		assert_none!(updater.register_action());
		updater.set_status(Status::Restarting(Version::new(1, 0, 0)));
		assert_none!(updater.register_action());
		updater.set_status(Status::Idle);
		assert_some!(updater.register_action());
	}
	#[tokio::test]
	async fn register_action__overflow() {
		let order   = Ordering::SeqCst;
		let updater = setup_safe_updater(
			Version::new(1, 0, 0),
			"https://api.example.com/api/",
			*EMPTY_PUBLIC_KEY,
			MockClient::new(),
		);
		let _ = updater.actions.fetch_add(usize::MAX - 1, order);
		assert_eq!(updater.actions.load(order), usize::MAX - 1);
		assert_eq!(updater.register_action(),   Some(usize::MAX));
		assert_eq!(updater.actions.load(order), usize::MAX);
		assert_eq!(updater.register_action(),   None);
		assert_eq!(updater.actions.load(order), usize::MAX);
	}
	
	//		deregister_action													
	#[tokio::test]
	async fn deregister_action() {
		let order   = Ordering::SeqCst;
		let updater = setup_safe_updater(
			Version::new(1, 0, 0),
			"https://api.example.com/api/",
			*EMPTY_PUBLIC_KEY,
			MockClient::new(),
		);
		assert_eq!(updater.actions.load(order), 0);
		assert_eq!(updater.register_action(),   Some(1));
		assert_eq!(updater.register_action(),   Some(2));
		assert_eq!(updater.register_action(),   Some(3));
		assert_eq!(updater.actions.load(order), 3);
		assert_eq!(updater.deregister_action(), Some(2));
		assert_eq!(updater.deregister_action(), Some(1));
		assert_eq!(updater.actions.load(order), 1);
		assert_eq!(updater.deregister_action(), Some(0));
		assert_eq!(updater.actions.load(order), 0);
	}
	#[tokio::test]
	async fn deregister_action__when_updating() {
		let updater = setup_safe_updater(
			Version::new(1, 0, 0),
			"https://api.example.com/api/",
			*EMPTY_PUBLIC_KEY,
			MockClient::new(),
		);
		let _ = updater.actions.fetch_add(10, Ordering::SeqCst);
		updater.set_status(Status::Checking);
		assert_some!(updater.deregister_action());
		updater.set_status(Status::Downloading(Version::new(1, 0, 0), 25));
		assert_some!(updater.deregister_action());
		updater.set_status(Status::Installing(Version::new(1, 0, 0)));
		assert_some!(updater.deregister_action());
		updater.set_status(Status::PendingRestart(Version::new(1, 0, 0)));
		assert_some!(updater.deregister_action());
		updater.set_status(Status::Restarting(Version::new(1, 0, 0)));
		assert_some!(updater.deregister_action());
		updater.set_status(Status::Idle);
		assert_some!(updater.deregister_action());
	}
	#[tokio::test]
	async fn deregister_action__when_restart_pending() {
		let updater = setup_safe_updater(
			Version::new(1, 0, 0),
			"https://api.example.com/api/",
			*EMPTY_PUBLIC_KEY,
			MockClient::new(),
		);
		let _ = updater.actions.fetch_add(3, Ordering::SeqCst);
		updater.set_status(Status::PendingRestart(Version::new(1, 0, 0)));
		assert_eq!(updater.deregister_action(), Some(2));
		assert_eq!(updater.status(),            Status::PendingRestart(Version::new(1, 0, 0)));
		assert_eq!(updater.deregister_action(), Some(1));
		assert_eq!(updater.status(),            Status::PendingRestart(Version::new(1, 0, 0)));
		assert_eq!(updater.deregister_action(), Some(0));
		assert_eq!(updater.status(),            Status::Restarting(Version::new(1, 0, 0)));
		//	Due to the status change, the restart() method will now be called. This
		//	will call FakeCommand::new(), which will return a wrapper around a
		//	MockCommand that is already set up with the necessary expectations.
	}
	#[tokio::test]
	async fn deregister_action__underflow() {
		let order   = Ordering::SeqCst;
		let updater = setup_safe_updater(
			Version::new(1, 0, 0),
			"https://api.example.com/api/",
			*EMPTY_PUBLIC_KEY,
			MockClient::new(),
		);
		assert_eq!(updater.actions.load(order), 0);
		assert_eq!(updater.register_action(),   Some(1));
		assert_eq!(updater.actions.load(order), 1);
		assert_eq!(updater.deregister_action(), Some(0));
		assert_eq!(updater.actions.load(order), 0);
		assert_eq!(updater.deregister_action(), None);
		assert_eq!(updater.actions.load(order), 0);
	}
	
	//		is_safe_to_update													
	#[tokio::test]
	async fn is_safe_to_update() {
		let order   = Ordering::SeqCst;
		let updater = setup_safe_updater(
			Version::new(1, 0, 0),
			"https://api.example.com/api/",
			*EMPTY_PUBLIC_KEY,
			MockClient::new(),
		);
		assert_eq!(updater.actions.load(order), 0);
		assert_eq!(updater.is_safe_to_update(), true);
		assert_eq!(updater.register_action(),   Some(1));
		assert_eq!(updater.is_safe_to_update(), false);
		assert_eq!(updater.register_action(),   Some(2));
		assert_eq!(updater.is_safe_to_update(), false);
		assert_eq!(updater.deregister_action(), Some(1));
		assert_eq!(updater.is_safe_to_update(), false);
		assert_eq!(updater.deregister_action(), Some(0));
		assert_eq!(updater.is_safe_to_update(), true);
		assert_eq!(updater.register_action(),   Some(1));
		assert_eq!(updater.is_safe_to_update(), false);
		assert_eq!(updater.deregister_action(), Some(0));
		assert_eq!(updater.is_safe_to_update(), true);
	}
	
	//		status																
	#[tokio::test]
	async fn status() {
		let updater = setup_safe_updater(
			Version::new(1, 0, 0),
			"https://api.example.com/api/",
			*EMPTY_PUBLIC_KEY,
			MockClient::new(),
		);
		assert_eq!(*updater.status.read(), Status::Idle);
		assert_eq!(updater.status(),       Status::Idle);
		let mut lock = updater.status.write();
		*lock        = Status::Restarting(Version::new(1, 0, 0));
		drop(lock);
		assert_eq!(*updater.status.read(), Status::Restarting(Version::new(1, 0, 0)));
		assert_eq!(updater.status(),       Status::Restarting(Version::new(1, 0, 0)));
	}
	
	//		set_status															
	#[tokio::test]
	async fn set_status() {
		let updater = setup_safe_updater(
			Version::new(1, 0, 0),
			"https://api.example.com/api/",
			*EMPTY_PUBLIC_KEY,
			MockClient::new(),
		);
		assert_eq!(updater.status(), Status::Idle);
		updater.set_status(Status::Checking);
		assert_eq!(updater.status(), Status::Checking);
		updater.set_status(Status::Downloading(Version::new(1, 0, 0), 50));
		assert_eq!(updater.status(), Status::Downloading(Version::new(1, 0, 0), 50));
		updater.set_status(Status::Idle);
		assert_eq!(updater.status(), Status::Idle);
	}
	
	//		subscribe															
	#[tokio::test]
	async fn subscribe() {
		let mut mock_subscriber = MockSubscriber::new();
		let _ = mock_subscriber.expect_update()
			.withf(|status| *status == Status::Checking)
			.times(1)
			.return_const(())
		;
		let updater = setup_safe_updater(
			Version::new(1, 0, 0),
			"https://api.example.com/api/",
			*EMPTY_PUBLIC_KEY,
			MockClient::new(),
		);
		let (sender, receiver) = flume::unbounded();
		let mut rx = updater.subscribe();
		#[expect(clippy::pattern_type_mismatch, reason = "Tokio code")]
		let thread = spawn(async move { select! {
			Ok(status) = rx.recv()             => mock_subscriber.update(status),
			_          = receiver.recv_async() => {},
		}});
		updater.set_status(Status::Checking);
		sleep(Duration::from_millis(10)).await;
		let _ignored = sender.send(());
		thread.await.unwrap();
	}
	#[tokio::test]
	async fn subscribe__no_status_change_events() {
		let mut mock_subscriber = MockSubscriber::new();
		let _ = mock_subscriber.expect_update()
			.withf(|status| *status == Status::Checking)
			.times(1)
			.return_const(())
		;
		let updater = setup_safe_updater(
			Version::new(1, 0, 0),
			"https://api.example.com/api/",
			*EMPTY_PUBLIC_KEY,
			MockClient::new(),
		);
		let (sender, receiver) = flume::unbounded();
		let mut rx = updater.subscribe();
		#[expect(clippy::pattern_type_mismatch, reason = "Tokio code")]
		let thread = spawn(async move { select! {
			Ok(status) = rx.recv()             => mock_subscriber.update(status),
			_          = receiver.recv_async() => {}
		}});
		sleep(Duration::from_millis(10)).await;
		sender.send(()).unwrap();
		assert!(async { thread.await.unwrap() }.catch_unwind().await.is_err());
	}
}

#[cfg(test)]
mod updater_private {
	use sham::reqwest::MockError;
	use crate::mocks::reqwest::ResponseSignature;
	use super::*;
	
	//		check_for_updates													
	#[tokio::test]
	async fn check_for_updates__complete_successful_process() {
		//	The lock and temp_dir need to be maintained for the duration of the test
		let (_lock, _temp_dir, _, _, _)  = setup_files();
		let version                      = Version::new(2, 3, 4);
		let private_key                  = generate_new_private_key();
		let url1                         = "https://api.example.com/api/latest";
		let url2                         = "https://api.example.com/api/releases/2.3.4";
		let url3                         = "https://api.example.com/api/hashes/2.3.4";
		let payload                      = b"Test payload";
		let json1                        = json!({
			"version": s!("2.3.4"),
		}).to_string();
		let json2                        = json!({
			"version": s!("2.3.4"),
			"hash":    hex::encode(Sha256::digest(payload)),
		}).to_string();
		let (mock_response1, public_key) = create_mock_response(
			url1,
			StatusCode::OK,
			Some("application/json"),
			Some(json1.len()),
			Ok(&json1),
			&ResponseSignature::GenerateUsing(private_key.clone()),
		);
		let mock_response2 = create_mock_binary_response(
			url2,
			StatusCode::OK,
			Some("application/octet-stream"),
			Some(payload.len()),
			Ok(payload),
		);
		let (mock_response3, _public_key) = create_mock_response(
			url3,
			StatusCode::OK,
			Some("application/json"),
			Some(json2.len()),
			Ok(&json2),
			&ResponseSignature::GenerateUsing(private_key.clone()),
		);
		let mock_client = create_mock_client(vec![
			(url1, Ok(mock_response1)),
			(url2, Ok(mock_response2)),
			(url3, Ok(mock_response3)),
		]);
		let updater = setup_safe_updater(
			Version::new(1, 0, 0),
			"https://api.example.com/api/",
			public_key,
			mock_client,
		);
		assert_eq!(updater.status(), Status::Idle);
		updater.check_for_updates().await;
		assert_eq!(updater.status(), Status::Restarting(version.clone()));
	}
	#[tokio::test]
	async fn check_for_updates__update_check_already_underway() {
		//	The lock needs to be maintained for the duration of the test. We call
		//	setup_files() to ensure that the mock executable path is set.
		let (_lock, _temp_dir, _, _, _) = setup_files();
		let updater = setup_safe_updater(
			Version::new(1, 0, 0),
			"https://api.example.com/api/",
			*EMPTY_PUBLIC_KEY,
			MockClient::new(),
		);
		updater.set_status(Status::Checking);
		updater.check_for_updates().await;
		assert_eq!(updater.status(), Status::Checking);
		updater.set_status(Status::Downloading(Version::new(1, 0, 0), 80));
		updater.check_for_updates().await;
		assert_eq!(updater.status(), Status::Downloading(Version::new(1, 0, 0), 80));
		updater.set_status(Status::Installing(Version::new(1, 0, 0)));
		updater.check_for_updates().await;
		assert_eq!(updater.status(), Status::Installing(Version::new(1, 0, 0)));
		updater.set_status(Status::PendingRestart(Version::new(1, 0, 0)));
		updater.check_for_updates().await;
		assert_eq!(updater.status(), Status::PendingRestart(Version::new(1, 0, 0)));
		updater.set_status(Status::Restarting(Version::new(1, 0, 0)));
		updater.check_for_updates().await;
		assert_eq!(updater.status(), Status::Restarting(Version::new(1, 0, 0)));
	}
	#[tokio::test]
	async fn check_for_updates__no_update_available() {
		let url                         = "https://api.example.com/api/latest";
		let json                        = json!({
			"version": s!("1.0.0"),
		}).to_string();
		let (mock_response, public_key) = create_mock_response(
			url,
			StatusCode::OK,
			Some("application/json"),
			Some(json.len()),
			Ok(&json),
			&ResponseSignature::Generate,
		);
		let mock_client = create_mock_client(vec![
			(url, Ok(mock_response)),
		]);
		let updater = setup_safe_updater(
			Version::new(1, 0, 0),
			"https://api.example.com/api/",
			public_key,
			mock_client,
		);
		assert_eq!(updater.status(), Status::Idle);
		updater.check_for_updates().await;
		assert_eq!(updater.status(), Status::Idle);
	}
	#[tokio::test]
	async fn check_for_updates__download_failed() {
		let version                      = Version::new(2, 3, 4);
		let url1                         = "https://api.example.com/api/latest";
		let url2                         = "https://api.example.com/api/releases/2.3.4";
		let payload                      = b"Test payload";
		let json                         = json!({
			"version": s!("2.3.4"),
		}).to_string();
		let (mock_response1, public_key) = create_mock_response(
			url1,
			StatusCode::OK,
			Some("application/json"),
			Some(json.len()),
			Ok(&json),
			&ResponseSignature::Generate,
		);
		let mock_response2 = create_mock_binary_response(
			url2,
			StatusCode::OK,
			//	Intentionally-incorrect content type, to make the process fail
			Some("text/plain"),
			Some(payload.len()),
			Ok(payload),
		);
		let mock_client = create_mock_client(vec![
			(url1, Ok(mock_response1)),
			(url2, Ok(mock_response2)),
		]);
		let updater = setup_safe_updater(
			Version::new(1, 0, 0),
			"https://api.example.com/api/",
			public_key,
			mock_client,
		);
		assert_eq!(updater.status(), Status::Idle);
		updater.check_for_updates().await;
		assert_eq!(updater.status(), Status::Downloading(version.clone(), 0));
	}
	#[tokio::test]
	async fn check_for_updates__download_partial() {
		let version                      = Version::new(2, 3, 4);
		let url1                         = "https://api.example.com/api/latest";
		let url2                         = "https://api.example.com/api/releases/2.3.4";
		let payload                      = b"Test payload";
		let json                         = json!({
			"version": s!("2.3.4"),
		}).to_string();
		let (mock_response1, public_key) = create_mock_response(
			url1,
			StatusCode::OK,
			Some("application/json"),
			Some(json.len()),
			Ok(&json),
			&ResponseSignature::Generate,
		);
		let mock_response2 = create_mock_binary_response(
			url2,
			StatusCode::OK,
			Some("application/octet-stream"),
			//	Intentionally-incorrect content length, to make the process fail
			Some(payload.len() * 2),
			Ok(payload),
		);
		let mock_client = create_mock_client(vec![
			(url1, Ok(mock_response1)),
			(url2, Ok(mock_response2)),
		]);
		let updater = setup_safe_updater(
			Version::new(1, 0, 0),
			"https://api.example.com/api/",
			public_key,
			mock_client,
		);
		assert_eq!(updater.status(), Status::Idle);
		updater.check_for_updates().await;
		assert_eq!(updater.status(), Status::Downloading(version.clone(), 50));
	}
	#[tokio::test]
	async fn check_for_updates__download_full() {
		let version                      = Version::new(2, 3, 4);
		let private_key                  = generate_new_private_key();
		let url1                         = "https://api.example.com/api/latest";
		let url2                         = "https://api.example.com/api/releases/2.3.4";
		let url3                         = "https://api.example.com/api/hashes/2.3.4";
		let payload                      = b"Test payload";
		let json1                        = json!({
			"version": s!("2.3.4"),
		}).to_string();
		let json2                        = json!({
			"version": s!("2.3.4"),
			//	Intentionally-incorrect hash, to make the process fail
			"hash":    hex::encode(Sha256::digest("Some other payload")),
		}).to_string();
		let (mock_response1, public_key) = create_mock_response(
			url1,
			StatusCode::OK,
			Some("application/json"),
			Some(json1.len()),
			Ok(&json1),
			&ResponseSignature::GenerateUsing(private_key.clone()),
		);
		let mock_response2 = create_mock_binary_response(
			url2,
			StatusCode::OK,
			Some("application/octet-stream"),
			Some(payload.len()),
			Ok(payload),
		);
		let (mock_response3, _public_key) = create_mock_response(
			url3,
			StatusCode::OK,
			Some("application/json"),
			Some(json2.len()),
			Ok(&json2),
			&ResponseSignature::GenerateUsing(private_key.clone()),
		);
		let mock_client = create_mock_client(vec![
			(url1, Ok(mock_response1)),
			(url2, Ok(mock_response2)),
			(url3, Ok(mock_response3)),
		]);
		let updater = setup_safe_updater(
			Version::new(1, 0, 0),
			"https://api.example.com/api/",
			public_key,
			mock_client,
		);
		assert_eq!(updater.status(), Status::Idle);
		updater.check_for_updates().await;
		assert_eq!(updater.status(), Status::Downloading(version.clone(), 100));
	}
	#[tokio::test]
	async fn check_for_updates__install_failed() {
		let version                      = Version::new(2, 3, 4);
		let private_key                  = generate_new_private_key();
		let url1                         = "https://api.example.com/api/latest";
		let url2                         = "https://api.example.com/api/releases/2.3.4";
		let url3                         = "https://api.example.com/api/hashes/2.3.4";
		let payload                      = b"Test payload";
		let json1                        = json!({
			"version": s!("2.3.4"),
		}).to_string();
		let json2                        = json!({
			"version": s!("2.3.4"),
			"hash":    hex::encode(Sha256::digest(payload)),
		}).to_string();
		let (mock_response1, public_key) = create_mock_response(
			url1,
			StatusCode::OK,
			Some("application/json"),
			Some(json1.len()),
			Ok(&json1),
			&ResponseSignature::GenerateUsing(private_key.clone()),
		);
		let mock_response2 = create_mock_binary_response(
			url2,
			StatusCode::OK,
			Some("application/octet-stream"),
			Some(payload.len()),
			Ok(payload),
		);
		let (mock_response3, _public_key) = create_mock_response(
			url3,
			StatusCode::OK,
			Some("application/json"),
			Some(json2.len()),
			Ok(&json2),
			&ResponseSignature::GenerateUsing(private_key.clone()),
		);
		let mock_client = create_mock_client(vec![
			(url1, Ok(mock_response1)),
			(url2, Ok(mock_response2)),
			(url3, Ok(mock_response3)),
		]);
		let updater = setup_safe_updater(
			Version::new(1, 0, 0),
			"https://api.example.com/api/",
			public_key,
			mock_client,
		);
		assert_eq!(updater.status(), Status::Idle);
		updater.check_for_updates().await;
		//	We haven't set up the test files, so the installation will fail, which
		//	is what we want here, so that we can check the status is correct
		assert_eq!(updater.status(), Status::Installing(version.clone()));
	}
	#[tokio::test]
	async fn check_for_updates__restart_blocked() {
		//	The lock and temp_dir need to be maintained for the duration of the test
		let (_lock, _temp_dir, _, _, _)  = setup_files();
		let version                      = Version::new(2, 3, 4);
		let private_key                  = generate_new_private_key();
		let url1                         = "https://api.example.com/api/latest";
		let url2                         = "https://api.example.com/api/releases/2.3.4";
		let url3                         = "https://api.example.com/api/hashes/2.3.4";
		let payload                      = b"Test payload";
		let json1                        = json!({
			"version": s!("2.3.4"),
		}).to_string();
		let json2                        = json!({
			"version": s!("2.3.4"),
			"hash":    hex::encode(Sha256::digest(payload)),
		}).to_string();
		let (mock_response1, public_key) = create_mock_response(
			url1,
			StatusCode::OK,
			Some("application/json"),
			Some(json1.len()),
			Ok(&json1),
			&ResponseSignature::GenerateUsing(private_key.clone()),
		);
		let mock_response2 = create_mock_binary_response(
			url2,
			StatusCode::OK,
			Some("application/octet-stream"),
			Some(payload.len()),
			Ok(payload),
		);
		let (mock_response3, _public_key) = create_mock_response(
			url3,
			StatusCode::OK,
			Some("application/json"),
			Some(json2.len()),
			Ok(&json2),
			&ResponseSignature::GenerateUsing(private_key.clone()),
		);
		let mock_client = create_mock_client(vec![
			(url1, Ok(mock_response1)),
			(url2, Ok(mock_response2)),
			(url3, Ok(mock_response3)),
		]);
		let updater = setup_safe_updater(
			Version::new(1, 0, 0),
			"https://api.example.com/api/",
			public_key,
			mock_client,
		);
		assert_eq!(updater.status(),          Status::Idle);
		assert_eq!(updater.register_action(), Some(1));
		updater.check_for_updates().await;
		//	We've registered a critical action, so the installation will be blocked,
		//	which is what we want here, so that we can check the status is correct
		assert_eq!(updater.status(),          Status::PendingRestart(version.clone()));
	}
	#[tokio::test]
	async fn check_for_updates__restart_failed() {
		//	No test for this at present, as it is difficult to simulate a failure
	}
	
	//		download_update														
	#[tokio::test]
	async fn download_update() {
		let version       = Version::new(2, 3, 4);
		let url           = "https://api.example.com/api/releases/2.3.4";
		let payload       = b"Test payload";
		let mock_response = create_mock_binary_response(
			url,
			StatusCode::OK,
			Some("application/octet-stream"),
			Some(payload.len()),
			Ok(payload),
		);
		let mock_client = create_mock_client(vec![
			(url, Ok(mock_response)),
		]);
		let updater = setup_safe_updater(
			Version::new(1, 0, 0),
			"https://api.example.com/api/",
			*EMPTY_PUBLIC_KEY,
			mock_client,
		);
		let (_download_dir, update_path, file_hash) = updater.download_update(&version).await.unwrap();
		let file_data                               = async_fs::read(update_path).await.unwrap();
		assert_eq!(file_hash, Sha256Hash::from(Sha256::digest(payload)));
		assert_eq!(file_hash, Sha256Hash::from(Sha256::digest(&file_data)));
		assert_eq!(file_data, payload);
	}
	#[tokio::test]
	async fn download_update__err_unable_to_create_download() {
		//	No test for this at present, as it is difficult to simulate a failure
	}
	#[tokio::test]
	async fn download_update__err_unable_to_create_tempdir() {
		//	No test for this at present, as it is difficult to simulate a failure
	}
	#[tokio::test]
	async fn download_update__err_unable_to_write_to_download() {
		//	No test for this at present, as it is difficult to simulate a failure
	}
	#[tokio::test]
	async fn download_update__err_unexpected_content_type() {
		let version               = Version::new(2, 3, 4);
		let url                   = "https://api.example.com/api/releases/2.3.4";
		let content_type          = "text/plain";
		let expected_content_type = s!("application/octet-stream");
		let payload               = b"Test payload";
		let mock_response         = create_mock_binary_response(
			url,
			StatusCode::OK,
			Some(content_type),
			Some(payload.len()),
			Ok(payload),
		);
		let mock_client = create_mock_client(vec![
			(url, Ok(mock_response)),
		]);
		let updater = setup_safe_updater(
			Version::new(1, 0, 0),
			"https://api.example.com/api/",
			*EMPTY_PUBLIC_KEY,
			mock_client,
		);
		let err = updater.download_update(&version).await.unwrap_err();
		assert_eq!(err,             UpdaterError::UnexpectedContentType(url.parse().unwrap(), content_type.to_owned(), expected_content_type.clone()));
		assert_eq!(err.to_string(), format!(r#"HTTP response from {url} had unexpected content type: "{content_type}", expected: "{expected_content_type}""#));
	}
	#[tokio::test]
	async fn download_update__err_missing_data() {
		let version               = Version::new(2, 3, 4);
		let url                   = "https://api.example.com/api/releases/2.3.4";
		let content_type          = "application/octet-stream";
		let payload               = b"Test payload";
		let content_len           = payload.len();
		let expected_content_len  = payload.len() + 1;
		let mock_response         = create_mock_binary_response(
			url,
			StatusCode::OK,
			Some(content_type),
			Some(expected_content_len),
			Ok(payload),
		);
		let mock_client = create_mock_client(vec![
			(url, Ok(mock_response)),
		]);
		let updater = setup_safe_updater(
			Version::new(1, 0, 0),
			"https://api.example.com/api/",
			*EMPTY_PUBLIC_KEY,
			mock_client,
		);
		let err = updater.download_update(&version).await.unwrap_err();
		assert_eq!(err,             UpdaterError::MissingData(url.parse().unwrap(), content_len, expected_content_len));
		assert_eq!(err.to_string(), format!("HTTP response body from {url} is shorter than expected: {content_len} < {expected_content_len}"));
	}
	#[tokio::test]
	async fn download_update__err_too_much_data() {
		let version               = Version::new(2, 3, 4);
		let url                   = "https://api.example.com/api/releases/2.3.4";
		let content_type          = "application/octet-stream";
		let payload               = b"Test payload";
		let content_len           = payload.len();
		let expected_content_len  = payload.len() - 1;
		let mock_response         = create_mock_binary_response(
			url,
			StatusCode::OK,
			Some(content_type),
			Some(expected_content_len),
			Ok(payload),
		);
		let mock_client = create_mock_client(vec![
			(url, Ok(mock_response)),
		]);
		let updater = setup_safe_updater(
			Version::new(1, 0, 0),
			"https://api.example.com/api/",
			*EMPTY_PUBLIC_KEY,
			mock_client,
		);
		let err = updater.download_update(&version).await.unwrap_err();
		assert_eq!(err,             UpdaterError::TooMuchData(url.parse().unwrap(), content_len, expected_content_len));
		assert_eq!(err.to_string(), format!("HTTP response body from {url} is longer than expected: {content_len} > {expected_content_len}"));
	}
	
	//		verify_update														
	#[tokio::test]
	async fn verify_update() {
		let version                     = Version::new(2, 3, 4);
		let hash                        = Sha256::digest(b"Test payload");
		let url                         = "https://api.example.com/api/hashes/2.3.4";
		let json                        = json!({
			"version": version.to_string(),
			"hash":    hex::encode(hash),
		}).to_string();
		let (mock_response, public_key) = create_mock_response(
			url,
			StatusCode::OK,
			Some("application/json"),
			Some(json.len()),
			Ok(&json),
			&ResponseSignature::Generate,
		);
		let mock_client = create_mock_client(vec![
			(url, Ok(mock_response)),
		]);
		let updater = setup_safe_updater(
			Version::new(1, 0, 0),
			"https://api.example.com/api/",
			public_key,
			mock_client,
		);
		assert_ok!(updater.verify_update(&version, hash.as_slice().try_into().unwrap()).await);
	}
	#[tokio::test]
	async fn verify_update__err_failed_hash_verification() {
		let version                     = Version::new(2, 3, 4);
		let hash                        = Sha256::digest(b"Test payload");
		let other_hash                  = Sha256::digest(b"Some other payload");
		let url                         = "https://api.example.com/api/hashes/2.3.4";
		let json                        = json!({
			"version": version.to_string(),
			"hash":    hex::encode(other_hash),
		}).to_string();
		let (mock_response, public_key) = create_mock_response(
			url,
			StatusCode::OK,
			Some("application/json"),
			Some(json.len()),
			Ok(&json),
			&ResponseSignature::Generate,
		);
		let mock_client = create_mock_client(vec![
			(url, Ok(mock_response)),
		]);
		let updater = setup_safe_updater(
			Version::new(1, 0, 0),
			"https://api.example.com/api/",
			public_key,
			mock_client,
		);
		let err = updater.verify_update(&version, hash.as_slice().try_into().unwrap()).await;
		assert_err_eq!(err.clone(), UpdaterError::FailedHashVerification(version.clone()));
		assert_eq!(err.unwrap_err().to_string(), format!("Failed hash verification for downloaded version {version}"));
	}
	#[tokio::test]
	async fn verify_update__err_invalid_payload() {
		let version                     = Version::new(2, 3, 4);
		let other_version               = Version::new(3, 3, 3);
		let hash                        = Sha256::digest(b"Test payload");
		let url                         = "https://api.example.com/api/hashes/2.3.4";
		let json                        = json!({
			"version": other_version.to_string(),
			"hash":    hex::encode(hash),
		}).to_string();
		let (mock_response, public_key) = create_mock_response(
			url,
			StatusCode::OK,
			Some("application/json"),
			Some(json.len()),
			Ok(&json),
			&ResponseSignature::Generate,
		);
		let mock_client = create_mock_client(vec![
			(url, Ok(mock_response)),
		]);
		let updater = setup_safe_updater(
			Version::new(1, 0, 0),
			"https://api.example.com/api/",
			public_key,
			mock_client,
		);
		let err = updater.verify_update(&version, hash.as_slice().try_into().unwrap()).await;
		assert_err_eq!(err.clone(), UpdaterError::InvalidPayload(url.parse().unwrap()));
		assert_eq!(err.unwrap_err().to_string(), format!("Invalid payload received from {url}"));
	}
	
	//		request																
	#[tokio::test]
	async fn request() {
		let url                         = "https://api.example.com/api/latest";
		let json                        = json!({
			"version": s!("3.3.3"),
		}).to_string();
		let (mock_response, public_key) = create_mock_response(
			url,
			StatusCode::OK,
			Some("application/json"),
			Some(json.len()),
			Ok(&json),
			&ResponseSignature::Generate,
		);
		let mock_client = create_mock_client(vec![
			(url, Ok(mock_response)),
		]);
		let updater = setup_safe_updater(
			Version::new(1, 0, 0),
			"https://api.example.com/api/",
			public_key,
			mock_client,
		);
		let (url2, response)   = updater.request("latest").await.unwrap();
		let parsed  = serde_json::from_str::<JsonValue>(&response.text().await.unwrap()).unwrap();
		let crafted = json!({
			"version": s!("3.3.3"),
		});
		assert_eq!(url2.as_str(), url);
		assert_json_eq!(parsed, crafted);
	}
	#[tokio::test]
	async fn request__err_http_error() {
		let url                         = "https://api.example.com/api/latest";
		let status                      = StatusCode::IM_A_TEAPOT;
		let json                        = json!({
			"version": s!("3.3.3"),
		}).to_string();
		let (mock_response, public_key) = create_mock_response(
			url,
			status,
			Some("application/json"),
			Some(json.len()),
			Ok(&json),
			&ResponseSignature::Generate,
		);
		let mock_client = create_mock_client(vec![
			(url, Ok(mock_response)),
		]);
		let updater = setup_safe_updater(
			Version::new(1, 0, 0),
			"https://api.example.com/api/",
			public_key,
			mock_client,
		);
		let err = updater.request("latest").await;
		assert_err_eq!(err.clone(), UpdaterError::HttpError(url.parse().unwrap(), status));
		assert_eq!(err.unwrap_err().to_string(), format!("HTTP status code {status} received when calling {url}"));
	}
	#[tokio::test]
	async fn request__err_http_request_failed() {
		let url         = "https://api.example.com/api/latest";
		let err_msg     = "Mocked Reqwest error";
		let mock_client = create_mock_client(vec![
			(url, Err(MockError::default())),
		]);
		let updater = setup_safe_updater(
			Version::new(1, 0, 0),
			"https://api.example.com/api/",
			*EMPTY_PUBLIC_KEY,
			mock_client,
		);
		let err = updater.request("latest").await;
		assert_err_eq!(err.clone(), UpdaterError::HttpRequestFailed(url.parse().unwrap(), err_msg.to_owned()));
		assert_eq!(err.unwrap_err().to_string(), format!("HTTP request to {url} failed: {err_msg}"));
	}
	#[tokio::test]
	async fn request__err_invalid_url() {
		let base     = "https://api.example.com/api";
		let endpoint = "https://[invalid]/../../../endpoint";
		let updater  = setup_safe_updater(
			Version::new(1, 0, 0),
			base,
			*EMPTY_PUBLIC_KEY,
			MockClient::new(),
		);
		let err = updater.request(endpoint).await;
		assert_err_eq!(err.clone(), UpdaterError::InvalidUrl(base.parse().unwrap(), endpoint.to_owned()));
		assert_eq!(err.unwrap_err().to_string(), format!("Invalid URL specified: {base} plus {endpoint}"));
	}
	
	//		decode_and_verify													
	#[tokio::test]
	async fn decode_and_verify__latest_version() {
		let version                     = Version::new(3, 3, 3);
		let url                         = "https://api.example.com/api/latest";
		let json                        = json!({
			"version": version.to_string(),
		}).to_string();
		let (mock_response, public_key) = create_mock_response(
			url,
			StatusCode::OK,
			Some("application/json"),
			Some(json.len()),
			Ok(&json),
			&ResponseSignature::Generate,
		);
		let updater = setup_safe_updater(
			Version::new(1, 0, 0),
			"https://api.example.com/api/",
			public_key,
			MockClient::new(),
		);
		let response = updater.decode_and_verify::<LatestVersionResponse>(url.parse().unwrap(), mock_response).await.unwrap();
		assert_eq!(response.version, version);
	}
	#[tokio::test]
	async fn decode_and_verify__version_hash() {
		let version                     = Version::new(3, 3, 3);
		let hash                        = hex::encode(Sha256::digest(b"Test payload"));
		let url                         = "https://api.example.com/api/hashes/3.3.3";
		let json                        = json!({
			"version": version.to_string(),
			"hash":    hash,
		}).to_string();
		let (mock_response, public_key) = create_mock_response(
			url,
			StatusCode::OK,
			Some("application/json"),
			Some(json.len()),
			Ok(&json),
			&ResponseSignature::Generate,
		);
		let updater = setup_safe_updater(
			Version::new(1, 0, 0),
			"https://api.example.com/api/",
			public_key,
			MockClient::new(),
		);
		let response = updater.decode_and_verify::<VersionHashResponse>(url.parse().unwrap(), mock_response).await.unwrap();
		assert_eq!(response.version, version);
		assert_eq!(response.hash,    Sha256Hash::from_hex(&hash).unwrap());
	}
	#[tokio::test]
	async fn decode_and_verify__err_failed_signature_verification() {
		let url                          = "https://api.example.com/api/latest";
		let other_public_key             = generate_new_private_key().verifying_key();
		let json                         = json!({
			"version": s!("3.3.3"),
		}).to_string();
		let (mock_response, _public_key) = create_mock_response(
			url,
			StatusCode::OK,
			Some("application/json"),
			Some(json.len()),
			Ok(&json),
			&ResponseSignature::Generate,
		);
		let updater = setup_safe_updater(
			Version::new(1, 0, 0),
			"https://api.example.com/api/",
			other_public_key,
			MockClient::new(),
		);
		let err = updater.decode_and_verify::<LatestVersionResponse>(url.parse().unwrap(), mock_response).await;
		assert_err_eq!(err.clone(), UpdaterError::FailedSignatureVerification(url.parse().unwrap()));
		assert_eq!(err.unwrap_err().to_string(), format!("Failed signature verification for response from {url}"));
	}
	#[tokio::test]
	async fn decode_and_verify__err_invalid_body() {
		let url                         = "https://api.example.com/api/latest";
		let (mock_response, public_key) = create_mock_response(
			url,
			StatusCode::OK,
			Some("application/json"),
			None,
			Err(MockError::default()),
			&ResponseSignature::Use(s!("dummy signature")),
		);
		let updater = setup_safe_updater(
			Version::new(1, 0, 0),
			"https://api.example.com/api/",
			public_key,
			MockClient::new(),
		);
		let err = updater.decode_and_verify::<LatestVersionResponse>(url.parse().unwrap(), mock_response).await;
		assert_err_eq!(err.clone(), UpdaterError::InvalidBody(url.parse().unwrap()));
		assert_eq!(err.unwrap_err().to_string(), format!("Invalid HTTP body received from {url}"));
	}
	#[tokio::test]
	async fn decode_and_verify__err_invalid_payload() {
		let url                         = "https://api.example.com/api/latest";
		let json                        = s!("{invalid json: 3.3.3");
		let (mock_response, public_key) = create_mock_response(
			url,
			StatusCode::OK,
			Some("application/json"),
			Some(json.len()),
			Ok(&json),
			&ResponseSignature::Generate,
		);
		let updater = setup_safe_updater(
			Version::new(1, 0, 0),
			"https://api.example.com/api/",
			public_key,
			MockClient::new(),
		);
		let err = updater.decode_and_verify::<LatestVersionResponse>(url.parse().unwrap(), mock_response).await;
		assert_err_eq!(err.clone(), UpdaterError::InvalidPayload(url.parse().unwrap()));
		assert_eq!(err.unwrap_err().to_string(), format!("Invalid payload received from {url}"));
	}
	#[tokio::test]
	async fn decode_and_verify__err_invalid_signature() {
		let url                         = "https://api.example.com/api/latest";
		let signature                   = s!("invalid signature");
		let json                        = json!({
			"version": s!("3.3.3"),
		}).to_string();
		let (mock_response, public_key) = create_mock_response(
			url,
			StatusCode::OK,
			Some("application/json"),
			Some(json.len()),
			Ok(&json),
			&ResponseSignature::Use(signature.clone()),
		);
		let updater = setup_safe_updater(
			Version::new(1, 0, 0),
			"https://api.example.com/api/",
			public_key,
			MockClient::new(),
		);
		let err = updater.decode_and_verify::<LatestVersionResponse>(url.parse().unwrap(), mock_response).await;
		assert_err_eq!(err.clone(), UpdaterError::InvalidSignature(url.parse().unwrap(), signature.clone()));
		assert_eq!(err.unwrap_err().to_string(), format!(r#"Invalid signature header "{signature}" received from {url}"#));
	}
	#[tokio::test]
	async fn decode_and_verify__err_missing_signature() {
		let url                         = "https://api.example.com/api/latest";
		let json                        = json!({
			"version": s!("3.3.3"),
		}).to_string();
		let (mock_response, public_key) = create_mock_response(
			url,
			StatusCode::OK,
			Some("application/json"),
			Some(json.len()),
			Ok(&json),
			&ResponseSignature::Omit,
		);
		let updater = setup_safe_updater(
			Version::new(1, 0, 0),
			"https://api.example.com/api/",
			public_key,
			MockClient::new(),
		);
		let err = updater.decode_and_verify::<LatestVersionResponse>(url.parse().unwrap(), mock_response).await;
		assert_err_eq!(err.clone(), UpdaterError::MissingSignature(url.parse().unwrap()));
		assert_eq!(err.unwrap_err().to_string(), format!("HTTP response from {url} does not contain a signature header"));
	}
	#[tokio::test]
	async fn decode_and_verify__err_unexpected_content_type() {
		let url                         = "https://api.example.com/api/latest";
		let content_type                = "text/plain";
		let expected_content_type       = s!("application/json");
		let json                        = json!({
			"version": s!("3.3.3"),
		}).to_string();
		let (mock_response, public_key) = create_mock_response(
			url,
			StatusCode::OK,
			Some(content_type),
			Some(json.len()),
			Ok(&json),
			&ResponseSignature::Generate,
		);
		let updater = setup_safe_updater(
			Version::new(1, 0, 0),
			"https://api.example.com/api/",
			public_key,
			MockClient::new(),
		);
		let err = updater.decode_and_verify::<LatestVersionResponse>(url.parse().unwrap(), mock_response).await;
		assert_err_eq!(err.clone(), UpdaterError::UnexpectedContentType(url.parse().unwrap(), content_type.to_owned(), expected_content_type.clone()));
		assert_eq!(err.unwrap_err().to_string(), format!(r#"HTTP response from {url} had unexpected content type: "{content_type}", expected: "{expected_content_type}""#));
	}
	#[tokio::test]
	async fn decode_and_verify__err_missing_data() {
		let url                         = "https://api.example.com/api/latest";
		let content_type                = "application/json";
		let json                        = json!({
			"version": s!("3.3.3"),
		}).to_string();
		let content_len                 = json.len();
		let expected_content_len        = json.len() + 1;
		let (mock_response, public_key) = create_mock_response(
			url,
			StatusCode::OK,
			Some(content_type),
			Some(expected_content_len),
			Ok(&json),
			&ResponseSignature::Generate,
		);
		let updater = setup_safe_updater(
			Version::new(1, 0, 0),
			"https://api.example.com/api/",
			public_key,
			MockClient::new(),
		);
		let err = updater.decode_and_verify::<LatestVersionResponse>(url.parse().unwrap(), mock_response).await;
		assert_err_eq!(err.clone(), UpdaterError::MissingData(url.parse().unwrap(), content_len, expected_content_len));
		assert_eq!(err.unwrap_err().to_string(), format!("HTTP response body from {url} is shorter than expected: {content_len} < {expected_content_len}"));
	}
	#[tokio::test]
	async fn decode_and_verify__err_too_much_data() {
		let url                         = "https://api.example.com/api/latest";
		let content_type                = "application/json";
		let json                        = json!({
			"version": s!("3.3.3"),
		}).to_string();
		let content_len                 = json.len();
		let expected_content_len        = json.len() - 1;
		let (mock_response, public_key) = create_mock_response(
			url,
			StatusCode::OK,
			Some(content_type),
			Some(expected_content_len),
			Ok(&json),
			&ResponseSignature::Generate,
		);
		let updater = setup_safe_updater(
			Version::new(1, 0, 0),
			"https://api.example.com/api/",
			public_key,
			MockClient::new(),
		);
		let err = updater.decode_and_verify::<LatestVersionResponse>(url.parse().unwrap(), mock_response).await;
		assert_err_eq!(err.clone(), UpdaterError::TooMuchData(url.parse().unwrap(), content_len, expected_content_len));
		assert_eq!(err.unwrap_err().to_string(), format!("HTTP response body from {url} is longer than expected: {content_len} > {expected_content_len}"));
	}
	
	//		replace_executable													
	#[tokio::test]
	async fn replace_executable() {
		//	The lock and temp_dir need to be maintained for the duration of the test
		let (_lock, _temp_dir, exe_path, old_path, new_path) = setup_files();
		let updater = setup_safe_updater(
			Version::new(1, 0, 0),
			"https://api.example.com/api/",
			*EMPTY_PUBLIC_KEY,
			MockClient::new(),
		);
		assert_ok!(updater.replace_executable(&new_path).await);
		assert!(exe_path.exists());
		assert!(old_path.exists());
		assert!(!new_path.exists());
		assert_eq!(fs::metadata(&exe_path).unwrap().permissions().mode() & 0o111, 0o111);
		assert_eq!(fs::read_to_string(old_path).unwrap(), "mock_exe contents");
		assert_eq!(fs::read_to_string(exe_path).unwrap(), "update contents");
	}
	#[tokio::test]
	async fn replace_executable__err_unable_to_get_file_metadata() {
		//	No test for this at present, as it is difficult to simulate a failure.
		//	It's also quite unlikely to occur.
	}
	#[tokio::test]
	async fn replace_executable__err_unable_to_move_new_exe() {
		//	The lock and temp_dir need to be maintained for the duration of the test
		let (_lock, _temp_dir, exe_path, old_path, new_path) = setup_files();
		let updater = setup_safe_updater(
			Version::new(1, 0, 0),
			"https://api.example.com/api/",
			*EMPTY_PUBLIC_KEY,
			MockClient::new(),
		);
		fs::remove_file(&new_path).unwrap();
		let err = updater.replace_executable(&new_path).await;
		assert_err_eq!(err.clone(), UpdaterError::UnableToMoveNewExe(new_path.clone(), s!("No such file or directory (os error 2)")));
		assert_eq!(err.unwrap_err().to_string(), format!(r"Unable to move the new executable {new_path:?}: No such file or directory (os error 2)"));
		assert!(!exe_path.exists());
		assert!(old_path.exists());
		assert!(!new_path.exists());
	}
	#[tokio::test]
	async fn replace_executable__err_unable_to_rename_current_exe() {
		//	The lock and temp_dir need to be maintained for the duration of the test
		let (_lock, _temp_dir, exe_path, old_path, new_path) = setup_files();
		let updater = setup_safe_updater(
			Version::new(1, 0, 0),
			"https://api.example.com/api/",
			*EMPTY_PUBLIC_KEY,
			MockClient::new(),
		);
		fs::remove_file(&exe_path).unwrap();
		let err = updater.replace_executable(&new_path).await;
		assert_err_eq!(err.clone(), UpdaterError::UnableToRenameCurrentExe(exe_path.clone(), s!("No such file or directory (os error 2)")));
		assert_eq!(err.unwrap_err().to_string(), format!(r"Unable to rename the current executable {exe_path:?}: No such file or directory (os error 2)"));
		assert!(!exe_path.exists());
		assert!(!old_path.exists());
		assert!(new_path.exists());
	}
	#[tokio::test]
	async fn replace_executable__err_unable_to_set_file_permissions() {
		//	No test for this at present, as it is difficult to simulate a failure.
		//	It's also quite unlikely to occur.
	}
	
	//		restart																
	#[tokio::test]
	async fn restart() {
		//	The lock needs to be maintained for the duration of the test. We call
		//	setup_files() to ensure that the mock executable path is set.
		let (_lock, _temp_dir, _, _, _) = setup_files();
		let updater = setup_safe_updater(
			Version::new(1, 0, 0),
			"https://api.example.com/api/",
			*EMPTY_PUBLIC_KEY,
			MockClient::new(),
		);
		//	The code being tested will call FakeCommand::new(), which will return a
		//	wrapper around a MockCommand that is already set up with the necessary
		//	expectations.
		updater.restart();
	}
}

//		Functions																
#[cfg(test)]
mod functions {
	use std::collections::HashMap;
	use super::*;
	
	//		get_header															
	#[test]
	fn get_header__string() {
		let mock_response = create_sham_response(
			"http://127.0.0.1",
			StatusCode::OK,
			Some("text/plain"),
			Some("Test body".len()),
			HashMap::<String, String>::new(),
			Ok("Test body".as_ref()),
		);
		let content_type: String = get_header(&mock_response, CONTENT_TYPE);
		assert_eq!(content_type, s!("text/plain"));
	}
	#[test]
	fn get_header__integer() {
		let mock_response = create_sham_response(
			"http://127.0.0.1",
			StatusCode::OK,
			Some("text/plain"),
			Some(1234),
			HashMap::<String, String>::new(),
			Ok("Test body".as_ref()),
		);
		let content_length: usize = get_header(&mock_response, CONTENT_LENGTH);
		assert_eq!(content_length, 1234);
	}
}


