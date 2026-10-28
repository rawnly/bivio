use crate::{
    fuzzy_scorer,
    git::{self, Worktree},
    storage::Storage,
    utils, Project,
};
use anyhow::Result;
use inquire::{
    ui::{RenderConfig, Styled},
    Select,
};
use serde::Serialize;
use std::path::Path;

pub struct PickOptions {
    pub query: Option<String>,
    pub tags: Option<Vec<String>>,
    pub show_broken: bool,
    pub json: bool,
}

#[derive(Serialize)]
struct PickResult<'a> {
    name: &'a str,
    path: &'a Path,
    project_path: &'a Path,
    worktree: Option<PickedWorktree<'a>>,
}

#[derive(Serialize)]
struct PickedWorktree<'a> {
    name: &'a str,
    branch: Option<&'a str>,
}

fn format_selection(project: &Project, worktree: Option<&Worktree>, json: bool) -> Result<String> {
    let path = worktree.map_or(project.path.as_path(), |wt| wt.path.as_path());
    if !json {
        return Ok(path.display().to_string());
    }

    Ok(serde_json::to_string(&PickResult {
        name: &project.name,
        path,
        project_path: &project.path,
        worktree: worktree.map(|wt| PickedWorktree {
            name: &wt.name,
            branch: wt
                .branch
                .as_deref()
                .map(|branch| branch.strip_prefix("refs/heads/").unwrap_or(branch)),
        }),
    })?)
}

pub fn pick(options: PickOptions) -> Result<()> {
    let PickOptions {
        query,
        show_broken,
        tags,
        json,
    } = options;
    let mut storage = Storage::load()?;

    let cwd = std::env::current_dir()?;
    let mut projects: Vec<Project> = match tags {
        None => storage
            .list()
            .into_iter()
            .filter(|p| p.path != cwd)
            .cloned()
            .collect(),
        Some(tags) => storage
            .list_by_tags(&tags)
            .into_iter()
            .filter(|p| p.path != cwd)
            .cloned()
            .collect(),
    };

    if projects.is_empty() {
        eprintln!("No projects available");
        std::process::exit(1);
    }

    if projects.iter().any(|p| !p.exists()) {
        if !show_broken {
            projects.retain(|p| p.exists());
        }

        let binary = env!("CARGO_BIN_NAME");

        eprintln!("WARN - Some projects points to non-existing path");
        eprintln!("run `{binary} list` to show broken projects");
        eprintln!();

        if utils::is_debug_enabled() {
            let broken_projects: Vec<Project> = projects
                .clone()
                .into_iter()
                .filter(|p| !p.exists())
                .collect();

            eprintln!("Found {} broken elements:", broken_projects.len());
            for project in broken_projects {
                eprintln!("- {}", project.name);
            }
            eprintln!();
        }
    }

    let projects = projects;
    fuzzy_scorer!(fuzzy_project_scorer, Project);

    let prompt_project_selection = |projects: &[Project], q: Option<String>| {
        Select::new("Select a project:", projects.to_vec())
            .with_starting_filter_input(&q.unwrap_or_default())
            .with_vim_mode(false)
            .with_scorer(&fuzzy_project_scorer)
            .with_render_config(
                RenderConfig::default_colored()
                    .with_scroll_up_prefix(Styled::new("↑"))
                    .with_scroll_down_prefix(Styled::new("↓")),
            )
            .prompt()
    };

    let project = if let Some(ref q) = query {
        let names: Vec<String> = projects.iter().map(|p| p.name.clone()).collect();
        let fuzzy_filtered = frizbee::match_list_indices(q, &names, &frizbee::Config::default());
        let pre_filtered: Vec<&Project> = fuzzy_filtered
            .iter()
            .filter_map(|m| projects.get(m.index as usize))
            .collect();

        if pre_filtered.len() == 1 {
            pre_filtered[0].clone()
        } else if pre_filtered.len() > 1 {
            prompt_project_selection(&projects, Some(q.clone()))?
        } else {
            prompt_project_selection(&projects, None)?
        }
    } else {
        prompt_project_selection(&projects, None)?
    };

    // If bare repo, show available worktrees
    let worktree = if project.is_bare_repo {
        match git::list_worktrees(&project.path) {
            Ok(worktrees) if worktrees.len() == 1 => worktrees.into_iter().next(),
            Ok(worktrees) if !worktrees.is_empty() => {
                fuzzy_scorer!(fuzzy_worktree_scorer, Worktree);

                let wt_selection = Select::new("Select a worktree:", worktrees)
                    .with_scorer(&fuzzy_worktree_scorer)
                    .with_help_message("<ESC> to skip this and navigate to the project root")
                    .with_vim_mode(false);

                match wt_selection.prompt_skippable() {
                    Ok(selected) => selected,
                    Err(_) => std::process::exit(1),
                }
            }
            _ => None,
        }
    } else {
        None
    };

    storage.update_access(&project.name)?;
    println!("{}", format_selection(&project, worktree.as_ref(), json)?);

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{json, Value};

    #[test]
    fn project_output_preserves_name_and_path() {
        let project = Project::new(
            "My \"project\"".into(),
            "/projects/my project".into(),
            false,
        );
        assert_eq!(
            format_selection(&project, None, false).unwrap(),
            "/projects/my project"
        );
        let result: Value =
            serde_json::from_str(&format_selection(&project, None, true).unwrap()).unwrap();
        assert_eq!(
            result,
            json!({
                "name": "My \"project\"",
                "path": "/projects/my project",
                "project_path": "/projects/my project",
                "worktree": null
            })
        );
    }

    #[test]
    fn worktree_output_keeps_project_identity() {
        let project = Project::new("bivio".into(), "/projects/bivio.git".into(), true);
        let mut worktree = Worktree {
            name: "feature-x".into(),
            path: "/projects/feature-x".into(),
            branch: Some("refs/heads/feature/x".into()),
        };
        assert_eq!(
            format_selection(&project, Some(&worktree), false).unwrap(),
            "/projects/feature-x"
        );
        let result: Value =
            serde_json::from_str(&format_selection(&project, Some(&worktree), true).unwrap())
                .unwrap();
        assert_eq!(
            result,
            json!({
                "name": "bivio",
                "path": "/projects/feature-x",
                "project_path": "/projects/bivio.git",
                "worktree": { "name": "feature-x", "branch": "feature/x" }
            })
        );
        worktree.branch = None;
        let result: Value =
            serde_json::from_str(&format_selection(&project, Some(&worktree), true).unwrap())
                .unwrap();
        assert!(result["worktree"]["branch"].is_null());
    }

    #[test]
    fn skipped_worktree_uses_project_root() {
        let project = Project::new("bivio".into(), "/projects/bivio.git".into(), true);
        let result: Value =
            serde_json::from_str(&format_selection(&project, None, true).unwrap()).unwrap();
        assert_eq!(result["path"], "/projects/bivio.git");
        assert!(result["worktree"].is_null());
    }
}
