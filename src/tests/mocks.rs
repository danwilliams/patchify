//		Modules

#[cfg(test)]
#[path = "mocks/reqwest.rs"]
pub(crate) mod reqwest;



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


