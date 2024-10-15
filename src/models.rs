use futures::future::BoxFuture;
use save_dweb_backend::common::DHTEntity;
use save_dweb_backend::group::Group;
use save_dweb_backend::repo::Repo;
use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Deserialize)]
pub struct GroupPath {
    pub group_id: String,
}

#[derive(Deserialize)]
pub struct GroupRepoPath {
    pub group_id: String,
    pub repo_id: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RequestName {
    pub name: String,
}

impl fmt::Display for RequestName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "RequestName {{ name: {} }}", self.name)
    }
}

#[derive(Debug, Deserialize, Serialize)]
pub struct SnowbirdGroup {
    pub key: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    pub uri: String,
}

impl From<&Group> for SnowbirdGroup {
    fn from(group: &Group) -> Self {
        SnowbirdGroup {
            key: group.id().to_string(),
            name: None,
            uri: group.get_url(),
        }
    }
}

impl SnowbirdGroup {
    pub async fn fill_name(&mut self, group: &Group) {
        self.name = Some(group.get_name().await.unwrap());
    }
}

pub trait IntoSnowbirdGroups {
    fn into_snowbird_groups(self) -> Vec<SnowbirdGroup>;
}

pub trait IntoSnowbirdGroupsWithNames {
    fn into_snowbird_groups_with_names(self) -> BoxFuture<'static, Vec<SnowbirdGroup>>;
}

impl IntoSnowbirdGroups for Vec<Box<Group>> {
    fn into_snowbird_groups(self) -> Vec<SnowbirdGroup> {
        self.iter()
            .map(AsRef::as_ref)
            .map(SnowbirdGroup::from)
            .collect()
    }
}

impl IntoSnowbirdGroupsWithNames for Vec<Box<Group>> {
    fn into_snowbird_groups_with_names(self) -> BoxFuture<'static, Vec<SnowbirdGroup>> {
        Box::pin(async move {
            let mut snowbird_groups: Vec<SnowbirdGroup> = self
                .iter()
                .map(AsRef::as_ref)
                .map(SnowbirdGroup::from)
                .collect();

            for (snowbird_group, boxed_group) in snowbird_groups.iter_mut().zip(self.iter()) {
                snowbird_group.fill_name(boxed_group.as_ref()).await;
            }

            snowbird_groups
        })
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SnowbirdRepo {
    pub id: String,
    pub name: String,
}

#[async_trait::async_trait]
pub trait AsyncFrom<T> {
    async fn async_from(value: T) -> Self;
}

#[async_trait::async_trait]
impl AsyncFrom<Repo> for SnowbirdRepo {
    async fn async_from(repo: Repo) -> Self {
        SnowbirdRepo {
            id: repo.id().to_string(),
            name: repo
                .get_name()
                .await
                .unwrap_or_else(|_| "Unknown".to_string()),
        }
    }
}

impl From<&Repo> for SnowbirdRepo {
    fn from(repo: &Repo) -> Self {
        SnowbirdRepo {
            id: repo.id().to_string(),
            name: "".to_string(),
        }
    }
}

impl From<Repo> for SnowbirdRepo {
    fn from(repo: Repo) -> Self {
        SnowbirdRepo {
            id: repo.id().to_string(),
            name: "".to_string(),
        }
    }
}

impl From<Box<Repo>> for SnowbirdRepo {
    fn from(boxed_repo: Box<Repo>) -> Self {
        Self::from(*boxed_repo)
    }
}

pub trait IntoSnowbirdRepos {
    fn into_snowbird_repos(self) -> Vec<SnowbirdRepo>;
}

impl IntoSnowbirdRepos for Vec<Repo> {
    fn into_snowbird_repos(self) -> Vec<SnowbirdRepo> {
        self.iter().map(SnowbirdRepo::from).collect()
    }
}
