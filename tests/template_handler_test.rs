use std::error::Error;

use homeboy::templating::handler::TemplateHandler;

#[test]
fn test_template_loader() -> Result<(), Box<dyn Error>> {
    let mut handler = TemplateHandler::new();
    handler.load_from_file("./tests/res/test_template_file.txt")?;
    Ok(())
}
