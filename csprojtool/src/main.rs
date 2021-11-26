mod cli;
mod csproj;
mod dependency_graph;
mod list;
mod move_command;
mod path_extensions;
mod post_migration_cleanup;
mod utils;
mod xml_extensions;
pub use dependency_graph::*;
pub use post_migration_cleanup::*;
mod sln;

use std::path::{Path, PathBuf};

fn get_glob_matcher(matches: &clap::ArgMatches) -> globset::GlobMatcher {
    let glob_pattern = matches.value_of(cli::ARG_GLOB).unwrap();
    globset::Glob::new(glob_pattern).unwrap().compile_matcher()
}

fn get_search_path(matches: &clap::ArgMatches) -> PathBuf {
    let search_path = matches.value_of(cli::ARG_SEARCH_PATH).unwrap();
    Path::new(search_path).components().collect()
}

fn main() {
    ::pretty_env_logger::init();

    let app = cli::build_cli();
    let matches = app.get_matches();

    if let Some(matches) = matches.subcommand_matches(cli::CMD_DEPENDENCY_GRAPH) {
        let glob = matches.value_of(cli::ARG_GLOB).unwrap();
        let search = matches.value_of(cli::ARG_SEARCH_PATH).unwrap();
        let dot = matches.value_of(cli::ARG_DOT);
        let json = matches.value_of(cli::ARG_JSON);
        dependency_graph(glob, search, dot, json);
    }

    if let Some(matches) = matches.subcommand_matches(cli::CMD_POST_MIGRATION_CLEANUP) {
        post_migration_cleanup(&PostMigrationCleanupOptions {
            search_path: get_search_path(&matches),
            glob_matcher: get_glob_matcher(&matches),
            follow_project_references: !matches
                .is_present(cli::ARG_DO_NOT_FOLLOW_OUTGOING_PROJECT_REFERENCES),
            clean_app_configs: matches.is_present(cli::ARG_CLEAN_APP_CONFIG),
        });
    }

    if let Some(matches) = matches.subcommand_matches(cli::CMD_LIST) {
        list::run(list::Options {
            search_path: &get_search_path(&matches),
            follow_incoming_project_references: !matches
                .is_present(cli::ARG_DO_NOT_FOLLOW_INCOMING_PROJECT_REFERENCES),
            follow_outgoing_project_references: !matches
                .is_present(cli::ARG_DO_NOT_FOLLOW_OUTGOING_PROJECT_REFERENCES),
        });
    }

    if let Some(matches) = matches.subcommand_matches(cli::CMD_SLN) {
        sln::sln(sln::Options {
            sln_path: &std::path::PathBuf::from(matches.value_of(cli::ARG_SLN_PATH).unwrap()),
            search_path: &get_search_path(&matches),
            follow_incoming_project_references: !matches
                .is_present(cli::ARG_DO_NOT_FOLLOW_INCOMING_PROJECT_REFERENCES),
            follow_outgoing_project_references: !matches
                .is_present(cli::ARG_DO_NOT_FOLLOW_OUTGOING_PROJECT_REFERENCES),
        });
    }

    if let Some(command) = move_command::MoveCommand::try_from_matches(&matches) {
        command.execute();
    }
}
