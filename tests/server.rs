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
use patchify::server::{
	Axum as Patchify,
	Config as PatchifyConfig,
	Core as PatchifyCore,
};
use reqwest::Client;
use rubedo::sugar::s;
use semver::Version;
use serde_json::{Value as JsonValue, json};
use sha2::{Sha256, Digest};
use std::{
	io::stdout,
	net::{IpAddr, SocketAddr},
	sync::{Arc, Once},
	time::Duration,
};
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



//		Statics

static INIT: Once = Once::new();



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
	});
}

//		create_server															
async fn create_server() -> SocketAddr {
	let version_data = vec![
		(Version::new(1, 0, 0), "foo"),
		(Version::new(0, 1, 0), "bar"),
		(Version::new(0, 0, 1), "baz"),
		(Version::new(1, 1, 0), "foobar"),
		(Version::new(0, 2, 0), "foobaz"),
	];
	let patchify = PatchifyCore::new(PatchifyConfig {
		appname:  s!("test"),
		versions: version_data.iter()
			.map(|(version, data)| (version.clone(), Sha256::digest(data).into()))
			.collect()
		,
	});
	let app = Router::new()
		.route("/api/ping",            get(get_ping))
		.route("/api/latest",          get(Patchify::get_latest_version))
		.route("/api/hashes/:version", get(Patchify::get_hash_for_version))
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
	let server  = Server::bind(&SocketAddr::from((IpAddr::from([127, 0, 0, 1]), 0))).serve(app.into_make_service());
	let address = server.local_addr();
	info!("Listening on {address}");
	spawn(server);
	address
}

//		get_ping																
async fn get_ping() {}

//		request																	
async fn request(path: String) -> (StatusCode, String, String) {
	let address      = spawn(async { create_server().await }).await.unwrap();
	let response     = Client::new().get(format!("http://{address}/{path}")).send().await.unwrap();
	let status       = response.status();
	let content_type = response.headers().get(CONTENT_TYPE).and_then(|h| h.to_str().ok()).unwrap_or("").to_owned();
	let body         = response.text().await.unwrap();
	(status, content_type, body)
}



//		Tests

#[cfg(test)]
mod endpoints {
	use super::*;
	
	//		get_ping															
	#[tokio::test]
	async fn get_ping() {
		initialize();
		let (status, _, body) = request(s!("api/ping")).await;
		assert_eq!(status, StatusCode::OK);
		assert_eq!(body,   "");
	}
	
	//		get_latest															
	#[tokio::test]
	async fn get_latest() {
		initialize();
		let (status, content_type, body) = request(s!("api/latest")).await;
		let parsed:  JsonValue = serde_json::from_str(&body).unwrap();
		let crafted: JsonValue = json!({
			"version": s!("1.1.0"),
		});
		assert_eq!(status,       StatusCode::OK);
		assert_eq!(content_type, "application/json");
		assert_json_eq!(parsed, crafted);
	}
	
	//		get_hashes_version													
	#[tokio::test]
	async fn get_hashes_version() {
		initialize();
		let (status, content_type, body) = request(s!("api/hashes/0.2.0")).await;
		let parsed:  JsonValue = serde_json::from_str(&body).unwrap();
		let crafted: JsonValue = json!({
			"version": s!("0.2.0"),
			"hash":    s!("798f012674b5b8dcab4b00114bdf6738a69a4cdcf7ca0db1149260c9f81b73f7"),
		});
		assert_eq!(status,       StatusCode::OK);
		assert_eq!(content_type, "application/json");
		assert_json_eq!(parsed, crafted);
	}
	#[tokio::test]
	async fn get_hashes_version__not_found() {
		initialize();
		let (status, content_type, body) = request(s!("api/hashes/3.2.1")).await;
		assert_eq!(status,       StatusCode::NOT_FOUND);
		assert_eq!(content_type, "text/plain; charset=utf-8");
		assert_eq!(body,         "Version 3.2.1 not found");
	}
	#[tokio::test]
	async fn get_hashes_version__invalid() {
		initialize();
		let (status, content_type, body) = request(s!("api/hashes/invalid")).await;
		assert_eq!(status,       StatusCode::BAD_REQUEST);
		assert_eq!(content_type, "text/plain; charset=utf-8");
		assert_eq!(body,         "Invalid URL: unexpected character 'i' while parsing major version number");
	}
}


