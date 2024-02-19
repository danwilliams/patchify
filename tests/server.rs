#![allow(non_snake_case)]

//		Packages

use assert_json_diff::assert_json_eq;
use axum::{
	Extension,
	Router,
	Server,
	http::{HeaderMap, StatusCode, header::CONTENT_TYPE},
	routing::get,
};
use bytes::Bytes;
use ed25519_dalek::{Signature, SigningKey, VerifyingKey};
use hex;
use patchify::server::{
	Axum as Patchify,
	Config as PatchifyConfig,
	Core as PatchifyCore,
};
use reqwest::Client;
use rand::rngs::OsRng;
use rubedo::sugar::s;
use semver::Version;
use serde_json::{Value as JsonValue, json};
use sha2::{Sha256, Digest};
use std::{
	fs::{File, self},
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
use tracing::{Level, Span, debug, error, info};
use tracing_subscriber::{
	EnvFilter,
	fmt::{format::FmtSpan, layer, writer::MakeWriterExt},
	layer::SubscriberExt,
	registry,
	util::SubscriberInitExt,
};



//		Constants

const VERSION_DATA: [(Version, usize, &[u8]); 5] = [
	(Version::new(1, 0, 0),       1, b"foo"),
	(Version::new(0, 1, 0),       1, b"bar"),
	(Version::new(0, 0, 1),       1, b"foobarbaz"),
	(Version::new(1, 1, 0),     512, &[0x00, 0x01, 0x23, 0x45, 0x67, 0x89, 0x1A, 0xBC, 0xDE, 0xFF]),  //  5KB binary string
	(Version::new(0, 2, 0), 524_288, &[0x00, 0x01, 0x23, 0x45, 0x67, 0x89, 0x1A, 0xBC, 0xDE, 0xFF]),  //  5MB binary string
];



//		Statics

static INIT: Once                 = Once::new();
static KEY:  OnceLock<SigningKey> = OnceLock::new();



//		Structs

//		AppState																
struct AppState {
}



//		Functions

//		initialize																
fn initialize() {
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
async fn create_server() -> (SocketAddr, TempDir) {
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
	let address = spawn(async {
		let server  = Server::bind(&SocketAddr::from((IpAddr::from([127, 0, 0, 1]), 0))).serve(app.into_make_service());
		let address = server.local_addr();
		info!("Listening on {address}");
		spawn(server);
		address
	}).await.unwrap();
	(address, releases_dir)
}

//		get_ping																
async fn get_ping() {}

//		request																	
async fn request(url: String, public_key: Option<VerifyingKey>) -> (StatusCode, String, Option<bool>, Bytes) {
	let response     = Client::new().get(url).send().await.unwrap();
	let status       = response.status();
	let content_type = response.headers().get(CONTENT_TYPE) .and_then(|h| h.to_str().ok()).unwrap_or("").to_owned();
	let signature    = response.headers().get("x-signature").and_then(|h| h.to_str().ok()).unwrap_or("").to_owned();
	let body         = response.bytes().await.unwrap();
	let verified     = if public_key.is_none() || signature.is_empty() {
		None
	} else {
		let signature_bytes            = hex::decode(signature).unwrap();
		let signature_array: &[u8; 64] = signature_bytes.as_slice().try_into().unwrap();
		Some(public_key.unwrap().verify_strict(&body, &Signature::from_bytes(signature_array)).is_ok())
	};
	(status, content_type, verified, body)
}



//		Tests

#[cfg(test)]
mod endpoints {
	use super::*;
	
	//		get_ping															
	#[tokio::test]
	async fn get_ping() {
		initialize();
		let (address, _releases_dir) = create_server().await;
		let (status, _, _, body) = request(
			format!("http://{address}/api/ping"),
			None,
		).await;
		assert_eq!(status,        StatusCode::OK);
		assert_eq!(body.as_ref(), b"");
	}
	
	//		get_latest															
	#[tokio::test]
	async fn get_latest() {
		initialize();
		let (address, _releases_dir) = create_server().await;
		let (status, content_type, verified, body) = request(
			format!("http://{address}/api/latest"),
			Some(KEY.get().unwrap().verifying_key()),
		).await;
		let parsed:  JsonValue = serde_json::from_slice(&body).unwrap();
		let crafted: JsonValue = json!({
			"version": s!("1.1.0"),
		});
		assert_eq!(status,       StatusCode::OK);
		assert_eq!(content_type, "application/json");
		assert_eq!(verified,     Some(true));
		assert_json_eq!(parsed, crafted);
	}
	#[tokio::test]
	async fn get_latest__fail_signature_verification() {
		initialize();
		let mut csprng         = OsRng{};
		let other_public_key   = SigningKey::generate(&mut csprng).verifying_key();
		let (address, _releases_dir) = create_server().await;
		let (status, content_type, verified, body) = request(
			format!("http://{address}/api/latest"),
			Some(other_public_key),
		).await;
		let parsed:  JsonValue = serde_json::from_slice(&body).unwrap();
		let crafted: JsonValue = json!({
			"version": s!("1.1.0"),
		});
		assert_eq!(status,       StatusCode::OK);
		assert_eq!(content_type, "application/json");
		assert_eq!(verified,     Some(false));
		assert_json_eq!(parsed, crafted);
	}
	
	//		get_hashes_version													
	#[tokio::test]
	async fn get_hashes_version() {
		initialize();
		let (address, _releases_dir) = create_server().await;
		let (status, content_type, verified, body) = request(
			format!("http://{address}/api/hashes/0.2.0"),
			Some(KEY.get().unwrap().verifying_key()),
		).await;
		let parsed:  JsonValue = serde_json::from_slice(&body).unwrap();
		let crafted: JsonValue = json!({
			"version": s!("0.2.0"),
			"hash":    s!("45fb074c75cfae708144969a1df5b33d845c95475a5ed69a60736b9391aac73b"),
		});
		assert_eq!(status,       StatusCode::OK);
		assert_eq!(content_type, "application/json");
		assert_eq!(verified,     Some(true));
		assert_json_eq!(parsed, crafted);
	}
	#[tokio::test]
	async fn get_hashes_version__not_found() {
		initialize();
		let (address, _releases_dir) = create_server().await;
		let (status, content_type, verified, body) = request(
			format!("http://{address}/api/hashes/3.2.1"),
			Some(KEY.get().unwrap().verifying_key()),
		).await;
		assert_eq!(status,        StatusCode::NOT_FOUND);
		assert_eq!(content_type,  "text/plain; charset=utf-8");
		assert_eq!(verified,      None);
		assert_eq!(body.as_ref(), b"Version 3.2.1 not found");
	}
	#[tokio::test]
	async fn get_hashes_version__invalid() {
		initialize();
		let (address, _releases_dir) = create_server().await;
		let (status, content_type, verified, body) = request(
			format!("http://{address}/api/hashes/invalid"),
			Some(KEY.get().unwrap().verifying_key()),
		).await;
		assert_eq!(status,        StatusCode::BAD_REQUEST);
		assert_eq!(content_type,  "text/plain; charset=utf-8");
		assert_eq!(verified,      None);
		assert_eq!(body.as_ref(), b"Invalid URL: unexpected character 'i' while parsing major version number");
	}
	
	//		get_releases_version												
	#[tokio::test]
	async fn get_releases_version() {
		initialize();
		let (address, _releases_dir) = create_server().await;
		let (status, content_type, verified, body) = request(
			format!("http://{address}/api/releases/1.0.0"),
			Some(KEY.get().unwrap().verifying_key()),
		).await;
		assert_eq!(status,        StatusCode::OK);
		assert_eq!(content_type,  "application/octet-stream");
		assert_eq!(verified,      None);
		assert_eq!(body.as_ref(), b"foo");
	}
	#[tokio::test]
	async fn get_releases_version__medium_binary() {
		initialize();
		let (address, _releases_dir) = create_server().await;
		let (status, content_type, verified, body) = request(
			format!("http://{address}/api/releases/1.1.0"),
			Some(KEY.get().unwrap().verifying_key()),
		).await;
		assert_eq!(status,        StatusCode::OK);
		assert_eq!(content_type,  "application/octet-stream");
		assert_eq!(verified,      None);
		assert_eq!(body.as_ref(), vec![0x00, 0x01, 0x23, 0x45, 0x67, 0x89, 0x1A, 0xBC, 0xDE, 0xFF].repeat(512));
	}
	#[tokio::test]
	async fn get_releases_version__large_binary() {
		initialize();
		let (address, _releases_dir) = create_server().await;
		let (status, content_type, verified, body) = request(
			format!("http://{address}/api/releases/0.2.0"),
			Some(KEY.get().unwrap().verifying_key()),
		).await;
		assert_eq!(status,        StatusCode::OK);
		assert_eq!(content_type,  "application/octet-stream");
		assert_eq!(verified,      None);
		assert_eq!(body.as_ref(), vec![0x00, 0x01, 0x23, 0x45, 0x67, 0x89, 0x1A, 0xBC, 0xDE, 0xFF].repeat(524_288));
	}
	#[tokio::test]
	async fn get_releases_version__not_found() {
		initialize();
		let (address, _releases_dir) = create_server().await;
		let (status, content_type, verified, body) = request(
			format!("http://{address}/api/releases/4.5.6"),
			Some(KEY.get().unwrap().verifying_key()),
		).await;
		assert_eq!(status,        StatusCode::NOT_FOUND);
		assert_eq!(content_type,  "text/plain; charset=utf-8");
		assert_eq!(verified,      None);
		assert_eq!(body.as_ref(), b"Version 4.5.6 not found");
	}
	#[tokio::test]
	async fn get_releases_version__invalid() {
		initialize();
		let (address, _releases_dir) = create_server().await;
		let (status, content_type, verified, body) = request(
			format!("http://{address}/api/releases/invalid"),
			Some(KEY.get().unwrap().verifying_key()),
		).await;
		assert_eq!(status,        StatusCode::BAD_REQUEST);
		assert_eq!(content_type,  "text/plain; charset=utf-8");
		assert_eq!(verified,      None);
		assert_eq!(body.as_ref(), b"Invalid URL: unexpected character 'i' while parsing major version number");
	}
	#[tokio::test]
	async fn get_releases_version__missing() {
		initialize();
		let (address, releases_dir) = create_server().await;
		fs::remove_file(&releases_dir.path().join("test-1.0.0")).unwrap();
		let (status, content_type, verified, body) = request(
			format!("http://{address}/api/releases/1.0.0"),
			Some(KEY.get().unwrap().verifying_key()),
		).await;
		assert_eq!(status,        StatusCode::INTERNAL_SERVER_ERROR);
		assert_eq!(content_type,  "text/plain; charset=utf-8");
		assert_eq!(verified,      None);
		assert_eq!(body.as_ref(), b"Release file missing");
	}
}

#[cfg(test)]
mod scenarios {
	use super::*;
	
	//		download_and_verify_latest_release									
	#[tokio::test]
	async fn download_and_verify_latest_release() {
		initialize();
		let (address, _releases_dir) = create_server().await;
		let public_key = Some(KEY.get().unwrap().verifying_key());
		
		//		Get latest version												
		let (status, content_type, verified, body) = request(
			format!("http://{address}/api/latest"),
			public_key,
		).await;
		let json:   JsonValue = serde_json::from_slice(&body).unwrap();
		let latest: Version   = json["version"].as_str().unwrap().parse().unwrap();
		assert_eq!(status,       StatusCode::OK);
		assert_eq!(content_type, "application/json");
		assert_eq!(verified,     Some(true));
		assert_eq!(latest,       Version::new(1, 1, 0));
		
		//		Download release file											
		let (status, content_type, verified, body) = request(
			format!("http://{address}/api/releases/{latest}"),
			public_key,
		).await;
		let release_file = body.as_ref();
		assert_eq!(status,       StatusCode::OK);
		assert_eq!(content_type, "application/octet-stream");
		assert_eq!(verified,     None);
		
		//		Verify release file												
		let (status, content_type, verified, body) = request(
			format!("http://{address}/api/hashes/{latest}"),
			public_key,
		).await;
		let json:    JsonValue = serde_json::from_slice(&body).unwrap();
		let version: Version   = json["version"].as_str().unwrap().parse().unwrap();
		let hash:    String    = json["hash"].as_str().unwrap().to_owned();
		assert_eq!(status,       StatusCode::OK);
		assert_eq!(content_type, "application/json");
		assert_eq!(verified,     Some(true));
		assert_eq!(version,      latest);
		assert_eq!(hash,         hex::encode(Sha256::digest(release_file)));
	}
	
	//		download_and_verify_release_with_hash_fail							
	#[tokio::test]
	async fn download_and_verify_release_with_hash_fail() {
		initialize();
		let (address, releases_dir) = create_server().await;
		let public_key = Some(KEY.get().unwrap().verifying_key());
		let wanted     = Version::new(1, 0, 0);
		let mut file   = File::create(&releases_dir.path().join("test-1.0.0")).unwrap();
		write!(file, "invalid").unwrap();
		
		//		Download release file											
		let (status, content_type, verified, body) = request(
			format!("http://{address}/api/releases/{wanted}"),
			public_key,
		).await;
		let release_file = body.as_ref();
		assert_eq!(status,       StatusCode::OK);
		assert_eq!(content_type, "application/octet-stream");
		assert_eq!(verified,     None);
		assert_eq!(release_file, b"invalid");
		
		//		Verify release file												
		let (status, content_type, verified, body) = request(
			format!("http://{address}/api/hashes/{wanted}"),
			public_key,
		).await;
		let json:    JsonValue = serde_json::from_slice(&body).unwrap();
		let version: Version   = json["version"].as_str().unwrap().parse().unwrap();
		let hash:    String    = json["hash"].as_str().unwrap().to_owned();
		assert_eq!(status,       StatusCode::OK);
		assert_eq!(content_type, "application/json");
		assert_eq!(verified,     Some(true));
		assert_eq!(version,      wanted);
		assert_ne!(hash,         hex::encode(Sha256::digest(release_file)));
	}
}


