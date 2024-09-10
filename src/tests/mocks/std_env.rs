//! This module mocks `std::env` in order to test the `Updater` struct.
//! 
//! The `Updater` struct is responsible for moving the application's executable
//! file and then restarting the application. In order to do that, it queries
//! its environment for information. This module mocks the critical parts of
//! `std::env` using `mockall`, in order to test the `Updater` struct by
//! controlling the environmental information that gets returned. This is
//! important because unit tests run via the Cargo test runner, and so the
//! information that would be obtained is about the test runner, which should
//! not be moved or restarted.
//! 
//! The approach taken is that the "real" code imports functionality from
//! `std::env` when running in non-test mode, but imports the mocked functions
//! when running in test mode. This is achieved by using conditional
//! compilation. The test code then configures the mocks to expect certain
//! requests and to return certain responses, and then runs the tests.
//! 

//		Packages

use core::cell::RefCell;
use parking_lot::ReentrantMutex;
use std::{
	io::Result as IoResult,
	path::PathBuf,
};



//		Statics

pub static MOCK_EXE: ReentrantMutex<RefCell<Option<PathBuf>>> = ReentrantMutex::new(RefCell::new(None));



//		Functions

//		mock_current_exe														
#[cfg_attr(    feature = "reasons",  allow(clippy::unnecessary_wraps, reason = "Needed for mock"))]
#[cfg_attr(not(feature = "reasons"), allow(clippy::unnecessary_wraps))]
pub fn mock_current_exe() -> IoResult<PathBuf> {
	Ok(MOCK_EXE.lock().borrow().as_ref().expect("Needs initialisation").clone())
}


