//		Modules

#[cfg(test)]
#[path = "mocks/reqwest.rs"]
pub mod reqwest;

#[cfg(test)]
#[path = "mocks/std_env.rs"]
pub mod std_env;

#[cfg(test)]
#[path = "mocks/std_process.rs"]
pub mod std_process;



//		Packages

use crate::client::Status;
use mockall::automock;



//		Traits

//§		Subscriber																
#[automock]
pub trait Subscriber {
	//		update																
	fn update(&self, status: Status);
}


