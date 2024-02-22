#![allow(non_snake_case)]

//		Packages

use super::*;



//		Tests

//		Updater																	
#[cfg(test)]
mod updater {
	use super::*;
	
	//		new																	
	#[tokio::test]
	async fn new() {
		let updater = Updater::new(Config {
			version:          Version::new(1, 0, 0),
			api:              "https://api.example.com".parse().unwrap(),
			key:              VerifyingKey::from_bytes(&[0; 32]).unwrap(),
			check_on_startup: false,
			check_interval:   Some(Duration::from_secs(60 * 60)),
		});
		assert_eq!(updater.config.version,          Version::new(1, 0, 0));
		assert_eq!(updater.config.api,              "https://api.example.com".parse().unwrap());
		assert_eq!(updater.config.key,              VerifyingKey::from_bytes(&[0; 32]).unwrap());
		assert_eq!(updater.config.check_on_startup, false);
		assert_eq!(updater.config.check_interval,   Some(Duration::from_secs(60 * 60)));
	}
}


