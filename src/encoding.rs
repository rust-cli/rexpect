#[derive(Debug, PartialEq, Eq, Clone, Copy, Default)]
#[allow(non_snake_case)]
pub enum Encoding {
	#[default]
	ASCII,
	UTF8,
}
