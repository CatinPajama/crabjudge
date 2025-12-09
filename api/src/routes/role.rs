use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, PartialEq)]
pub enum Role {
    User,
    ProblemSetter,
    Admin,
}

impl Role {
    fn rank(&self) -> u8 {
        match self {
            Self::User => 0,
            Self::ProblemSetter => 1,
            Self::Admin => 2,
        }
    }
}

impl PartialOrd for Role {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        self.rank().partial_cmp(&other.rank())
    }
}

impl From<Role> for &str {
    fn from(value: Role) -> Self {
        match value {
            Role::User => "user",
            Role::Admin => "admin",
            Role::ProblemSetter => "problemsetter",
        }
    }
}

impl TryFrom<&str> for Role {
    type Error = &'static str;
    fn try_from(value: &str) -> Result<Self, Self::Error> {
        match value {
            "user" => Ok(Self::User),
            "admin" => Ok(Self::Admin),
            "problemsetter" => Ok(Self::ProblemSetter),
            _ => Err("No such role exists"),
        }
    }
}
