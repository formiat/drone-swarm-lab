mod aggregate;
mod display;
mod run;
#[cfg(test)]
use aggregate::percentile_of_sorted;
pub use aggregate::*;
pub use run::*;

#[cfg(test)]
mod tests;
