//! This module mocks `std::process` in order to test the `Updater` struct.
//! 
//! The `Updater` struct restarts the application by using `Command`. This
//! module mocks the critical parts of `Command` using `mockall`, in order to
//! test the `Updater` struct without actually running any system commands. This
//! is important because unit tests should not actually restart the application.
//! 
//! Because the `Command` functions use a chaining pattern, there is also a
//! `FakeCommand` struct that is used as a wrapper. This sets up the actual mock
//! along with the expected sequence of calls, and then interacts with the
//! mocked functions, whilst returning itself for chaining.
//! 
//! Notably, the mock definitions are more restrictive than the "real" code. If
//! the standard library accepts group X, which includes types A, B, and C, the
//! mocks are reducing the acceptance to just type B only, the type actually
//! used by the application code. This is still compatible with the "real" code.
//! 
//! The approach taken is that the "real" code imports the `Command` from
//! `std::process` when running in non-test mode, but imports `FakeCommand` when
//! running in test mode. This is achieved by using conditional compilation. The
//! test code then configures the mocks to expect certain requests and to return
//! certain responses, and then runs the tests.
//! 

//		Packages

use crate::mocks::std_env::mock_current_exe;
use std::{
	env::args,
	io::Error as IoError,
	path::PathBuf,
};
use mockall::{Sequence, automock};



//		Traits

//§		Command																	
#[automock]
pub trait Command {
	//		args																
	fn args(&self, args: Vec<String>);
	
	//		exec																
	fn exec(&self) -> IoError;
	
	//		stdin																
	fn stdin(&self, cfg: MockStdio);
	
	//		stdout																
	fn stdout(&self, cfg: MockStdio);
	
	//		stderr																
	fn stderr(&self, cfg: MockStdio);
}



//		Structs

//		FakeCommand																
#[derive(Debug)]
pub struct FakeCommand {
	command: MockCommand,
}

//󰭅		FakeCommand																
impl FakeCommand {
	//		new																	
	pub fn new(program: &PathBuf) -> Self {
		assert_eq!(program, &mock_current_exe().unwrap(),
			"Command instance should be created with the current executable path"
		);
		let mut sequence     = Sequence::new();
		let mut mock_command = MockCommand::new();
		let _ = mock_command.expect_args()
			.withf(|list| list == &args().skip(1).collect::<Vec<_>>())
			.times(1)
			.in_sequence(&mut sequence)
			.returning(|_| ())
		;
		let _ = mock_command.expect_stdin()
			.times(1)
			.in_sequence(&mut sequence)
			.returning(|_| ())
		;
		let _ = mock_command.expect_stdout()
			.times(1)
			.in_sequence(&mut sequence)
			.returning(|_| ())
		;
		let _ = mock_command.expect_stderr()
			.times(1)
			.in_sequence(&mut sequence)
			.returning(|_| ())
		;
		let _ = mock_command.expect_exec()
			.times(1)
			.in_sequence(&mut sequence)
			.returning(|| IoError::from_raw_os_error(0))
		;
		Self {
			command: mock_command,
		}
	}
	
	//		args																
	pub fn args(&mut self, args: Vec<String>) -> &mut Self {
		self.command.args(args);
		self
	}
	
	//		exec																
	#[cfg_attr(    feature = "reasons",  allow(clippy::needless_pass_by_ref_mut, reason = "Needed for mock"))]
	#[cfg_attr(not(feature = "reasons"), allow(clippy::needless_pass_by_ref_mut))]
	pub fn exec(&mut self) -> IoError {
		self.command.exec()
	}
	
	//		stdin																
	pub fn stdin(&mut self, cfg: MockStdio) -> &mut Self {
		self.command.stdin(cfg);
		self
	}
	
	//		stdout																
	pub fn stdout(&mut self, cfg: MockStdio) -> &mut Self {
		self.command.stdout(cfg);
		self
	}
	
	//		stderr																
	pub fn stderr(&mut self, cfg: MockStdio) -> &mut Self {
		self.command.stderr(cfg);
		self
	}
}

//		MockStdio																
#[derive(Debug)]
pub struct MockStdio;

//󰭅		MockStdio																
#[cfg_attr(    feature = "reasons",  allow(clippy::missing_const_for_fn, reason = "Needed for mock"))]
#[cfg_attr(not(feature = "reasons"), allow(clippy::missing_const_for_fn))]
impl MockStdio {
	//		inherit																
	pub fn inherit() -> Self {
		Self
	}
}



//		Functions

//		mock_exit																
#[cfg_attr(    feature = "reasons",  allow(clippy::missing_const_for_fn, reason = "Needed for mock"))]
#[cfg_attr(not(feature = "reasons"), allow(clippy::missing_const_for_fn))]
pub fn mock_exit(_code: i32) {}


