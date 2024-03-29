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
pub(crate) trait Command {
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
pub(crate) struct FakeCommand {
	command: MockCommand,
}

//󰭅		FakeCommand																
impl FakeCommand {
	//		new																	
	pub(crate) fn new(program: PathBuf) -> Self {
		if program != mock_current_exe().unwrap() {
			panic!("Command instance should be created with the current executable path");
		}
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
	pub(crate) fn args(&mut self, args: Vec<String>) -> &mut Self {
		self.command.args(args);
		self
	}
	
	//		exec																
	pub(crate) fn exec(&mut self) -> IoError {
		self.command.exec()
	}
	
	//		stdin																
	pub(crate) fn stdin(&mut self, cfg: MockStdio) -> &mut Self {
		self.command.stdin(cfg);
		self
	}
	
	//		stdout																
	pub(crate) fn stdout(&mut self, cfg: MockStdio) -> &mut Self {
		self.command.stdout(cfg);
		self
	}
	
	//		stderr																
	pub(crate) fn stderr(&mut self, cfg: MockStdio) -> &mut Self {
		self.command.stderr(cfg);
		self
	}
}

//		MockStdio																
#[derive(Debug)]
pub(crate) struct MockStdio;

//󰭅		MockStdio																
impl MockStdio {
	//		inherit																
	pub(crate) fn inherit() -> Self {
		Self
	}
}



//		Functions

//		mock_exit																
pub(crate) fn mock_exit(_code: i32) {}


