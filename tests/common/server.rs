//! Common shared server functionality for tests and examples.

//		Packages

use crate::common::utils::generate_new_private_key;
use axum::{
	Extension,
	Router,
	http::HeaderMap,
	routing::get,
};
use bytes::Bytes;
use core::{
	net::{IpAddr, SocketAddr},
	time::Duration,
};
use patchify::server::{
	Axum as Patchify,
	Config as PatchifyConfig,
	Core as PatchifyCore,
};
use rubedo::{
	crypto::{Sha256Hash, SigningKey},
	std::ByteSized as _,
};
use semver::Version;
use sha2::{Sha256, Digest as _};
use std::{
	collections::HashMap,
	fs::File,
	io::{Write as _, stdout},
	path::PathBuf,
	sync::{Arc, Once, OnceLock},
};
use tempfile::{tempdir, TempDir};
use tokio::net::TcpListener;
use tokio::spawn;
use tower_http::{
	LatencyUnit,
	classify::ServerErrorsFailureClass,
	trace::{DefaultOnRequest, DefaultOnResponse, TraceLayer},
};
use tracing::{Level, Span, debug, error};
use tracing_subscriber::{
	EnvFilter,
	fmt::{format::FmtSpan, layer, writer::MakeWriterExt as _},
	layer::SubscriberExt as _,
	registry,
	util::SubscriberInitExt as _,
};



//		Constants

/// A list of available versions with their sizes and data.
pub const VERSION_DATA: [(Version, usize, &[u8]); 5] = [
	(Version::new(1, 0, 0),           1, b"foo"),
	(Version::new(0, 1, 0),           1, b"bar"),
	(Version::new(0, 0, 1),           1, b"foobarbaz"),
	(Version::new(1, 1, 0),         512, &[0x00, 0x01, 0x23, 0x45, 0x67, 0x89, 0x1A, 0xBC, 0xDE, 0xFF]),  //  5KB binary string
	(Version::new(0, 2, 0), 0x0008_0000, &[0x00, 0x01, 0x23, 0x45, 0x67, 0x89, 0x1A, 0xBC, 0xDE, 0xFF]),  //  5MB binary string
];



//		Statics

/// A global initialization lock.
pub static INIT: Once                 = Once::new();

/// A global signing key.
pub static KEY:  OnceLock<SigningKey> = OnceLock::new();



//		Functions

//		initialize																
/// Initializes the global logger and signing key.
pub fn initialize() {
	INIT.call_once(|| {
		registry()
			.with(
				EnvFilter::new("server=debug,reqwest=debug,tower_http=debug")
			)
			.with(
				layer()
					.with_writer(stdout.with_max_level(Level::INFO))
					.with_span_events(FmtSpan::NONE)
					.with_target(false)
			)
			.init()
		;
		KEY.set(generate_new_private_key()).unwrap();
	});
}

//		create_basic_server														
/// Creates a basic server with the provided routes.
/// 
/// # Parameters
/// 
/// * `address` - The address to bind the server to.
/// * `routes`  - The routes to use for the server.
/// 
pub async fn create_basic_server(
	address: SocketAddr,
	routes:  Router,
) -> SocketAddr {
	let app = routes
		.layer(TraceLayer::new_for_http()
			.on_request(
				DefaultOnRequest::new()
					.level(Level::INFO)
			)
			.on_response(
				DefaultOnResponse::new()
					.level(Level::INFO)
					.latency_unit(LatencyUnit::Micros)
			)
			.on_body_chunk(|chunk: &Bytes, _latency: Duration, _span: &Span| {
				debug!("Sending {} bytes", chunk.len());
			})
			.on_eos(|_trailers: Option<&HeaderMap>, stream_duration: Duration, _span: &Span| {
				debug!("Stream closed after {:?}", stream_duration);
			})
			.on_failure(|_error: ServerErrorsFailureClass, _latency: Duration, _span: &Span| {
				error!("Something went wrong");
			})
		)
	;
	let listener          = TcpListener::bind(address).await.expect("Failed to bind to address");
	let allocated_address = listener.local_addr().expect("Failed to get local address");
	drop(spawn(async move { axum::serve(listener, app).await.expect("Failed to serve") }));
	allocated_address
}

//		create_patchify_api_server												
/// Creates a Patchify API server with the provided configuration.
/// 
/// # Parameters
/// 
/// * `appname`  - The name of the application.
/// * `address`  - The address to bind the server to.
/// * `routes`   - The routes to use for the server.
/// * `releases` - The path to the releases directory.
/// * `versions` - A map of versions to their SHA-256 hashes.
/// 
pub async fn create_patchify_api_server(
	appname:  &str,
	address:  SocketAddr,
	routes:   Router,
	releases: PathBuf,
	versions: HashMap<Version, Sha256Hash>,
) -> SocketAddr {
	println!("Verifying release hashes... this could take a while");
	let patchify = PatchifyCore::new(PatchifyConfig {
		appname:          appname.to_owned(),
		key:              KEY.get().unwrap().clone(),
		releases,
		stream_threshold: 1000,
		stream_buffer:    256,
		read_buffer:      128,
		versions,
	}).unwrap();
	let allocated_address = create_basic_server(
		address,
		routes.layer(Extension(Arc::new(patchify))),
	).await;
	println!("Listening on: {allocated_address}");
	println!("App name:     {appname}");
	println!("Public key:   {}", KEY.get().unwrap().verifying_key().to_hex());
	allocated_address
}

//		create_test_server														
/// Creates a test server with the provided versions.
pub async fn create_test_server() -> (SocketAddr, TempDir) {
	let releases_dir = tempdir().unwrap();
	let address      = create_patchify_api_server(
		"test",
		SocketAddr::from((IpAddr::from([127, 0, 0, 1]), 0)),
		patchify_api_routes(),
		releases_dir.path().to_path_buf(),
		#[expect(clippy::pattern_type_mismatch, reason = "Not resolvable")]
		VERSION_DATA.iter()
			.map(|(version, repetitions, data)| {
				let path     = releases_dir.path().join(format!("test-{version}"));
				let mut file = File::create(&path).unwrap();
				file.write_all(&data.repeat(*repetitions)).unwrap();
				(version.clone(), Sha256::digest(data.repeat(*repetitions)).into())
			})
			.collect()
		,
	).await;
	(address, releases_dir)
}

//		patchify_api_routes														
/// Creates the Patchify API routes.
pub fn patchify_api_routes() -> Router {
	Router::new()
		.route("/api/ping",              get(get_ping))
		.route("/api/latest",            get(Patchify::get_latest_version))
		.route("/api/hashes/:version",   get(Patchify::get_hash_for_version))
		.route("/api/releases/:version", get(Patchify::get_release_file))
}

//		get_ping																
/// A simple ping route.
pub async fn get_ping() {}


