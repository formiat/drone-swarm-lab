use std::env;
use std::error::Error;
use std::fmt;
use std::path::PathBuf;

use swarm_scenarios::{
    ScenarioGenerator, SyntheticScenarioCategory, SyntheticScenarioLibrary, SyntheticUrbanConfig,
    SyntheticUrbanGenerator,
};
use swarm_sim::export_suite;

#[derive(Clone, Debug, PartialEq)]
struct GenerateArgs {
    family: String,
    category: SyntheticScenarioCategory,
    seed: u64,
    rows: Option<usize>,
    cols: Option<usize>,
    output: PathBuf,
    force: bool,
}

impl Default for GenerateArgs {
    fn default() -> Self {
        Self {
            family: "urban".to_owned(),
            category: SyntheticScenarioCategory::Tiny,
            seed: 0,
            rows: None,
            cols: None,
            output: PathBuf::from("scenarios/urban.generated.tiny.json"),
            force: false,
        }
    }
}

#[derive(Debug, PartialEq)]
enum GenerateCliError {
    Help,
    MissingValue(String),
    UnknownFlag(String),
    InvalidValue { flag: String, value: String },
    UnsupportedFamily(String),
    OutputExists(PathBuf),
}

impl fmt::Display for GenerateCliError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Help => write!(f, "{}", usage()),
            Self::MissingValue(flag) => write!(f, "missing value for {flag}"),
            Self::UnknownFlag(flag) => write!(f, "unknown flag {flag}"),
            Self::InvalidValue { flag, value } => write!(f, "invalid value for {flag}: {value}"),
            Self::UnsupportedFamily(family) => write!(f, "unsupported family: {family}"),
            Self::OutputExists(path) => write!(
                f,
                "output exists: {}; pass --force to overwrite",
                path.display()
            ),
        }
    }
}

impl Error for GenerateCliError {}

fn main() {
    if let Err(error) = run(env::args().skip(1)) {
        if matches!(
            error.downcast_ref::<GenerateCliError>(),
            Some(GenerateCliError::Help)
        ) {
            println!("{}", usage());
            return;
        }
        eprintln!("error: {error}");
        std::process::exit(2);
    }
}

fn run(args: impl IntoIterator<Item = String>) -> Result<(), Box<dyn Error>> {
    let args = parse_args(args)?;
    if args.output.exists() && !args.force {
        return Err(Box::new(GenerateCliError::OutputExists(args.output)));
    }

    let config = config_from_args(&args)?;
    let generated = SyntheticUrbanGenerator.generate(&config)?;
    let json = export_suite(&generated.suite)?;

    if let Some(parent) = args
        .output
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(&args.output, format!("{json}\n"))?;
    println!(
        "wrote {} with generator={} version={} seed={} category={}",
        args.output.display(),
        generated.manifest.generator_name,
        generated.manifest.generator_version,
        generated.manifest.seed,
        generated.manifest.category
    );
    Ok(())
}

fn config_from_args(args: &GenerateArgs) -> Result<SyntheticUrbanConfig, GenerateCliError> {
    if args.family != "urban" {
        return Err(GenerateCliError::UnsupportedFamily(args.family.clone()));
    }
    let mut config = SyntheticScenarioLibrary::urban_for_category(args.category, args.seed);
    config.category = args.category;
    if let Some(rows) = args.rows {
        config.rows = rows;
    }
    if let Some(cols) = args.cols {
        config.cols = cols;
    }
    Ok(config)
}

fn parse_args(args: impl IntoIterator<Item = String>) -> Result<GenerateArgs, GenerateCliError> {
    let mut parsed = GenerateArgs::default();
    let mut args = args.into_iter();
    while let Some(flag) = args.next() {
        match flag.as_str() {
            "--help" | "-h" => return Err(GenerateCliError::Help),
            "--family" => parsed.family = next_value(&flag, &mut args)?,
            "--category" => {
                let value = next_value(&flag, &mut args)?;
                parsed.category = value.parse().map_err(|_| GenerateCliError::InvalidValue {
                    flag: flag.clone(),
                    value,
                })?;
            }
            "--seed" => parsed.seed = parse_value(&flag, next_value(&flag, &mut args)?)?,
            "--rows" => parsed.rows = Some(parse_value(&flag, next_value(&flag, &mut args)?)?),
            "--cols" => parsed.cols = Some(parse_value(&flag, next_value(&flag, &mut args)?)?),
            "--output" => parsed.output = PathBuf::from(next_value(&flag, &mut args)?),
            "--force" => parsed.force = true,
            _ => return Err(GenerateCliError::UnknownFlag(flag)),
        }
    }
    Ok(parsed)
}

fn next_value(
    flag: &str,
    args: &mut impl Iterator<Item = String>,
) -> Result<String, GenerateCliError> {
    args.next()
        .ok_or_else(|| GenerateCliError::MissingValue(flag.to_owned()))
}

fn parse_value<T: std::str::FromStr>(flag: &str, value: String) -> Result<T, GenerateCliError> {
    value.parse().map_err(|_| GenerateCliError::InvalidValue {
        flag: flag.to_owned(),
        value,
    })
}

fn usage() -> &'static str {
    "Usage: generate_scenario_suite [--family urban] [--category tiny|small|medium|stress|regression-stable|experimental] [--seed N] [--rows N] [--cols N] --output PATH [--force]"
}

#[cfg(test)]
mod scenario_generator_cli_tests {
    use super::*;

    #[test]
    fn parse_defaults_to_tiny_urban_suite() {
        let args = parse_args(Vec::<String>::new()).unwrap();

        assert_eq!(args.family, "urban");
        assert_eq!(args.category, SyntheticScenarioCategory::Tiny);
        assert_eq!(args.seed, 0);
    }

    #[test]
    fn parse_accepts_overrides() {
        let args = parse_args(
            [
                "--category",
                "small",
                "--seed",
                "42",
                "--rows",
                "4",
                "--cols",
                "5",
                "--output",
                "out.json",
                "--force",
            ]
            .into_iter()
            .map(str::to_owned),
        )
        .unwrap();

        assert_eq!(args.category, SyntheticScenarioCategory::Small);
        assert_eq!(args.seed, 42);
        assert_eq!(args.rows, Some(4));
        assert_eq!(args.cols, Some(5));
        assert_eq!(args.output, PathBuf::from("out.json"));
        assert!(args.force);
    }

    #[test]
    fn config_rejects_unsupported_family() {
        let args = GenerateArgs {
            family: "flood".to_owned(),
            ..GenerateArgs::default()
        };

        let error = config_from_args(&args).unwrap_err();

        assert_eq!(
            error,
            GenerateCliError::UnsupportedFamily("flood".to_owned())
        );
    }
}
