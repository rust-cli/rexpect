#[derive(Debug, PartialEq, Eq, Clone, Copy, Default)]
#[allow(non_snake_case)]
pub enum Encoding {
	ASCII,
	#[default]
	UTF8,
}
