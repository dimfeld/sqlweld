use std::path::PathBuf;

use clap::Parser;
use error_stack::{Report, ResultExt};
use liquid::partials::{EagerCompiler, InMemorySource};
use rayon::prelude::*;

#[derive(Debug, Default, Parser)]
pub struct Options {
    /// Where to look for input files. This can be a glob. If omitted, the current directory is used.
    #[clap(short, long)]
    input: Option<PathBuf>,

    /// Where to write the output files. If omitted, output files are written to the same directory as the input files.
    #[clap(short, long)]
    output: Option<PathBuf>,

    /// Extra context to pass into the templates.
    #[clap(skip)]
    context: Option<liquid::Object>,

    /// Print output as files are processed.
    #[clap(short, long)]
    verbose: bool,

    /// Print rerun-if-changed statements for build.rs.
    #[clap(long)]
    print_rerun_if_changed: bool,

    /// Traverse normally-ignored directories such as those in .gitignore.
    #[clap(long)]
    check_ignored_dirs: bool,
}

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("Failed to read template file")]
    ReadTemplate,
    #[error("Failed to render template")]
    Render,
    #[error("Failed to write render result")]
    WriteResult,
    #[error("Internal consistency error")]
    InternalError,
}

pub fn build(options: Options) -> Result<(), Report<Error>> {
    // Find the files
    let input_dir = options
        .input
        .unwrap_or_else(|| std::env::current_dir().expect("getting current directory"));

    let mut walker = ignore::WalkBuilder::new(&input_dir);

    walker
        .hidden(!options.check_ignored_dirs)
        .follow_links(false)
        .filter_entry(|e| {
            let file_type = e.file_type();

            if file_type.map(|f| f.is_dir()).unwrap_or(false) {
                return true;
            } else if !file_type.map(|f| f.is_file()).unwrap_or(false) {
                return false;
            }

            let Some(filename) = e.file_name().to_str() else {
                return false;
            };

            filename.ends_with(".sql.liquid")
        });

    let walker = walker.build_parallel();

    let mut templates = vec![];

    let (file_tx, file_rx) = flume::bounded(64);

    std::thread::spawn(move || {
        let file_tx = file_tx;
        walker.run(|| {
            let file_tx = file_tx.clone();

            Box::new(move |result| {
                let Ok(result) = result else {
                    return ignore::WalkState::Skip;
                };

                if !result.file_type().map(|f| f.is_file()).unwrap_or(false) {
                    return ignore::WalkState::Continue;
                }

                let path = result.path().to_owned();
                match file_tx.send(path) {
                    Ok(_) => ignore::WalkState::Continue,
                    Err(_) => ignore::WalkState::Quit,
                }
            })
        });
    });

    let mut partial_source = InMemorySource::new();

    for path in file_rx {
        if options.print_rerun_if_changed {
            println!("cargo:rerun-if-changed={}", path.display());
        }

        let name = path
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or_default();

        let partial_name = name.strip_suffix(".partial.sql.liquid");
        if let Some(partial_name) = partial_name {
            if options.verbose {
                println!("Reading partial {} from {}", partial_name, path.display());
            }

            let contents = std::fs::read_to_string(&path)
                .change_context(Error::ReadTemplate)
                .attach_printable_lazy(|| path.display().to_string())?;
            partial_source.add(format!("{partial_name}.liquid"), contents);
        } else {
            // We read the templates later.
            templates.push(path);
        }
    }

    let partials = EagerCompiler::new(partial_source);
    let parser = liquid::ParserBuilder::with_stdlib()
        .partials(partials)
        .build()
        .expect("building parser");

    let context = options.context.unwrap_or_default();

    if templates.is_empty() {
        if options.verbose {
            println!("No templates found");
        }
        return Ok(());
    }

    templates.into_par_iter().try_for_each(|path| {
        let template = parser
            .parse_file(&path)
            .change_context(Error::ReadTemplate)
            .attach_printable_lazy(|| path.display().to_string())?;

        let result = template
            .render(&context)
            .change_context(Error::Render)
            .attach_printable_lazy(|| path.display().to_string())?;

        let template_base_name = path
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or_default()
            .strip_suffix(".sql.liquid")
            .ok_or(Error::InternalError)
            .attach_printable_lazy(|| {
                format!(
                    "Template path did not end in .sql.liquid: {}",
                    path.display().to_string()
                )
            })?;

        let output_filename = format!("{template_base_name}.sql");
        let output_path = if let Some(output) = options.output.as_ref() {
            output.join(output_filename)
        } else {
            path.with_file_name(output_filename)
        };

        if options.verbose {
            println!(
                "Writing template\n    {}\nto  {}",
                path.display(),
                output_path.display()
            );
        }

        std::fs::write(&output_path, result)
            .change_context(Error::WriteResult)
            .attach_printable_lazy(|| output_path.display().to_string())?;

        Ok::<_, Report<Error>>(())
    })?;

    // Read the partials
    // Load the context
    Ok(())
}