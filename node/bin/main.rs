//! Massbit collator binary.

#![warn(missing_docs)]

fn main() -> Result<(), sc_cli::Error> {
	massbit_collator::run()
}
