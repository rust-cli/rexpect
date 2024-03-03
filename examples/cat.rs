use rexpect::error::Error;
use rexpect::spawn;

fn main() -> Result<(), Error> {
	let mut p = spawn("cat", Some(1000))?;

	let ex: String = "∀".to_string();
	p.send_line(&ex)?;
	let line = p.read_line()?;

	println!("In: {}", &ex);
	println!("Out: {}", &line);
	Ok(())
}
