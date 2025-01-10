use std::{
	fmt::{self, Display, Formatter},
	iter::Iterator,
};

use lazy_static::lazy_static;
use rand::seq::IteratorRandom;
use regex::Regex;

use crate::check;

lazy_static! {
	static ref SPACE_REPLACE: Regex = Regex::new(r#" +"#).unwrap();
}

/// Renders `Some<T>` as `T` and does not render `None`.
pub struct OptDisplay<T: Display>(pub Option<T>);

impl<T: Display> Display for OptDisplay<T> {
	fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
		if let Some(value) = &self.0 {
			write!(f, "{}", value)
		} else {
			Ok(())
		}
	}
}

/// Formats a user's biography properly. Assumes util::check::biography succeeded before this function
pub fn biography<T: AsRef<str>>(s: T) -> String {
	let s = s.as_ref();

	// Get chars (filtered to only have MAX_NEW_LINES new lines)
	let mut accum = 0;
	let chars = s
		.chars()
		.filter(|c| {
			if c == &'\n' {
				accum += 1;

				accum <= check::MAX_NEW_LINES
			} else {
				true
			}
		})
		.collect::<Vec<char>>();

	if let Ok(string) = truncate_at_code_point(&chars, check::MAX_BIOGRAPHY_LEN) {
		// Replace chains of spaces
		SPACE_REPLACE.replace_all(&string, " ").into_owned()
	} else {
		"".to_owned()
	}
}

pub fn truncate_at_code_point(
	chars: &Vec<char>,
	length: usize,
) -> Result<String, std::string::FromUtf8Error> {
	let mut accum = 0;

	String::from_utf8(
		chars
			.iter()
			.map(|c| Vec::from(c.encode_utf8(&mut [0u8; 8]).as_bytes()))
			.filter(|c| {
				accum += c.len();

				accum < length + 1
			})
			.flatten()
			.collect(),
	)
}

pub fn item_list<'a, I: Iterator<Item = impl AsRef<str>>>(mut iter: I) -> String {
	let mut s = String::new();

	if let Some(item) = iter.next() {
		s.push_str(item.as_ref());
	}

	for item in iter {
		s.push_str(", ");
		s.push_str(item.as_ref());
	}

	s
}

pub fn str_to_ident(s: impl AsRef<str>) -> String {
	let s = s.as_ref().to_ascii_lowercase();
	let mut last_was_underscore = false;

	let dashed = s
		.chars()
		.filter_map(|c| match c {
			'0'..='9' | 'a'..='z' => {
				last_was_underscore = false;

				Some(c)
			}
			_ => {
				if !last_was_underscore {
					last_was_underscore = true;
					Some('-')
				} else {
					None
				}
			}
		})
		.collect::<String>();

	dashed.trim_matches('-').to_string()
}

pub fn gen_name_id(s: impl AsRef<str>, default: &str) -> String {
	assert!(
		default.len() + 8 <= check::MAX_IDENT_LEN,
		"default ident too long"
	);

	let proc_ident = str_to_ident(s);

	// Default
	let (proc_ident, rng_count) = if proc_ident.is_empty() {
		(default, 8)
	} else {
		(proc_ident.as_str(), 3)
	};

	// Choose a random hash to add to the name id
	let chars = "abcdefghijklmnopqrstuvwxyz1234567890"; // pragma: allowlist secret
	let mut rng = rand::thread_rng();
	let hash = std::iter::repeat_with(|| chars.chars().choose(&mut rng))
		.flatten()
		.take(rng_count);

	proc_ident
		.chars()
		.take(check::MAX_IDENT_LEN - 4)
		.chain(std::iter::once('-'))
		.chain(hash)
		.collect::<String>()
}
