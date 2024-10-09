use futures::future::BoxFuture;
use std::fmt;
use save_dweb_backend::common::DHTEntity;
use serde::{Deserialize, Serialize};
use save_dweb_backend::group::Group;
use save_dweb_backend::repo::Repo;

#[derive(Debug, Deserialize)]
pub struct RequestName {
    pub name: String,
}

impl fmt::Display for RequestName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "RequestName {{ name: {} }}", self.name)
    }
}


#[derive(Deserialize, Serialize)]
pub struct SnowbirdGroup {
    pub key: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
}

impl From<&Group> for SnowbirdGroup {
    fn from(group: &Group) -> Self {
        SnowbirdGroup {
            key: group.id().to_string(),
            name: None,
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
            let mut snowbird_groups: Vec<SnowbirdGroup> = self.iter()
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

// impl From<Group> for SnowbirdGroup {
//     fn from(group: Group) -> Self {
//         Self::from_backend_group(&group)
//     }
// }

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SnowbirdRepo {
    pub id: String,
}

impl SnowbirdRepo {
    pub fn from_dweb_repo(repo: &Repo) -> Self {
        SnowbirdRepo {
            id: repo.id().to_string(),
        }
    }
}

impl From<Repo> for SnowbirdRepo {
    fn from(repo: Repo) -> Self {
        Self::from_dweb_repo(&repo)
    }
}

// impl FromIterator<ThirdPartyRepo> for Vec<SnowbirdRepo> {
//     fn from_iter<I: IntoIterator<Item = ThirdPartyRepo>>(iter: I) -> Self {
//         iter.into_iter()
//             .map(|repo| SnowbirdRepo::from_third_party(&repo))
//             .collect()
//     }
// }