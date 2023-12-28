#[cfg(test)]
mod test;

use std::{
    collections::HashMap,
    io::Write,
    path::{Path, PathBuf},
    process::Stdio,
};

use clap::Parser;
use error_stack::{Report, ResultExt};
use itertools::Itertools;
use rayon::prelude::*;
use tera::Tera;

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
    context: Option<tera::Context>,

    /// Print output as files are processed.
    #[clap(short, long, action=clap::ArgAction::Count)]
    verbose: u8,

    /// Print rerun-if-changed statements for build.rs.
    #[clap(long)]
    print_rerun_if_changed: bool,

    /// Traverse normally-ignored directories such as those in .gitignore.
    #[clap(long)]
    check_ignored_dirs: bool,

    /// Customize the header line that will be added to the generated files.
    /// The SQL comment prefix will be added automatically.
    #[clap(long)]
    header: Option<String>,

    /// Customize the extension that will be added to the generated files.
    #[clap(long = "ext")]
    extension: Option<String>,

    /// Always write the output files, even if the rendered template is identical to the file's
    /// current contents.
    #[clap(long)]
    always_write: bool,

    /// If provided, format the files using this command.
    ///
    /// The command should take output on stdin and return the formatted output on stdout.
    #[clap(short, long)]
    formatter: Option<String>,
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
    #[error("Multiple partials or macro files with the same name were found")]
    DuplicatePartial,
    #[error("Failed to run SQL formatter")]
    Formatter,
}

pub fn build(options: Options) -> Result<(), Report<Error>> {
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

            filename.ends_with(".sql.tera")
        });

    let walker = walker.build_parallel();

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

    #[derive(PartialEq, Eq, Copy, Clone)]
    enum TemplateType {
        Macro,
        Partial,
        Normal,
    }

    const MACRO_SUFFIX: &str = ".macros.sql.tera";
    const PARTIAL_SUFFIX: &str = ".partial.sql.tera";
    fn template_type(path: &Path) -> TemplateType {
        let p = path.to_string_lossy();
        match p {
            p if p.ends_with(MACRO_SUFFIX) => TemplateType::Macro,
            p if p.ends_with(PARTIAL_SUFFIX) => TemplateType::Partial,
            _ => TemplateType::Normal,
        }
    }

    let mut tera = Tera::default();
    let mut partials: HashMap<String, PathBuf> = HashMap::new();
    let mut templates = vec![];

    for path in file_rx {
        if options.print_rerun_if_changed {
            println!("cargo:rerun-if-changed={}", path.display());
        }

        let template_name = path.strip_prefix(&input_dir).unwrap();

        let typ = template_type(&template_name);
        let template_name = match typ {
            TemplateType::Normal => template_name.to_string_lossy().to_string(),
            TemplateType::Macro => template_name
                .file_name()
                .unwrap()
                .to_string_lossy()
                .strip_suffix(MACRO_SUFFIX)
                .unwrap()
                .to_string(),
            TemplateType::Partial => template_name
                .file_name()
                .unwrap()
                .to_string_lossy()
                .strip_suffix(PARTIAL_SUFFIX)
                .unwrap()
                .to_string(),
        };

        if typ != TemplateType::Normal {
            if let Some(existing) = partials.get(&template_name) {
                return Err(Error::DuplicatePartial)
                    .attach_printable(existing.display().to_string())
                    .attach_printable(path.display().to_string());
            }

            partials.insert(template_name.clone(), path.clone());
        }

        templates.push((path, Some(template_name)));
    }

    tera.add_template_files(templates.clone())
        .change_context(Error::ReadTemplate)?;

    if tera.get_template_names().next().is_none() {
        if options.verbose >= 1 {
            println!("No templates found");
        }
        return Ok(());
    }

    let context = options.context.unwrap_or_default();

    let extension = options.extension.as_deref().unwrap_or("sql");

    templates
        .into_par_iter()
        .filter(|(path, _)| template_type(path) == TemplateType::Normal)
        .try_for_each(|(path, name)| {
            let name = name.unwrap();
            let output = tera
                .render(&name, &context)
                .change_context(Error::Render)
                .attach_printable_lazy(|| path.display().to_string())?;

            let template_base_name = path
                .file_name()
                .and_then(|s| s.to_str())
                .unwrap_or_default()
                .strip_suffix(".sql.tera")
                .ok_or(Error::InternalError)
                .attach_printable_lazy(|| {
                    format!(
                        "Template path did not end in .sql.tera: {}",
                        path.display().to_string()
                    )
                })?;

            let output_filename = format!("{template_base_name}.{extension}");
            let output_path = if let Some(output) = options.output.as_ref() {
                output.join(output_filename)
            } else {
                path.with_file_name(output_filename)
            };

            let header = options
                .header
                .as_deref()
                .unwrap_or("Autogenerated by sqlweld");

            let header_lines = header
                .split(['\n', '\r'])
                .map(|s| s.trim())
                .filter(|s| !s.is_empty())
                .map(|s| format!("-- {}", s))
                .join("\n");

            let output = if header_lines.is_empty() {
                output
            } else {
                format!("{}\n\n{}", header_lines, output)
            };

            let output = if let Some(formatter) = options.formatter.as_ref() {
                let mut format_process = std::process::Command::new(formatter)
                    .stdin(Stdio::piped())
                    .stdout(Stdio::piped())
                    .stderr(Stdio::piped())
                    .spawn()
                    .change_context(Error::Formatter)?;

                let mut stdin = format_process.stdin.take().ok_or(Error::Formatter)?;
                let writer_thread = std::thread::spawn(move || {
                    stdin
                        .write_all(output.as_bytes())
                        .change_context(Error::Formatter)
                });

                let result = format_process
                    .wait_with_output()
                    .change_context(Error::Formatter)?;

                writer_thread
                    .join()
                    .expect("format writer thread")
                    .change_context(Error::Formatter)?;

                let code = result.status.code().unwrap_or(0);
                if !result.status.success() {
                    return Err(Error::Formatter)
                        .attach_printable(format!("Formatter exited with code {code}"))
                        .attach_printable(String::from_utf8(result.stderr).unwrap_or_default());
                }

                let output = result.stdout;

                String::from_utf8(output).change_context(Error::Formatter)?
            } else {
                output
            };

            if !options.always_write {
                if let Ok(existing) = std::fs::read_to_string(&output_path) {
                    if existing == output {
                        if options.verbose >= 3 {
                            println!(
                                "Skipping {} because it did not change",
                                output_path.display()
                            );
                        }
                        return Ok(());
                    }
                }
            }

            if options.verbose >= 1 {
                println!("Writing {}", output_path.display());
            }

            write_file(&output_path, &output)?;

            Ok::<_, Report<Error>>(())
        })?;

    Ok(())
}

fn atomic_write_file(path: &Path, contents: &str) -> Result<(), std::io::Error> {
    let mut temp = tempfile::NamedTempFile::new()?;
    temp.write_all(contents.as_bytes())?;
    temp.persist(path)?;
    Ok(())
}

fn write_file(path: &Path, contents: &str) -> Result<(), Report<Error>> {
    let atomic_result = atomic_write_file(path, contents);
    if atomic_result.is_ok() {
        return Ok(());
    }

    std::fs::write(path, contents)
        .change_context(Error::WriteResult)
        .attach_printable_lazy(|| path.display().to_string())?;
    Ok(())
}
