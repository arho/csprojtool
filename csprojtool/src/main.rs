mod cli;
mod csproj;
mod dependency_graph;
mod list_projects;
mod path_extensions;
mod post_migration_cleanup;

pub use dependency_graph::*;
pub use list_projects::*;
pub use post_migration_cleanup::*;

use std::path::{Path, PathBuf};

fn get_glob_matcher(matches: &clap::ArgMatches) -> globset::GlobMatcher {
    let glob_pattern = matches.value_of(cli::ARG_GLOB).unwrap();
    globset::Glob::new(glob_pattern).unwrap().compile_matcher()
}

fn get_search_path(matches: &clap::ArgMatches) -> PathBuf {
    let search_path = matches.value_of(cli::ARG_SEARCH).unwrap();
    std::fs::canonicalize(search_path).unwrap()
}

fn main() {
    let app = cli::build_cli();
    let matches = app.get_matches();

    if let Some(matches) = matches.subcommand_matches(cli::CMD_DEPENDENCY_GRAPH) {
        let glob = matches.value_of(cli::ARG_GLOB).unwrap();
        let search = matches.value_of(cli::ARG_SEARCH).unwrap();
        let dot = matches.value_of(cli::ARG_DOT);
        let json = matches.value_of(cli::ARG_JSON);
        dependency_graph(glob, search, dot, json);
    }

    if let Some(matches) = matches.subcommand_matches(cli::CMD_POST_MIGRATION_CLEANUP) {
        post_migration_cleanup(&PostMigrationCleanupOptions {
            search_path: get_search_path(&matches),
            glob_matcher: get_glob_matcher(&matches),
            follow_project_references: !matches.is_present(cli::ARG_NO_FOLLOW),
            clean_app_configs: matches.is_present(cli::ARG_CLEAN_APP_CONFIG),
        });
    }

    if let Some(matches) = matches.subcommand_matches(cli::CMD_LIST_PROJECTS) {
        list_projects(&ListProjectsOptions {
            search_path: get_search_path(&matches),
            glob_matcher: get_glob_matcher(&matches),
            follow_project_references: !matches.is_present(cli::ARG_NO_FOLLOW),
            exclude_sdk: matches.is_present(cli::ARG_EXCLUDE_SDK),
        });
    }
}
