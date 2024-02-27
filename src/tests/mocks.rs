//		Modules

#[cfg(test)]
#[path = "mocks/reqwest.rs"]
pub(crate) mod reqwest;

#[cfg(test)]
#[path = "mocks/std_env.rs"]
pub(crate) mod std_env;



//		Packages

use crate::client::Status;
use mockall::automock;



//		Traits

//§		Subscriber																
#[automock]
pub(crate) trait Subscriber {
	//		update																
	fn update(&self, status: Status);
}


