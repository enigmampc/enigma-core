use std::path::PathBuf;
use structopt::StructOpt;

#[derive(Debug, StructOpt)]
#[structopt(name = "example", about = "Enigma Core CLI commands.")]
pub(crate) struct Opt {
    /// Activate debug mode
    #[structopt(short = "v", long = "verbose", parse(from_occurrences))]
    pub verbose: u8,
    /// Print the debugging directly to stdout
    #[structopt(long = "debug-stdout")]
    pub debug_stdout: bool,
    /// Specify data directory
    #[structopt(parse(from_os_str), default_value="~/.enigma", long = "data-dir")]
    pub data_dir: PathBuf,
    /// Specify a different SPID to use for the Quote/Report
    #[structopt(long = "spid", default_value = "1601F95C39B9EA307FEAABB901ADC3EE")]
    pub spid: String,
    /// Select a port for the enigma-p2p listener
    #[structopt(long = "port", short = "p", default_value = "5552")]
    pub port: u16,
}