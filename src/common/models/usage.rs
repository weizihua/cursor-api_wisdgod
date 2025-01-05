use serde::{Deserialize, Serialize};

#[derive(Serialize)]
pub enum GetUserInfo {
    #[serde(rename = "usage")]
    Usage(UserUsageInfo),
    #[serde(rename = "error")]
    Error(String),
}

#[derive(Serialize, Clone)]
pub struct UserUsageInfo {
    pub fast_requests: u32,
    pub max_fast_requests: u32,
    pub mtype: String,
    pub trial_days: u32,
}

impl rusqlite::types::FromSql for UserUsageInfo {
    fn column_result(value: rusqlite::types::ValueRef<'_>) -> rusqlite::types::FromSqlResult<Self> {
        let str = value.as_str()?;
        let parts: Vec<&str> = str.split(',').collect();
        if parts.len() != 4 {
            return Err(rusqlite::types::FromSqlError::InvalidType);
        }

        Ok(UserUsageInfo {
            fast_requests: parts[0].parse().map_err(|_| rusqlite::types::FromSqlError::InvalidType)?,
            max_fast_requests: parts[1].parse().map_err(|_| rusqlite::types::FromSqlError::InvalidType)?,
            mtype: parts[2].to_string(),
            trial_days: parts[3].parse().map_err(|_| rusqlite::types::FromSqlError::InvalidType)?,
        })
    }
}

impl rusqlite::ToSql for UserUsageInfo {
    fn to_sql(&self) -> rusqlite::Result<rusqlite::types::ToSqlOutput<'_>> {
        let str = format!("{},{},{},{}", 
            self.fast_requests,
            self.max_fast_requests,
            self.mtype,
            self.trial_days
        );
        Ok(rusqlite::types::ToSqlOutput::from(str))
    }
}

#[derive(Deserialize)]
pub struct StripeProfile {
    pub membership_type: String,
    pub days_remaining_on_trial: i32,
}
