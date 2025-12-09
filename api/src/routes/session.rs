use serde::{Deserialize, Serialize};

use crate::routes::role::Role;

#[derive(Serialize, Deserialize)]
pub struct SessionAuth {
    pub user_id: i64,
    pub role: Role,
}
