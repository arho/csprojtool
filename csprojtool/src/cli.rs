use clap::*;

pub const ARG_CLEAN_APP_CONFIG: &'static str = "clean-app-config";
pub const ARG_DOT: &'static str = "dot";
pub const ARG_EXCLUDE_SDK: &'static str = "exclude-sdk";
pub const ARG_NO_FOLLOW: &'static str = "no-follow";
pub const ARG_GLOB: &'static str = "glob";
pub const ARG_JSON: &'static str = "json";
pub const ARG_SEARCH: &'static str = "search";
pub const CMD_DEPENDENCY_GRAPH: &'static str = "dependency-graph";
pub const CMD_LIST_PROJECTS: &'static str = "list-projects";
pub const CMD_POST_MIGRATION_CLEANUP: &'static str = "post-migration-cleanup";

#[cfg(windows)]
const DEFAULT_GLOB: &'static str = "**\\*.csproj";
#[cfg(not(windows))]
const DEFAULT_GLOB: &'static str = "**/*.csproj";

#[cfg(windows)]
const DEFAULT_SEARCH: &'static str = ".\\";
#[cfg(not(windows))]
const DEFAULT_SEARCH: &'static str = "./";

pub fn build_cli() -> App<'static, 'static> {
    let arg_glob = &Arg::with_name(ARG_GLOB)
        .short("g")
        .long("glob")
        .value_name("GLOB")
        .help("Specifies the glob pattern for which files to include")
        .takes_value(true)
        .default_value(DEFAULT_GLOB);

    let arg_search = &Arg::with_name(ARG_SEARCH)
        .value_name("PATH")
        .help("Sets the file to process or directory to search")
        .index(1)
        .default_value(DEFAULT_SEARCH);

    let arg_no_follow = &Arg::with_name(ARG_NO_FOLLOW)
        .short("F")
        .long("no-follow")
        .takes_value(false)
        .help("Do not follow project references when searching for projects");

    let arg_clean_app_config = &Arg::with_name(ARG_CLEAN_APP_CONFIG)
        .long("clean-app-config")
        .takes_value(false)
        .help("Clean up app.config files");

    let exclude_sdk = &Arg::with_name(ARG_EXCLUDE_SDK).long("exclude-sdk");

    App::new("csprojtool")
        .version("0.1.0")
        .author("Mick van Gelderen <mick@logiqs.nl>")
        .about("Manages csproj files")
        .setting(AppSettings::SubcommandRequiredElseHelp)
        .subcommands(vec![
            clap::SubCommand::with_name(CMD_DEPENDENCY_GRAPH)
                .about("Generate dependency graph of project references")
                .arg(arg_glob)
                .arg(arg_search)
                .arg(
                    Arg::with_name(ARG_DOT)
                        .long("dot")
                        .value_name("PATH")
                        .help("Writes the output to a dot file"),
                )
                .arg(
                    Arg::with_name(ARG_JSON)
                        .long("json")
                        .value_name("PATH")
                        .help("Writes the output to a json file"),
                ),
            clap::SubCommand::with_name(CMD_POST_MIGRATION_CLEANUP)
                .about("Perform post csproj migration cleanup")
                .arg(arg_glob)
                .arg(arg_search)
                .arg(arg_no_follow)
                .arg(arg_clean_app_config),
            clap::SubCommand::with_name(CMD_LIST_PROJECTS)
                .about("List all projects and their dependencies")
                .arg(arg_glob)
                .arg(arg_search)
                .arg(arg_no_follow)
                .arg(exclude_sdk),
            crate::move_command::MoveCommand::subcommand(),
        ])
}
