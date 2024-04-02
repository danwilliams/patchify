# Changelog

[Keep a Changelog]:    https://keepachangelog.com/en/1.0.0/
[Semantic Versioning]: https://semver.org/spec/v2.0.0.html

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog][], and this project adheres to
[Semantic Versioning][].


## 0.1.1 (02 April 2024)

### Changed

  - Updated lint configuration for Rust 1.77
  - Updated crate dependencies


## 0.1.0 (16 March 2024)

### Added

  - Added `server` module
      - Added release file checking and serving
      - Added response signing
      - Added `Config` struct
      - Added `Core` struct
          - Added `Core::new()`
          - Added `Core::latest_version()`
          - Added `Core::release_file()`
          - Added `Core::versions()`
      - Added `Axum` struct
          - Added `Axum::get_latest_version()`
          - Added `Axum::get_hash_for_version()`
          - Added `Axum::get_release_file()`
          - Added `Axum::sign_response()`
  - Added `client` module
      - Added update checking, downloading, verifying, and installing
      - Added `Config` struct
      - Added `Updater` struct
          - Added `Updater::new()`
          - Added `Updater::deregister_action()`
          - Added `Updater::is_safe_to_update()`
          - Added `Updater::register_action()`
          - Added `Updater::set_status()`
          - Added `Updater::status()`
          - Added `Updater::subscribe()`
  - Added README documentation
  - Added examples
  - Added full unit, integration, and end-to-end tests


