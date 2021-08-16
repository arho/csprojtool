use crate::sln::*;
use std::path::PathBuf;
use std::ffi::OsStr;

 

pub struct ValidateSolutionsOptions {
    pub search_path: PathBuf,
    pub glob_matcher: globset::GlobMatcher,
}

pub fn validate_solutions(options: &ValidateSolutionsOptions) {
    let ValidateSolutionsOptions {
        ref search_path,
        ref glob_matcher
    } = *options;

    let solutions = read_and_parse_solutions(search_path, glob_matcher);
    let _csproj_ext = OsStr::new("csproj");
    for (sln_path, sln_result) in solutions {
        let mut sln_printed = false;
        let sln = sln_result.unwrap();
        for proj in sln.projects() { 
            let path_buf =  proj.path();
            let path = path_buf.as_path();
            match path.extension() {
               Some(_csproj_ext) => {
                    if path.exists() {
                            // do nothing
                        } else{ 
                            if !sln_printed {
                                println!("{}", sln_path.display());
                                sln_printed=true;
                            }
                            println!("\tMISSING: {}", path.display());
                        }
                } 
                None => {},
            }
            
        }
    }  
}