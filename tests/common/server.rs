//		Packages

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
use rand::rngs::OsRng;
use rubedo::sugar::s;
use semver::Version;
use sha2::{Sha256, Digest};
use std::{
	fs::File,
	io::{Write, stdout},
	net::{IpAddr, SocketAddr},
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



//		Structs

//		AppState																
pub struct AppState {
}



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
		let mut csprng = OsRng{};
		KEY.set(SigningKey::generate(&mut csprng)).unwrap();
	});
}

//		create_server															
pub async fn create_server() -> (SocketAddr, TempDir) {
	let releases_dir = tempdir().unwrap();
	let patchify = PatchifyCore::new(PatchifyConfig {
		appname:  s!("test"),
		key:      KEY.get().unwrap().clone(),
		releases: releases_dir.path().to_path_buf(),
		versions: VERSION_DATA.iter()
			.map(|(version, repetitions, data)| {
				let path     = releases_dir.path().join(&format!("test-{}", version));
				let mut file = File::create(&path).unwrap();
				file.write_all(&data.repeat(*repetitions)).unwrap();
				(version.clone(), Sha256::digest(data.repeat(*repetitions)).into())
			})
			.collect()
		,
		stream_threshold: 1000,
		stream_buffer:    256,
		read_buffer:      128,
	}).unwrap();
	let app = Router::new()
		.route("/api/ping",              get(get_ping))
		.route("/api/latest",            get(Patchify::get_latest_version))
		.route("/api/hashes/:version",   get(Patchify::get_hash_for_version))
		.route("/api/releases/:version", get(Patchify::get_release_file))
		.with_state(Arc::new(AppState {
		}))
		.layer(Extension(Arc::new(patchify)))
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
	let server   = Server::bind(&SocketAddr::from((IpAddr::from([127, 0, 0, 1]), 0))).serve(app.into_make_service());
	let address  = server.local_addr();
	spawn(server);
	println!("Listening on {address}");
	(address, releases_dir)
}

//		get_ping																
async fn get_ping() {}


