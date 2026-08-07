use std::path::PathBuf;

use crate::{project::Project, storage::Storage};
use anyhow::Result;

#[derive(Debug, Clone, serde::Serialize)]
pub struct JSONProject {
    pub name: String,
    pub path: PathBuf,

    #[serde(default)]
    pub tags: Vec<String>,

    pub score: f64,

    pub broken: bool,
}

impl From<&Project> for JSONProject {
    fn from(p: &Project) -> Self {
        Self {
            name: p.name.clone(),
            path: p.path.clone(),
            tags: p.tags.clone(),
            score: p.frecency(),
            broken: !p.exists(),
        }
    }
}

pub struct ListOptions {
    pub limit: usize,
    pub json: bool,
    pub tags: Option<Vec<String>>,
    pub broken: bool,
}

pub fn list(options: ListOptions) -> Result<()> {
    let ListOptions {
        json,
        limit,
        tags,
        broken,
        ..
    } = options;

    let storage = Storage::load()?;
    let tags = tags.unwrap_or_default();
    let projects = storage.list_by_tags(&tags);

    let cwd = std::env::current_dir()?;

    let mut projects: Vec<&Project> = projects.iter().filter(|p| p.path != cwd).cloned().collect();

    if broken {
        projects.retain(|p| p.exists() == false);
    }

    if projects.is_empty() {
        if json {
            println!("[]");
        } else {
            println!("No projects found");
        }

        return Ok(());
    }

    if json {
        let projects: Vec<JSONProject> = projects
            .into_iter()
            .map(JSONProject::from)
            .take(limit)
            .collect();

        let json_projects = serde_json::to_string(&projects)?;
        println!("{}", json_projects);
    } else {
        for project in projects.iter().take(limit) {
            println!("{}", project.to_list_item());
            continue;
        }
    }

    Ok(())
}
