use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug)]
#[allow(non_snake_case)]
pub struct AurItem {
	pub Name: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct AurInfo {
	pub resultcount: u32,
	pub results: Vec<AurItem>,
	pub r#type: String,
	pub version: u32,
}
