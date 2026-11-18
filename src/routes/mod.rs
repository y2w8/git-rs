use serde::Deserialize;

use crate::prelude::*;

pub mod about;
pub mod git;
pub mod repository;
pub mod r#static;

const PAGE_SIZE: usize = 50;

#[derive(Clone, Deserialize)]
pub struct PageQuery {
    #[serde(rename = "h")]
    pub head: Option<String>,

    #[serde(rename = "qt")]
    pub query_type: Option<String>,

    #[serde(rename = "v")]
    pub value: Option<String>,

    pub id: Option<String>,

    #[serde(rename = "ofs")]
    pub offset: Option<usize>,
}

impl PageQuery {
    pub fn id_param(&self) -> String {
        match &self.id {
            Some(id) => format!("?id={id}"),
            None => String::new(),
        }
    }

    pub fn head_param(&self) -> String {
        match (&self.id, &self.head) {
            (None, Some(head)) => format!("?h={head}"),
            _ => String::new(),
        }
    }

    pub fn id_or_head_param(&self) -> String {
        if let Some(id) = &self.id {
            format!("?id={id}")
        } else if let Some(head) = &self.head {
            format!("?h={head}")
        } else {
            String::new()
        }
    }
}

struct RepoEntry {
    repo: Repository,
    last_time: Option<Time>,
}

#[derive(Template)]
#[template(path = "repositorys.html")]
struct ReposPage {
    repos: Vec<RepoEntry>,
}

#[get("/")]
pub async fn page(query: web::Query<PageQuery>, data: web::Data<App>) -> ActixResult<HttpResponse> {
    let rendered = {
        let raw_repos: Vec<Repository> = {
            let guard = match data.repos.read() {
                Ok(g) => g,
                Err(e) => return Ok(HttpResponse::InternalServerError().body(e.to_string())),
            };

            match (query.query_type.as_deref(), query.value.as_deref()) {
                (Some(qt), Some(value)) => {
                    let value_lower = value.to_lowercase();
                    guard
                        .iter()
                        .filter(|r| {
                            field_for(r, Some(qt))
                                .map(|f| f.to_lowercase().contains(&value_lower))
                                .unwrap_or(false)
                        })
                        .cloned()
                        .collect()
                }
                _ => guard.clone(),
            }
        }; // guard dropped here

        // Resolve last_commit_time once per repo for both sorting and template rendering
        let mut repos: Vec<RepoEntry> = raw_repos
            .into_iter()
            .map(|repo| {
                let last_time = repo.last_commit_time();
                RepoEntry { repo, last_time }
            })
            .collect();

        if let (qt, Some(value)) = (query.query_type.as_deref(), query.value.as_deref()) {
            let value_lower = value.to_lowercase();
            repos.sort_by_key(|a| {
                (
                    relevance(&a.repo, qt, &value_lower),
                    std::cmp::Reverse(
                        a.last_time
                            .as_ref()
                            .map(|t| t.seconds())
                            .unwrap_or(i64::MIN),
                    ),
                )
            });
        } else {
            // Auto-sort repositories by Idle/Age (most recently updated first)
            repos.sort_by_key(|a| {
                std::cmp::Reverse(
                    a.last_time
                        .as_ref()
                        .map(|t| t.seconds())
                        .unwrap_or(i64::MIN),
                )
            });
        }
        ReposPage { repos }.render()
    };
    match rendered {
        Ok(html) => Ok(HttpResponse::Ok().content_type("text/html").body(html)),
        Err(e) => Ok(HttpResponse::InternalServerError().body(e.to_string())),
    }
}

fn field_for<'a>(repo: &'a Repository, qt: Option<&str>) -> Option<&'a str> {
    match qt {
        Some("description") => Some(&repo.description),
        _ => Some(&repo.name),
    }
}

// Lower rank = better match, sorted first
fn relevance(repo: &Repository, qt: Option<&str>, value_lower: &str) -> u8 {
    match field_for(repo, qt) {
        Some(field) => {
            let field_lower = field.to_lowercase();
            if field_lower == value_lower {
                0
            } else if field_lower.starts_with(value_lower) {
                1
            } else {
                2 // must be a contains-match since we already filtered
            }
        }
        None => 3,
    }
}
