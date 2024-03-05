//		Packages

use crate::common::utils::*;
use axum::{
	Extension,
	Router,
	Server,
	http::HeaderMap,
	routing::get,
};
use bytes::Bytes;
use ed25519_dalek::SigningKey;
use patchify::server::{
	Axum as Patchify,
	Config as PatchifyConfig,
	Core as PatchifyCore,
};
use rubedo::sugar::s;
use semver::Version;
use sha2::{Sha256, Digest};
use std::{
	collections::HashMap,
	fs::File,
	io::{Write, stdout},
	net::{IpAddr, SocketAddr},
	path::PathBuf,
	sync::{Arc, Once, OnceLock},
	time::Duration,
};
use tempfile::{tempdir, TempDir};
use tokio::spawn;
use tower_http::{
	LatencyUnit,
	classify::ServerErrorsFailureClass,
	trace::{DefaultOnRequest, DefaultOnResponse, TraceLayer},
};
use tracing::{Level, Span, debug, error};
use tracing_subscriber::{
	EnvFilter,
	fmt::{format::FmtSpan, layer, writer::MakeWriterExt},
	layer::SubscriberExt,
	registry,
	util::SubscriberInitExt,
};



//		Constants

pub const VERSION_DATA: [(Version, usize, &[u8]); 5] = [
	(Version::new(1, 0, 0),       1, b"foo"),
	(Version::new(0, 1, 0),       1, b"bar"),
	(Version::new(0, 0, 1),       1, b"foobarbaz"),
	(Version::new(1, 1, 0),     512, &[0x00, 0x01, 0x23, 0x45, 0x67, 0x89, 0x1A, 0xBC, 0xDE, 0xFF]),  //  5KB binary string
	(Version::new(0, 2, 0), 524_288, &[0x00, 0x01, 0x23, 0x45, 0x67, 0x89, 0x1A, 0xBC, 0xDE, 0xFF]),  //  5MB binary string
];



//		Statics

pub static INIT: Once                 = Once::new();
pub static KEY:  OnceLock<SigningKey> = OnceLock::new();



//		Functions

//		initialize																
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
				debug!("Sending {} bytes", chunk.len())
			})
			.on_eos(|_trailers: Option<&HeaderMap>, stream_duration: Duration, _span: &Span| {
				debug!("Stream closed after {:?}", stream_duration)
			})
			.on_failure(|_error: ServerErrorsFailureClass, _latency: Duration, _span: &Span| {
				error!("Something went wrong")
			})
		)
	;
	let server  = Server::bind(&address).serve(app.into_make_service());
	let address = server.local_addr();
	spawn(server);
	address
}

//		create_patchify_api_server												
pub async fn create_patchify_api_server(
	appname:  String,
	address:  SocketAddr,
	routes:   Router,
	releases: PathBuf,
	versions: HashMap<Version, [u8; 32]>,
) -> SocketAddr {
	println!("Verifying release hashes... this could take a while");
	let patchify = PatchifyCore::new(PatchifyConfig {
		appname:          appname.clone(),
		key:              KEY.get().unwrap().clone(),
		releases,
		stream_threshold: 1000,
		stream_buffer:    256,
		read_buffer:      128,
		versions,
	}).unwrap();
	let address = create_basic_server(
		address,
		routes.layer(Extension(Arc::new(patchify))),
	).await;
	println!("Listening on: {address}");
	println!("App name:     {appname}");
	println!("Public key:   {}", hex::encode(KEY.get().unwrap().verifying_key()));
	address
}

//		create_test_server														
pub async fn create_test_server() -> (SocketAddr, TempDir) {
	let releases_dir = tempdir().unwrap();
	let address      = create_patchify_api_server(
		s!("test"),
		SocketAddr::from((IpAddr::from([127, 0, 0, 1]), 0)),
		patchify_api_routes(),
		releases_dir.path().to_path_buf(),
		VERSION_DATA.iter()
			.map(|(version, repetitions, data)| {
				let path     = releases_dir.path().join(&format!("test-{}", version));
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
pub fn patchify_api_routes() -> Router {
	Router::new()
		.route("/api/ping",              get(get_ping))
		.route("/api/latest",            get(Patchify::get_latest_version))
		.route("/api/hashes/:version",   get(Patchify::get_hash_for_version))
		.route("/api/releases/:version", get(Patchify::get_release_file))
}

//		get_ping																
pub async fn get_ping() {}


